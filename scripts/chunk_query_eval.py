#!/usr/bin/env python3
"""Paired, independent ten-second queries with explicit chunk-to-song scoring."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import real_mix_eval as e

ROOT = Path('target/chunk-query')


def score(events, reference_map, expected):
    starts = [event for event in events if event['event'] == 'start']
    ranked = sorted(starts, key=lambda row: (-row['evidence'], row['consumed'], row['song']))
    mapped = []
    for event in ranked:
        ref = reference_map[event['song']]
        mapped.append(dict(**event, parent_song=ref['song'], chunk_id=event['song'],
                           original_position=ref['start_seconds'] + event['position']))
    best = mapped[0] if mapped else None
    predicted = best['parent_song'] if best else None
    return dict(expected=expected, predicted=predicted,
                outcome='correct' if predicted == expected else ('no-match' if predicted is None else 'wrong-match'),
                best=best, starts=mapped, accepted_parent_songs=sorted({row['parent_song'] for row in mapped}),
                wrong_parent_starts=[row for row in mapped if row['parent_song'] != expected],
                repeated_parent_starts=len(mapped)-len({row['parent_song'] for row in mapped}))


def evaluate(args):
    sources = json.loads((ROOT/'sources.json').read_text())
    queries_path = ROOT/'queries.json'
    queries = json.loads(queries_path.read_text())
    assert len(queries['queries']) == 22
    assert {q['song'] for q in queries['queries']} == set(sources)-{'mix'}
    binary = Path(args.binary).resolve()
    references = {
        'midpoint': [sources[s]['midpoint'] for s in sorted(sources) if s != 'mix'],
        'chunks': [c for s in sorted(sources) if s != 'mix' for c in sources[s]['chunks']],
    }
    # Reject changed or incomplete input files before attributing any output.
    for rows in [*references.values(), queries['queries']]:
        for row in rows:
            assert e.sha(row['path']) == row['sha256'], row['path']
    output = ROOT/'runs'/args.label
    output.mkdir(parents=True, exist_ok=False)
    meta = dict(status='running', evaluator=e.identity(__file__), helpers=e.identity(e.__file__),
                binary=e.identity(binary), sources=e.identity(ROOT/'sources.json'),
                queries=e.identity(queries_path), platform=platform.platform(), python=sys.version,
                rayon_threads=1, native_options=[], comparison='same executable; reference coverage only',
                source_commit=Path(binary.parent/'candidate-commit.txt').read_text().strip(),
                reference_counts={side:len(refs) for side,refs in references.items()})
    e.save(output/'metadata.json', meta)
    runs = []
    try:
        for number, query in enumerate(queries['queries']):
            sides = ['midpoint', 'chunks'] if number % 2 == 0 else ['chunks', 'midpoint']
            for side in sides:
                refs = references[side]
                cmd = [str(binary)]
                for ref in refs:
                    cmd += ['--watch', ref['id']+'='+ref['path']]
                cmd += ['--stream', query['path']]
                key = query['song']+'-'+side
                stdout, stderr = output/(key+'.jsonl'), output/(key+'.stderr.txt')
                row = dict(song=query['song'], side=side, command=cmd, input=query, status='running')
                e.save(output/'active.json', row)
                begin = time.perf_counter()
                try:
                    proc = subprocess.run(cmd, capture_output=True,
                                          env=dict(os.environ, RAYON_NUM_THREADS='1'), timeout=600)
                except subprocess.TimeoutExpired as ex:
                    stdout.write_bytes(ex.stdout or b'')
                    stderr.write_bytes(ex.stderr or b'')
                    raise
                stdout.write_bytes(proc.stdout)
                stderr.write_bytes(proc.stderr)
                elapsed = time.perf_counter()-begin
                row.update(returncode=proc.returncode, elapsed_seconds=elapsed,
                           stdout=e.identity(stdout), stderr=e.identity(stderr))
                assert proc.returncode == 0, (key, proc.returncode)
                events = [json.loads(line) for line in stdout.read_text().splitlines()]
                assert events[-1]['event'] == 'done', key
                assert events[-1]['audio_seconds'] == 10.0, key
                assert e.wav_info(query['path'])['samples'] == 441000, key
                indexed = [v for v in events if v['event'] == 'index']
                assert len(indexed) == len(refs), key
                assert {v['song'] for v in indexed} == {r['id'] for r in refs}, key
                row.update(status='completed', done=events[-1],
                           annotation_supported=query['annotation']['status']=='algorithmically-supported',
                           index=next(v for v in events if v['event']=='index_done'),
                           event_digest=e.stable_digest(events),
                           **score(events, {ref['id']:ref for ref in refs}, query['song']))
                runs.append(row)
                e.save(output/'results.json', runs)
                print(key, row['outcome'], row['predicted'], f'{elapsed:.3f}s', flush=True)
        meta.update(status='completed', runs=len(runs))
    except BaseException as ex:
        meta.update(status='failed', error=repr(ex), completed_runs=len(runs))
        raise
    finally:
        e.save(output/'metadata.json', meta)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', default='target/chunk-query/binaries/candidate')
    p.add_argument('--label', default='default-a')
    evaluate(p.parse_args())


if __name__ == '__main__':
    main()
