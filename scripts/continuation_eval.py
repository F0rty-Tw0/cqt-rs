#!/usr/bin/env python3
"""E014 frozen inputs, resumable paired native execution and raw-event scoring."""
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
import zipfile

import numpy as np
import real_mix_eval as e
import sequence_eval as seq
from song_index_eval import outcome, validate_events
from song_index_prepare import atomic_bytes, pcm_wav
from continuation_inputs import recover

ROOT = Path('target/continuation')
RATE = 44100
ARMS = {
    'legacy': ['--modal-fit'],
    'long': ['--modal-fit', '--window', '10', '--report', '2'],
    'continuation': ['--modal-fit', '--window', '10', '--continuation-seconds', '2', '--continuation-hypotheses', '3'],
    'single': ['--modal-fit', '--window', '10', '--continuation-seconds', '2', '--continuation-hypotheses', '1'],
}
TREATMENTS = {
    'gain': 'volume=0.15',
    'pitch_minus2': f'volume=0.15,rubberband=pitch={2**(-2/12):.15f}',
    'pitch_plus2': f'volume=0.15,rubberband=pitch={2**(2/12):.15f}',
    'pitch_plus_half': f'volume=0.15,rubberband=pitch={2**(.5/12):.15f}',
    'tempo_088': 'volume=0.15,rubberband=tempo=0.88',
    'tempo_112': 'volume=0.15,rubberband=tempo=1.12',
    'bass_cut12': 'volume=0.15,bass=f=200:t=q:w=0.707:g=-12:p=2:r=f64',
    'treble_cut12': 'volume=0.15,treble=f=3000:t=q:w=0.707:g=-12:p=2:r=f64',
    'noise_10db': 'volume=0.15',
    'noise_0db': 'volume=0.15',
    'voice_0db': 'volume=0.15',
    'combined': f'volume=0.15,rubberband=tempo=0.88:pitch={2**(2/12):.15f},bass=f=200:t=q:w=0.707:g=-12:p=2:r=f64',
}


def read(path):
    return json.loads(Path(path).read_text())


def save(path, data):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    e.save(path, data)


def pcm(path):
    with wave.open(str(path)) as w:
        assert (w.getnchannels(), w.getsampwidth(), w.getframerate()) == (1, 2, RATE)
        return np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').copy()


def prepare():
    destination = ROOT/'manifest.json'
    if destination.exists():
        return read(destination)
    m = read(ROOT/'recovered.json') if (ROOT/'recovered.json').exists() else recover()
    for r in m['queries'] + m['references']['full'] + [m['mix']]:
        assert e.sha(r['path']) == r['sha256'], r['path']
    save(ROOT/'e011/manifest.json', m)
    seq.ROOT = ROOT/'programme'
    seq.OLD = ROOT/'e011'
    if not (seq.ROOT/'manifest.json').exists():
        seq.prepare()
    programme = read(seq.ROOT/'manifest.json')['programme']
    shifted = ROOT/'programme-phase.wav'
    pcm_wav(shifted, bytes(2*RATE) + pcm(programme['path']).tobytes())
    truth = [dict(p, start=p['start']+1, end=p['end']+1,
                  start_sample=p['start_sample']+RATE, end_sample=p['end_sample']+RATE)
             for p in programme['intervals']]
    manifest = dict(references=m['references']['full'], queries=m['queries'],
                    programme=dict(programme, id='programme', group='programme', song=None),
                    phase=dict(**e.identity(shifted), **e.wav_info(shifted), intervals=truth,
                               id='phase', group='programme', song=None),
                    mix=dict(m['mix'], id='full-mix', group='full-mix', song=None),
                    arms=ARMS, protocol=e.identity('docs/experiments/E014-long-context.md'),
                    preparation=e.identity(__file__), frozen_source=e.identity(ROOT/'frozen-e011-manifest.json'))
    save(destination, manifest)
    print('Prepared 96 exact queries, 25-play programme, shifted programme and full mix', flush=True)
    return manifest


