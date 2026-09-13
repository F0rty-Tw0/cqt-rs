#!/usr/bin/env python3
"""E013 frozen query and controlled-boundary experiment, with raw evidence."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import wave

import numpy as np
import real_mix_eval as e
from song_index_prepare import atomic_bytes
from song_index_eval import outcome, validate_events

ROOT = Path('target/sequence')
OLD = Path('target/song-index')


def read(path):
    return json.loads(Path(path).read_text())


def prepare():
    ROOT.mkdir(parents=True, exist_ok=True)
    if (ROOT/'manifest.json').exists():
        raise RuntimeError('Frozen manifest already exists')
    old = read(OLD/'manifest.json')
    for r in old['queries'] + old['references']['full']:
        assert e.sha(r['path']) == r['sha256'], r['path']
    clips = sorted([q for q in old['queries'] if q['group'] == 'clean'], key=lambda q:q['id'])
    parts, intervals, samples = [], [], 0
    def silence(seconds):
        nonlocal samples
        data = np.zeros(round(seconds*44100), dtype='<i2')
        parts.append(data)
        samples += len(data)
    def pcm(q):
        with wave.open(q['path'], 'rb') as w:
            assert w.getnchannels() == 1 and w.getsampwidth() == 2 and w.getframerate() == 44100
            return np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').copy()
    def add_play(q, data, treatment, offset=0):
        intervals.append(dict(play_id=f'p{len(intervals)+1:02}', song=q['song'],
            start_sample=samples+offset, end_sample=samples+offset+len(data),
            source_start=q['start_seconds'], treatment=treatment, source_query=q['id']))
    for q in clips:
        silence(6)
        data = pcm(q)
        add_play(q, data, 'abrupt')
        parts.append(data)
        samples += len(data)
        silence(8)
    silence(6)
    data = pcm(clips[0])
    data[4*44100:5*44100] = 0
    add_play(clips[0], data, 'repeat-with-one-second-dropout')
    parts.append(data)
    samples += len(data)
    silence(8)
    silence(6)
    a, b = pcm(clips[0]), pcm(clips[1])
    add_play(clips[0], a, 'two-second-crossfade')
    add_play(clips[1], b, 'two-second-crossfade', offset=8*44100)
    n = 2*44100
    fade = np.linspace(0, 1, n, endpoint=False)
    overlap = np.rint(a[-n:]*(1-fade) + b[:n]*fade).clip(-32768, 32767).astype('<i2')
    data = np.concatenate([a[:-n], overlap, b[n:]])
    parts.append(data)
    samples += len(data)
    silence(8)
    path = ROOT/'programme.wav'
    import io
    buffer = io.BytesIO()
    with wave.open(buffer, 'wb') as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(44100)
        w.writeframes(b''.join(p.tobytes() for p in parts))
    atomic_bytes(path, buffer.getvalue())
    for p in intervals:
        p['start'] = p['start_sample']/44100
        p['end'] = p['end_sample']/44100
    manifest = dict(status='frozen', old_manifest=e.identity(OLD/'manifest.json'),
        references=old['references']['full'], queries=old['queries'],
        programme=dict(**e.identity(path), samples=samples, seconds=samples/44100, intervals=intervals),
        preparation=e.identity(__file__), protocol=e.identity('docs/experiments/E013-two-second-sequence.md'))
    e.save(ROOT/'manifest.json', manifest)
    print('Frozen:', len(old['queries']), 'queries;', len(intervals), 'controlled plays;', samples/44100, 'seconds')


def programme_score(events, truth):
    """Assign every start once using the support time; keep misses and false starts."""
    starts = [x for x in events if x['event'] == 'start']
    ends = [x for x in events if x['event'] == 'end']
    used, plays, false = set(), [], []
    for i, event in enumerate(starts):
        # Legacy t is query time; sequence t is the last contributing interval edge.
        matches = [p for p in truth if p['song'] == event['song'] and p['start'] <= event['t'] <= p['end']]
        if not matches:
            false.append(event)
    for p in truth:
        detected = [(i, s) for i, s in enumerate(starts)
                    if i not in used and s['song'] == p['song'] and p['start'] <= s['t'] <= p['end']]
        used.update(i for i, _ in detected)
        first = detected[0][1] if detected else None
        end_event = next((x for x in ends if first and x['song'] == p['song'] and x['t'] >= first['t']), None)
        plays.append(dict(**p, detected=bool(first), starts=len(detected),
            notification_start_delay=first['consumed']-p['start'] if first else None,
            notification_end_delay=end_event['consumed']-p['end'] if end_event else None,
            estimated_start_error=first['estimated_start']-p['start'] if first and 'estimated_start' in first else None,
            estimated_end_error=end_event['estimated_end']-p['end'] if end_event and 'estimated_end' in end_event else None,
            premature=bool(end_event and end_event['t'] < p['end']), first=first, end_event=end_event))
    return dict(plays=plays, detected=sum(p['detected'] for p in plays), total=len(plays),
                duplicate_starts=sum(max(0, p['starts']-1) for p in plays),
                false_starts=false, premature_endings=sum(p['premature'] for p in plays))


def run_one(binary, manifest, query, side, options):
    output = ROOT/'runs'
    key = query['id']+'-'+side
    if (output/(key+'.result.json')).exists():
        row = read(output/(key+'.result.json'))
        assert row['status'] == 'completed'
        assert row['binary_sha256'] == e.sha(binary)
        assert e.sha(row['stdout']['path']) == row['stdout']['sha256']
        return row
    command = [str(binary)]
    for r in manifest['references']:
        command += ['--watch', r['id']+'='+r['path']]
    command += ['--stream', query['path'], *options]
    row = dict(id=key, query_id=query['id'], group=query['group'], expected=query['song'],
               side=side, command=command, binary_sha256=e.sha(binary), status='running')
    e.save(output/(key+'.active.json'), row)
    before = time.perf_counter()
    stdout, stderr = output/(key+'.jsonl'), output/(key+'.stderr.txt')
    try:
        p = subprocess.run(command, capture_output=True, timeout=900,
                           env=dict(os.environ, RAYON_NUM_THREADS='1'))
        atomic_bytes(stdout, p.stdout); atomic_bytes(stderr, p.stderr)
        row.update(returncode=p.returncode, elapsed_seconds=time.perf_counter()-before,
                   stdout=e.identity(stdout), stderr=e.identity(stderr))
        assert p.returncode == 0, p.stderr[-2000:]
        events = [json.loads(line) for line in p.stdout.splitlines()]
        assert events[-1]['event'] == 'done'
        assert abs(events[-1]['audio_seconds'] - query['seconds']) < .006
        refs = {r['id']:r for r in manifest['references']}
        assert {x['song'] for x in events if x['event']=='index'} == set(refs)
        if query['group'] == 'programme':
            row['programme'] = programme_score(events, manifest['programme']['intervals'])
        else:
            validate_events(events, refs, query)
            row.update(outcome(events, refs, query['song']))
        row.update(status='completed', done=events[-1])
    except subprocess.TimeoutExpired as ex:
        atomic_bytes(stdout, ex.stdout or b''); atomic_bytes(stderr, ex.stderr or b'')
        row.update(status='failed', error=repr(ex), stdout=e.identity(stdout), stderr=e.identity(stderr))
    except Exception as ex:
        row.update(status='failed', error=repr(ex))
    e.save(output/(key+'.result.json'), row)
    (output/(key+'.active.json')).unlink()
    print(key, row['status'], row.get('outcome', row.get('programme', {}).get('detected')), flush=True)
    return row


def run(binary):
    m = read(ROOT/'manifest.json')
    for r in m['references'] + m['queries'] + [m['programme']]:
        assert e.sha(r['path']) == r['sha256'], r['path']
    output = ROOT/'runs'
    output.mkdir(parents=True, exist_ok=True)
    source = (binary.parent/'candidate-commit.txt').read_text().strip()
    e.save(ROOT/'metadata.json', dict(binary=e.identity(binary), source_commit=source,
        evaluator=e.identity(__file__), manifest=e.identity(ROOT/'manifest.json'),
        python=sys.version, platform=platform.platform(), workers=4, rayon_threads=1,
        protocol=e.identity('docs/experiments/E013-two-second-sequence.md')))
    jobs = []
    for q in m['queries']:
        for side, options in [('sequence', ['--sequence-seconds', '2']),
                              ('sequence-modal', ['--sequence-seconds', '2', '--modal-fit'])]:
            jobs.append((q, side, options))
    q = dict(m['programme'], id='programme', group='programme', song=None)
    for side, options in [('default', []), ('modal', ['--modal-fit']),
                          ('sequence', ['--sequence-seconds', '2']),
                          ('sequence-modal', ['--sequence-seconds', '2', '--modal-fit'])]:
        jobs.append((q, side, options))
    for block in [257, 65536]:
        jobs.append((q, f'sequence-block{block}', ['--sequence-seconds', '2', '--block', str(block)]))
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        futures = [pool.submit(run_one, binary, m, *j) for j in jobs]
        results = [f.result() for f in concurrent.futures.as_completed(futures)]
    e.save(ROOT/'results.json', sorted(results, key=lambda r:r['id']))
    assert len(results) == 198 and all(r['status'] == 'completed' for r in results)


def report():
    rows = read(ROOT/'results.json')
    for row in rows:
        assert row['status']=='completed'
        for key in ['stdout','stderr']:
            assert e.sha(row[key]['path']) == row[key]['sha256']
    comparisons = []
    for side, path in [('sequence', OLD/'runs/default-a/results.json'),
                       ('sequence-modal', OLD/'runs/modal-a/results.json')]:
        baseline = {r['query_id']:r for r in read(path) if r.get('side') == 'full' or side == 'sequence-modal'}
        for group in ['exact','clean','frozen','mix','negative']:
            group_rows = [r for r in rows if r['side']==side and r['group']==group]
            pairs = [(baseline[r['query_id']], r) for r in group_rows]
            correct = lambda r: r['outcome'] in ['correct', 'correct-rejection']
            comparisons.append(dict(side=side,group=group,n=len(pairs),
                baseline_correct=sum(correct(a) for a,b in pairs),candidate_correct=sum(correct(b) for a,b in pairs),
                gains=[b['query_id'] for a,b in pairs if correct(b) and not correct(a)],
                losses=[b['query_id'] for a,b in pairs if correct(a) and not correct(b)],
                no_match=[b['query_id'] for a,b in pairs if b['outcome']=='no-match'],
                wrong=[b['query_id'] for a,b in pairs if b['outcome']=='wrong-match'],
                wrong_starts=sum(len(b['wrong_parent_starts']) for a,b in pairs),
                duplicate_starts=sum(b['repeated_parent_starts'] for a,b in pairs)))
    programmes = {r['side']:r['programme'] for r in rows if r['group']=='programme'}
    # Only consumed audio differs with block size; observation/decision content must not.
    def semantic(side):
        r = next(x for x in rows if x['id']=='programme-'+side)
        return [{k:v for k,v in x.items() if k!='consumed'}
                for x in map(json.loads, Path(r['stdout']['path']).read_text().splitlines())
                if x['event'] in ['observation','window','start','end']]
    parity = all(semantic('sequence') == semantic(f'sequence-block{b}') for b in [257,65536])
    assert parity
    summary = dict(comparisons=comparisons, programmes=programmes, block_parity=parity,
                   native_runs=len(rows), limitations='Known recordings; evidence edges are not audible ground truth.')
    e.save(ROOT/'summary.json', summary)
    for row in comparisons:
        print(row)
    for side,p in programmes.items():
        print(side, {k:v for k,v in p.items() if k!='plays'})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prepare', action='store_true')
    parser.add_argument('--run', action='store_true')
    parser.add_argument('--report', action='store_true')
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    if args.prepare: prepare()
    if args.run: run(args.binary.resolve())
    if args.report: report()
