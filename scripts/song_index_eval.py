#!/usr/bin/env python3
"""Run E011's frozen queries against both complete reference layouts."""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

from chunk_query_eval import score
import real_mix_eval as e
from song_index_prepare import atomic_bytes

ROOT = Path('target/song-index')
BINARY_SHA = 'd3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc'
SOURCE_COMMIT = 'bace1a92feb7d9e862c21d295fd84df6a6df6626'


def outcome(events, references, expected):
    result = score(events, references, expected)
    if expected is None:
        result['outcome'] = 'false-accept' if result['starts'] else 'correct-rejection'
    reports = [r for r in events if r['event']=='report' and r.get('song') in references]
    correct = [r for r in reports if references[r['song']]['song']==expected]
    high = [r for r in correct if r['confidence'] >= 70]
    result['diagnostics'] = dict(
        strongest_report=max(reports, key=lambda r:r['evidence'], default=None),
        strongest_expected_report=max(correct, key=lambda r:r['evidence'], default=None),
        maximum_reported_expected_alignment_above_confidence=max((r['verify_q'] for r in high), default=None),
        scope='Report events expose only the greatest-evidence candidate at each reporting time.')
    return result


def validate_events(events, references, query):
    if not events or events[-1]['event'] != 'done' or events[-1]['audio_seconds'] != 10.0:
        raise ValueError(('incomplete native query', query['id']))
    indexed = [r for r in events if r['event']=='index']
    if len(indexed) != len(references) or {r['song'] for r in indexed} != set(references):
        raise ValueError(('wrong reference set', query['id']))
    for row in indexed:
        if abs(row['seconds']-references[row['song']]['seconds']) > .006:
            raise ValueError(('wrong indexed duration', row))


def run_one(binary, output, query, side, refs):
    key = query['id']+'-'+side
    command = [str(binary)]
    for ref in refs:
        command += ['--watch', ref['id']+'='+ref['path']]
    command += ['--stream', query['path']]
    row = dict(id=key, query_id=query['id'], group=query['group'], song=query['song'],
               supported=query['supported'], side=side, command=command,
               input=query, status='running')
    e.save(output/(key+'.active.json'), row)
    before = time.perf_counter()
    stdout, stderr = output/(key+'.jsonl'), output/(key+'.stderr.txt')
    try:
        try:
            proc = subprocess.run(command, capture_output=True, timeout=600,
                                  env=dict(os.environ, RAYON_NUM_THREADS='1'))
        except subprocess.TimeoutExpired as ex:
            atomic_bytes(stdout, ex.stdout or b'')
            atomic_bytes(stderr, ex.stderr or b'')
            raise
        atomic_bytes(stdout, proc.stdout)
        atomic_bytes(stderr, proc.stderr)
        row.update(returncode=proc.returncode, elapsed_seconds=time.perf_counter()-before,
                   stdout=e.identity(stdout), stderr=e.identity(stderr))
        if proc.returncode:
            raise RuntimeError(('native process failed', key, proc.returncode))
        events = [json.loads(line) for line in proc.stdout.splitlines()]
        reference_map = {r['id']:r for r in refs}
        validate_events(events, reference_map, query)
        row.update(status='completed', done=events[-1],
                   index=next(r for r in events if r['event']=='index_done'),
                   event_digest=e.stable_digest(events),
                   **outcome(events, reference_map, query['song']))
    except Exception as ex:
        row.update(status='failed', error=repr(ex), elapsed_seconds=time.perf_counter()-before)
    e.save(output/(key+'.result.json'), row)
    (output/(key+'.active.json')).unlink()
    print(key, row['status'], row.get('outcome'), row.get('predicted'),
          f"{row['elapsed_seconds']:.2f}s", flush=True)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label', default='default-a')
    parser.add_argument('--workers', type=int, choices=range(1,5), default=4)
    parser.add_argument('--binary', default='target/chunk-query/binaries/candidate')
    args = parser.parse_args()
    manifest_path = ROOT/'manifest.json'
    manifest = json.loads(manifest_path.read_text())
    binary = Path(args.binary).resolve()
    assert e.sha(binary) == BINARY_SHA
    assert (binary.parent/'candidate-commit.txt').read_text().strip() == SOURCE_COMMIT
    for rows in [*manifest['references'].values(), manifest['queries']]:
        for row in rows:
            assert e.sha(row['path']) == row['sha256'], row['path']
    for row in manifest['queries']:
        assert e.wav_info(row['path'])['samples'] == 441000
    output = ROOT/'runs'/args.label
    output.mkdir(parents=True, exist_ok=False)
    metadata = dict(status='running', binary=e.identity(binary), source_commit=SOURCE_COMMIT,
                    manifest=e.identity(manifest_path), evaluator=e.identity(__file__),
                    scorer=e.identity(Path(__file__).with_name('chunk_query_eval.py')),
                    helpers=e.identity(e.__file__), platform=platform.platform(), python=sys.version,
                    workers=args.workers, rayon_threads_per_process=1, native_options=[],
                    default_threshold=70, default_verify_start=.4, modal_fit=False,
                    source_correction=manifest['source_correction'],
                    expected_runs=2*len(manifest['queries']))
    e.save(output/'metadata.json', metadata)
    groups = ['exact', 'clean', 'frozen', 'mix', 'negative']
    queries = sorted(manifest['queries'], key=lambda q:(groups.index(q['group']), q['id']))
    results = []
    def paired(number, query):
        sides = ['chunks','full'] if number % 2 == 0 else ['full','chunks']
        return [run_one(binary, output, query, side, manifest['references'][side]) for side in sides]
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures = [pool.submit(paired, number, query) for number,query in enumerate(queries)]
            for future in concurrent.futures.as_completed(futures):
                results.extend(future.result())
                e.save(output/'results.json', sorted(results, key=lambda r:r['id']))
        assert len(results) == metadata['expected_runs']
        metadata.update(status='completed' if all(r['status']=='completed' for r in results) else 'failed',
                        completed_runs=sum(r['status']=='completed' for r in results))
    except BaseException as ex:
        metadata.update(status='failed', error=repr(ex))
        raise
    finally:
        e.save(output/'metadata.json', metadata)
    if metadata['status'] != 'completed':
        raise RuntimeError('Native failures retained in per-query results')


if __name__ == '__main__':
    main()