def prepare_robustness():
    destination = ROOT/'robustness.json'
    if destination.exists():
        return read(destination)
    m = prepare()
    output = ROOT/'robustness'
    output.mkdir(exist_ok=True)
    voice_path = output/'voice.wav'
    voice_text = Path('docs/evidence/E007/voice.txt').resolve()
    voice_command = ['ffmpeg', '-v', 'error', '-nostdin', '-y', '-f', 'lavfi', '-i',
                     f'flite=textfile={voice_text}:voice=slt', '-ac', '1', '-ar', str(RATE),
                     '-c:a', 'pcm_s16le', str(voice_path)]
    subprocess.run(voice_command, check=True)
    voice = pcm(voice_path).astype(float)/32768
    clips = sorted((q for q in m['queries'] if q['group']=='clean'), key=lambda q:q['id'])

    def treatment(name, filt):
        parts, truth, logs, cursor = [], [], [], 0
        for q in clips:
            command = ['ffmpeg', '-v', 'error', '-nostdin', '-threads', '1', '-i', q['path'],
                       '-af', filt, '-ac', '1', '-ar', str(RATE), '-f', 's16le', '-']
            result = subprocess.run(command, capture_output=True, check=True)
            music = np.frombuffer(result.stdout, dtype='<i2').astype(float)/32768
            rms = float(np.sqrt(np.mean(music**2)))
            data = music.copy()
            seed = int.from_bytes(hashlib.sha256(('E014:noise:'+name+':'+q['id']).encode()).digest()[:8], 'little')
            additions = {}
            if name.startswith('noise') or name == 'combined':
                snr = 0 if name == 'noise_0db' else 10
                noise = np.random.default_rng(seed).normal(size=len(music))
                noise *= rms / max(float(np.sqrt(np.mean(noise**2))), 1e-12) / 10**(snr/20)
                data += noise
                additions['noise'] = dict(seed=seed, target_music_snr_db=snr,
                                           rms=float(np.sqrt(np.mean(noise**2))))
            if name in ('voice_0db', 'combined'):
                speech = np.resize(voice, len(music))
                speech = speech * rms / max(float(np.sqrt(np.mean(speech**2))), 1e-12)
                data += speech
                additions['voice'] = dict(target_speech_music_db=0, rms=float(np.sqrt(np.mean(speech**2))))
            scale = min(1.0, 0.95/max(float(np.max(np.abs(data))), 1e-12))
            data = np.rint(data*scale*32768).astype('<i2')
            assert np.max(np.abs(data.astype(np.int32))) < 32767
            parts.append(bytes(7*RATE*2)); cursor += 7*RATE
            tempo = .88 if name in ('tempo_088', 'combined') else 1.12 if name == 'tempo_112' else 1.0
            truth.append(dict(play_id=q['id'], song=q['song'], start=cursor/RATE,
                              end=(cursor+len(data))/RATE, source_start=q['start_seconds'],
                              tempo=tempo, treatment=name))
            parts.append(data.tobytes()); cursor += len(data)
            parts.append(bytes(9*RATE*2)); cursor += 9*RATE
            logs.append(dict(query=q['id'], input=e.identity(q['path']), command=command,
                             returncode=result.returncode, stderr=result.stderr.decode(),
                             music_rms=rms, common_headroom_gain=scale, additions=additions,
                             rendered_pcm_sha256=hashlib.sha256(data.tobytes()).hexdigest()))
        path = output/(name+'.wav')
        pcm_wav(path, b''.join(parts))
        row = dict(**e.identity(path), **e.wav_info(path), intervals=truth, transforms=logs,
                   id=name, group='robustness', song=None)
        save(output/(name+'.json'), row)
        print('Rendered', name, flush=True)
        return row

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        jobs = [pool.submit(treatment, name, filt) for name, filt in TREATMENTS.items()]
        cases = [f.result() for f in jobs]
    result = dict(cases=cases, treatments=TREATMENTS, voice=e.identity(voice_path), voice_command=voice_command,
                  text=e.identity(voice_text), ffmpeg=subprocess.run(['ffmpeg', '-version'], capture_output=True,
                  check=True, text=True).stdout, preparer=e.identity(__file__))
    save(destination, result)
    return result


def validate_resume(row, signature):
    # JSON has arrays, so normalize in-memory tuple fields before comparison.
    signature = json.loads(json.dumps(signature, allow_nan=False))
    if row.get('signature') != signature or row.get('status') != 'completed':
        raise ValueError('Refusing mismatched or incomplete cached run')
    for name in ('stdout', 'stderr'):
        if e.sha(row[name]['path']) != row[name]['sha256']:
            raise ValueError('Cached native output changed')


