#!/usr/bin/env python3
"""E016 frozen paired execution; labels are used only after native recognition."""
import concurrent.futures
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

import numpy as np
import continuation_eval as c
import continuation_report as recount
import real_mix_eval as e
import sequence_eval as seq
from song_index_eval import outcome, validate_events

ROOT = Path('target/e016')
BASELINE = '25435ea781315b247a0a4331bd4be95f23309fe8'
FLAGS = ['--modal-fit', '--window', '10', '--continuation-seconds', '2',
         '--continuation-hypotheses', '3']
ARMS = {'baseline': FLAGS, 'pair': FLAGS + ['--pair-fallback']}
HELPERS = ['scripts/continuation_eval.py', 'scripts/continuation_report.py',
           'scripts/sequence_eval.py', 'scripts/song_index_eval.py',
           'scripts/chunk_query_eval.py', 'scripts/real_mix_eval.py']


def save(path, obj):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + '.tmp')
    temp.write_text(json.dumps(obj, indent=2, allow_nan=False) + '\n')
    temp.replace(path)


def inputs():
    m = c.read('target/continuation/manifest.json')
    r = c.read('target/continuation/robustness.json')
    old = json.loads(gzip.open('docs/evidence/E014/manifest.json.gz', 'rt').read())
    old_r = json.loads(gzip.open('docs/evidence/E014/robustness.json.gz', 'rt').read())
    expected = {q['id']: q['sha256'] for q in old['queries'] + old['references']}
    expected.update({old[k]['id']: old[k]['sha256'] for k in ('programme', 'phase', 'mix')})
    expected.update({q['id']: q['sha256'] for q in old_r['cases']})
    all_inputs = m['queries'] + m['references'] + [m[k] for k in ('programme', 'phase', 'mix')] + r['cases']
    for q in all_inputs:
        assert q['sha256'] == expected[q['id']] == e.sha(q['path']), q['id']
    return m, r


def budget(events):
    rows = [r for r in events if r['event'] == 'pair_budget']
    for r in rows:
        assert r['peaks'] <= 512 and r['pairs'] <= 4096
        assert r['votes'] <= 262144 and r['fits'] <= 64 and r['candidates'] <= 8
    return dict(windows=len(rows), totals={k: sum(r[k] for r in rows) for k in
                ('pairs', 'occurrences', 'collision_ranges', 'collision_entries', 'votes', 'fits', 'candidates')},
                peaks={k: max((r[k] for r in rows), default=0) for k in
                ('peaks', 'pairs', 'votes', 'cells', 'fits', 'candidates')},
                saturated={k: sum(r[k] for r in rows) for k in ('peak_overflow', 'pair_limit', 'vote_limit')},
                pair_proposals=sum(r['event']=='observation' and r.get('origin')=='pair_retrieval' for r in events),
                pair_verified=sum(r['event']=='observation' and r.get('origin')=='pair_retrieval'
                                  and r['query_peaks']>0 and r['query_matched']>=.4*r['query_peaks'] for r in events))


def run_one(m, q, arm, extra=(), suffix='', live=False, legacy=False, candidate_default=False):
    side = 'candidate' if arm == 'pair' or candidate_default else 'baseline'
    binary = (ROOT/side/'monitor').resolve()
    command = [str(binary)]
    for ref in m['references']:
        command += ['--watch', ref['id']+'='+ref['path']]
    command += ['--stream', '-' if live else q['path']]
    command += ['--modal-fit'] if legacy else ARMS[arm]
    command += list(extra)
    key = q['id']+'-'+arm+suffix
    source = (binary.parent/'commit.txt').read_text().strip()
    signature = dict(binary=e.identity(binary), source=source, input_sha256=e.sha(q['path']),
                     command=command, references=[(r['id'],r['sha256']) for r in m['references']],
                     evaluator=e.identity(__file__), helpers=[e.identity(p) for p in HELPERS],
                     protocol=e.identity('docs/experiments/E016-pair-protocol.md'))
    signature = json.loads(json.dumps(signature))
    path = ROOT/'runs'/(key+'.json')
    if path.exists():
        row = c.read(path)
        c.validate_resume(row, signature)
        return row
    path.parent.mkdir(parents=True, exist_ok=True)
    stdout, stderr = path.with_suffix('.jsonl'), path.with_suffix('.stderr')
    row = dict(id=key, query_id=q['id'], group=q['group'], arm=arm,
               source=source, signature=signature, input=q, status='running')
    save(path.with_suffix('.active.json'), row)
    started = time.perf_counter()
    try:
        with stdout.open('wb') as out, stderr.open('wb') as err:
            process = subprocess.run(command, stdout=out, stderr=err,
                         input=c.pcm(q['path']).tobytes() if live else None,
                         timeout=1800 if q['group']=='full-mix' else 900,
                         env=dict(os.environ, RAYON_NUM_THREADS='1'))
        row['returncode'] = process.returncode
        assert process.returncode == 0, (key, stderr.read_text()[-2000:])
        events = list(map(json.loads, stdout.read_text().splitlines()))
        assert events[-1]['event']=='done'
        assert abs(events[-1]['audio_seconds']-q['seconds']) < .006
        refs = {r['id']:r for r in m['references']}
        assert {x['song'] for x in events if x['event']=='index'} == set(refs)
        if q['group'] in ('programme','robustness'):
            row['score'] = seq.programme_score(events, q['intervals'])
        elif q['group']=='full-mix':
            row['starts'] = [x for x in events if x['event']=='start']
        else:
            validate_events(events, refs, q)
            row.update(outcome(events, refs, q['song']))
        windows = [x for x in events if x['event']=='window']
        if not legacy:
            assert sum(w['hashes'] for w in windows)==events[-1]['lookups']
            assert sum(w['query_peaks'] for w in windows)==events[-1]['peaks']
            assert windows[0]['begin']==0
            assert all(a['t']==b['begin'] for a,b in zip(windows,windows[1:]))
        row.update(status='completed', done=events[-1], budget=budget(events))
    except Exception as ex:
        row.update(status='failed', error=repr(ex))
    row.update(elapsed_seconds=time.perf_counter()-started,
               stdout=e.identity(stdout), stderr=e.identity(stderr))
    save(path, row)
    path.with_suffix('.active.json').unlink()
    print(key, row['status'], row.get('outcome', row.get('score',{}).get('detected','')),
          round(row['elapsed_seconds'],2), flush=True)
    return row