def run_one(binary, m, q, arm, extra=(), suffix='', live=False):
    key = q['id']+'-'+arm+suffix
    output = ROOT/'runs'
    output.mkdir(exist_ok=True)
    command = [str(binary)]
    for r in m['references']:
        command += ['--watch', r['id']+'='+r['path']]
    command += ['--stream', '-' if live else q['path'], *ARMS[arm], *extra]
    signature = dict(binary_sha256=e.sha(binary), input_sha256=e.sha(q['path']), command=command,
                     references=[(r['id'], r['sha256']) for r in m['references']],
                     evaluator=e.sha(__file__), helpers=[e.sha(p) for p in
                     ['scripts/sequence_eval.py', 'scripts/song_index_eval.py', 'scripts/chunk_query_eval.py']])
    assert signature['input_sha256'] == q['sha256']
    result_path = output/(key+'.result.json')
    if result_path.exists():
        row = read(result_path); validate_resume(row, signature); return row
    row = dict(id=key, query_id=q['id'], arm=arm, group=q['group'], input=q, signature=signature,
               command=command, source_commit=(binary.parent/'candidate-commit.txt').read_text().strip(),
               status='running')
    save(output/(key+'.active.json'), row)
    stdout, stderr = output/(key+'.jsonl'), output/(key+'.stderr.txt')
    start = time.perf_counter()
    try:
        with stdout.open('wb') as out, stderr.open('wb') as err:
            p = subprocess.run(command, input=pcm(q['path']).tobytes() if live else None,
                               stdout=out, stderr=err, timeout=900,
                               env=dict(os.environ, RAYON_NUM_THREADS='1'))
        row.update(returncode=p.returncode, elapsed_seconds=time.perf_counter()-start,
                   stdout=e.identity(stdout), stderr=e.identity(stderr))
        assert p.returncode == 0, (p.returncode, stderr.read_text()[-1000:])
        events = [json.loads(line) for line in stdout.read_text().splitlines()]
        assert events[-1]['event']=='done'
        assert abs(events[-1]['audio_seconds']-q['seconds']) < .006
        refs = {r['id']:r for r in m['references']}
        assert {x['song'] for x in events if x['event']=='index'} == set(refs)
        if q['group'] in ('programme', 'robustness'):
            row['score'] = seq.programme_score(events, q['intervals'])
        elif q['group'] == 'full-mix':
            row['starts'] = [x for x in events if x['event']=='start']
            row['ends'] = [x for x in events if x['event']=='end']
        else:
            validate_events(events, refs, q)
            row.update(outcome(events, refs, q['song']))
        windows = [x for x in events if x['event']=='window']
        if windows:
            assert sum(w['hashes'] for w in windows) == events[-1]['lookups']
            assert sum(w['query_peaks'] for w in windows) == events[-1]['peaks']
            assert windows[0]['begin'] == 0
            assert all(a['t']==b['begin'] for a,b in zip(windows,windows[1:]))
        row.update(status='completed', done=events[-1])
    except Exception as ex:
        row.update(status='failed', error=repr(ex), elapsed_seconds=time.perf_counter()-start)
        for name, path in [('stdout', stdout), ('stderr', stderr)]:
            if path.exists(): row[name]=e.identity(path)
    save(result_path, row)
    (output/(key+'.active.json')).unlink()
    print(key, row['status'], row.get('outcome', row.get('score', {}).get('detected', '')),
          round(row['elapsed_seconds'], 2), flush=True)
    return row


def semantic(path, consumed=True):
    fields = ('observation', 'window', 'start', 'end', 'report')
    rows = [x for x in map(json.loads, Path(path).read_text().splitlines()) if x['event'] in fields]
    return [{k:v for k,v in row.items() if consumed or k!='consumed'} for row in rows]


def run(binary, stage):
    m = prepare()
    for r in m['references']:
        assert e.sha(r['path']) == r['sha256']
    jobs = []
    if stage == 'queries':
        jobs = [(q, arm) for q in m['queries'] for arm in ('long', 'continuation')]
    elif stage == 'programme':
        jobs = [(m['programme'], a) for a in ARMS]
        jobs += [(m['phase'], a) for a in ('long','continuation')]
    elif stage == 'full-mix':
        jobs = [(m['mix'], a) for a in ('long','continuation','single')]
    elif stage == 'robustness':
        jobs = [(q, a) for q in prepare_robustness()['cases'] for a in ('long','continuation')]
    elif stage == 'parity':
        baseline = (ROOT/'baseline/candidate').resolve()
        clean = next(q for q in m['queries'] if q['id']=='clean-t01')
        for q in [clean, m['programme']]:
            old=run_one(baseline, m, q, 'legacy', suffix='-baseline')
            new=run_one(binary, m, q, 'legacy')
            assert semantic(old['stdout']['path']) == semantic(new['stdout']['path'])
        standard = run_one(binary, m, m['programme'], 'continuation')
        for size in (257,65536):
            block=run_one(binary,m,m['programme'],'continuation',['--block',str(size)],f'-block{size}')
            assert semantic(standard['stdout']['path'],False)==semantic(block['stdout']['path'],False)
        file=run_one(binary,m,clean,'continuation')
        live=run_one(binary,m,clean,'continuation',suffix='-live',live=True)
        assert semantic(file['stdout']['path'])==semantic(live['stdout']['path'])
        save(ROOT/'parity.json',dict(status='passed',baseline_source=old['source_commit'],
                                   source=new['source_commit'],binary=e.identity(binary)))
        return
    workers = 2 if stage=='full-mix' else 4
    metadata = dict(binary=e.identity(binary), source_commit=(binary.parent/'candidate-commit.txt').read_text().strip(),
                    evaluator=e.identity(__file__), manifest=e.identity(ROOT/'manifest.json'), workers=workers,
                    threads=1, python=sys.version, platform=platform.platform(), stage=stage)
    save(ROOT/(stage+'-metadata.json'),metadata)
    with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
        futures=[pool.submit(run_one,binary,m,*j) for j in jobs]
        rows=[f.result() for f in concurrent.futures.as_completed(futures)]
    save(ROOT/(stage+'-results.json'), sorted(rows,key=lambda r:r['id']))
    assert all(r['status']=='completed' for r in rows)


def report():
    m = prepare(); refs={r['id']:r for r in m['references']}
    # Recompute from raw JSONL, not cached scores.
    rows=[]
    for path in sorted((ROOT/'runs').glob('*.result.json')):
        r=read(path); validate_resume(r,r['signature'])
        events=list(map(json.loads,Path(r['stdout']['path']).read_text().splitlines()))
        q=r['input']; row=dict(id=r['id'],group=q['group'],arm=r['arm'],query=q['id'])
        if q['group'] in ('programme','robustness'):
            row['score']=seq.programme_score(events,q['intervals'])
        elif q['group']=='full-mix':
            starts=[x for x in events if x['event']=='start']; ends=[x for x in events if x['event']=='end']
            row.update(songs=sorted({s['song'] for s in starts}),starts=starts,ends=ends)
        else:
            row.update(outcome(events,refs,q['song']))
        rows.append(row)
    save(ROOT/'recount.json',rows)
    table=[]
    for group in ('exact','clean','frozen','mix','negative'):
        for arm in ('long','continuation'):
            selected=[r for r in rows if r['group']==group and r['arm']==arm and r['id']==r['query']+'-'+arm]
            table.append(dict(group=group,arm=arm,total=len(selected),
                         correct=sum(r['outcome'] in ('correct','correct-rejection') for r in selected),
                         misses=[r['query'] for r in selected if r['outcome']=='no-match'],
                         wrong=sum(len(r['wrong_parent_starts']) for r in selected),
                         duplicates=sum(r['repeated_parent_starts'] for r in selected)))
    save(ROOT/'summary.json',dict(table=table,runs=len(rows)))
    print(json.dumps(dict(table=table,runs=len(rows)),indent=2))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action',choices=['prepare','prepare-robustness','run','report'])
    parser.add_argument('--stage',choices=['queries','programme','full-mix','robustness','parity'],default='queries')
    parser.add_argument('--binary',type=Path,default=ROOT/'candidate-v3/candidate')
    args=parser.parse_args()
    if args.action=='prepare':prepare()
    elif args.action=='prepare-robustness':prepare_robustness()
    elif args.action=='run':run(args.binary.resolve(),args.stage)
    else:report()

if __name__=='__main__':main()