def run():
    m,r = inputs()
    save(ROOT/'inputs.json', dict(manifest=m, robustness=r))
    # Fixed one-configuration test; no adaptive selection from outcomes.
    cases = m['queries'] + [m['programme'],m['phase']] + r['cases'] + [m['mix']]
    assert len(cases)==111
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        futures = [pool.submit(run_one,m,q,a) for q in cases for a in ARMS]
        rows = [f.result() for f in futures]
    assert all(r['status']=='completed' for r in rows)
    for q in [next(q for q in m['queries'] if q['id']=='clean-t01'),m['programme']]:
        old = run_one(m,q,'baseline',suffix='-legacy',legacy=True)
        new = run_one(m,q,'baseline',suffix='-candidate-default',legacy=True,candidate_default=True)
        assert c.semantic(old['stdout']['path'])==c.semantic(new['stdout']['path'])
    standard = run_one(m,m['programme'],'pair')
    for size in (257,65536):
        row = run_one(m,m['programme'],'pair',extra=['--block',str(size)],suffix=f'-block{size}')
        assert c.semantic(standard['stdout']['path'],False)==c.semantic(row['stdout']['path'],False)
    live = run_one(m,m['programme'],'pair',suffix='-live',live=True)
    assert c.semantic(standard['stdout']['path'])==c.semantic(live['stdout']['path'])
    save(ROOT/'parity.json',dict(status='passed',runs=229))


def report():
    m,r = inputs()
    cases = m['queries'] + [m['programme'],m['phase']] + r['cases'] + [m['mix']]
    results = {}; raw = {}
    for q in cases:
        for arm in ARMS:
            key = q['id']+'-'+arm
            row = c.read(ROOT/'runs'/(key+'.json'))
            c.validate_resume(row,row['signature'])
            events = list(map(json.loads,Path(row['stdout']['path']).read_text().splitlines()))
            raw[key]=events
            result = dict(id=key,query_id=q['id'],arm=arm,group=q['group'],budget=budget(events))
            if q['group'] in ('programme','robustness'):
                result['score']=recount.controlled(events,q['intervals'])
                for k in ('total','detected','duplicate_starts','premature_endings'):
                    assert row['score'][k]==result['score'][k],(key,k)
            elif q['group']=='full-mix':
                result['starts']=[x for x in events if x['event']=='start']
                result['songs']=sorted({x['song'] for x in result['starts']})
            else:
                result.update(outcome(events,{x['id']:x for x in m['references']},q['song']))
            results[key]=result
    groups={}
    for group in recount.GROUPS:
        sides=[{x['query_id']:x for x in results.values() if x['group']==group and x['arm']==arm} for arm in ARMS]
        groups[group]=recount.paired(*sides)
    treatments={}
    for q in [m['programme'],m['phase']]+r['cases']:
        old,new=[results[q['id']+'-'+arm]['score'] for arm in ARMS]
        a={p['play_id']:p for p in old['plays']};b={p['play_id']:p for p in new['plays']}
        diff=np.array([int(b[k]['detected'])-int(a[k]['detected']) for k in a])
        boot=np.random.default_rng(1602026).choice(diff,(10000,len(diff))).mean(axis=1)
        treatments[q['id']]=dict(before=old['detected'],after=new['detected'],
            gains=[k for k in a if not a[k]['detected'] and b[k]['detected']],
            losses=[k for k in a if a[k]['detected'] and not b[k]['detected']],
            false_before=len(old['false_starts']),false_after=len(new['false_starts']),
            duplicates_before=old['duplicate_starts'],duplicates_after=new['duplicate_starts'],
            premature_before=old['premature_endings'],premature_after=new['premature_endings'],
            bootstrap=dict(delta=float(diff.mean()),low=float(np.quantile(boot,.025)),high=float(np.quantile(boot,.975))))
    accepted=treatments['noise_0db']['after']>8
    accepted &= all(not t['losses'] and t['false_after']<=t['false_before'] for t in treatments.values())
    accepted &= all(not g['losses'] and g['wrong_after']<=g['wrong_before'] for g in groups.values())
    accepted &= all(treatments[k]['duplicates_after']<=treatments[k]['duplicates_before']
                    and treatments[k]['premature_after']<=treatments[k]['premature_before'] for k in ('programme','phase'))
    save(ROOT/'report.json',dict(status='measured',quality_gate=bool(accepted),groups=groups,
                                 treatments=treatments,results=results,
                                 scope='Development recordings and synthetic speech; CI and parity are separate gates.'))
    print(json.dumps(dict(groups=groups,treatments=treatments,quality_gate=bool(accepted)),indent=2))


if __name__=='__main__':
    {'run':run,'report':report}[sys.argv[1]]()
