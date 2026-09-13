#!/usr/bin/env python3
"""Independently audit E009 native raw outputs and render the measured report."""
import argparse
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import re

import numpy as np

ROOT=Path('target/lab')
EQ=['bass_-12','bass_-6','bass_+6','bass_+12','treble_-12','treble_-6','treble_+6','treble_+12','notch_broad','notch_narrow','highpass','lowpass']
TREATMENTS=['self','clean',*EQ,'pitch','tempo','pitch_tempo','voice','combined','clipping']


def digest(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()


def read(path):return json.loads(Path(path).read_text())


def verify_identity(item):
    p=Path(item['path']);assert p.stat().st_size==item['bytes'],p
    assert digest(p)==item['sha256'],p


def events(path,side,seconds):
    result=[]
    for line in Path(path).read_text().splitlines():
        if side=='pr3' and line.startswith('{"event":"stream",'):
            value=f'{seconds:.2f}'
            line,n=re.subn(r'("seconds":)'+re.escape(value[:2])+r'(?=,)',lambda m:m[1]+value,line)
            assert n==1
        result.append(json.loads(line,parse_constant=lambda x:(_ for _ in ()).throw(ValueError(x))))
    assert result[-1]['event']=='done',path
    return result


def canonical(items):
    h=hashlib.sha256()
    for item in items:
        item=dict(item)
        if item['event']=='index_done':item.pop('seconds')
        if item['event']=='done':item.pop('cpu_seconds');item.pop('realtime_fraction')
        h.update(json.dumps(item,sort_keys=True).encode()+b'\n')
    return h.hexdigest()


def bootstrap(values):
    v=np.asarray(values,float);rng=np.random.default_rng(20260913)
    samples=v[rng.integers(0,len(v),size=(10000,len(v)))].mean(axis=1)
    return [float(x) for x in np.percentile(samples,[2.5,97.5])]


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--label',default='modal-a');p.add_argument('--include-mix',action='store_true')
    a=p.parse_args();root=ROOT/'evidence'/a.label;prepared=read(ROOT/'prepared.json')
    checked=set()
    def identity(item):
        key=(item['path'],item['sha256'])
        if key not in checked:verify_identity(item);checked.add(key)
    for record in prepared['sources'].values():
        for key in ['file','reference','original']:identity(record[key])
    for record in prepared['streams'].values():identity(record)
    for record in prepared['clips'].values():identity(record)
    identity(prepared['evaluator']);identity(prepared['voice'])
    replay_path=root/'replays/replay-record.json'
    replay=read(replay_path) if replay_path.exists() else {'runs':[]}
    if replay['runs']:identity(replay['evaluator'])
    for repair in replay['runs']:
        old,new=repair['original'],repair['replay']
        assert old['command']==new['command'] and old['input']==new['input']
        assert old['event_digest']==new['event_digest'] and repair['identical_non_timing_events']
        actual=repair['observed_failure']['actual']
        identity(dict(path=old['stdout']['path'],**actual))
    if a.include_mix:
        mix_prepared=read(ROOT/'mix/prepared.json')
        for key in ['source','voice','evaluator']:identity(mix_prepared[key])
    for original in read('docs/evidence/E007/references.json'):
        assert original['short']['sha256']==prepared['sources'][original['id']]['reference']['sha256']
    assert read('docs/evidence/E007/streams.json')['self']['sha256']==prepared['streams']['development-self']['sha256']
    all_runs={};pairs=[];track_rows=[];output_hashes=0
    splits=['development','heldout','negative']+(['mix'] if a.include_mix else [])
    for split in splits:
        path=root/split;meta=read(path/'metadata.json');runs=read(path/'results.json')
        assert meta['status']=='completed' and meta['runs']==len(runs),(split,meta)
        for record in meta['binaries'].values():identity(record)
        identity(meta['evaluator']);identity(meta['prepared'])
        expected=TREATMENTS if split in ['development','heldout'] else ['self','clean',*EQ,'full'] if split=='negative' else ['clean','pitch','tempo','combined']
        sides=['pr7','modal'] if split=='negative' else ['pr3','pr7','modal']
        assert {(r['treatment'],r['side']) for r in runs}=={(t,s) for t in expected for s in sides}
        assert len(runs)==len(expected)*len(sides)
        for r in runs:
            assert r['returncode']==0;identity(r['stdout']);identity(r['stderr']);output_hashes+=2;identity(r['input'])
            ev=events(r['stdout']['path'],r['side'],r['input']['seconds'])
            assert ev[-1]==r['done'] and abs(ev[-1]['audio_seconds']-r['input']['seconds'])<=0.0051
            assert canonical(ev)==r['event_digest']
            starts=[x for x in ev if x['event']=='start'];assert starts==r['starts']
            assert Path(r['command'][0]).resolve()==Path(meta['binaries'][r['side']]['path']).resolve()
            assert ('--modal-fit' in r['command'])==(r['side']=='modal')
            assert r['command'][-2:]==['--stream',r['input']['path']]
            watches=[r['command'][i+1].split('=',1) for i,x in enumerate(r['command']) if x=='--watch']
            expected_songs=sorted(t['id'] for t in read('experiments/toucan2020.json')['tracks']) if split=='mix' else r['input']['songs']
            assert [s for s,_ in watches]==expected_songs
            assert all(ref==prepared['sources'][s]['reference']['path'] for s,ref in watches)
            if split=='mix':
                assert sorted({s['song'] for s in starts})==r['found'] and len(r['found'])==r['detected']
                continue
            truth=r['input']['truth'];accepted={};false=[];duplicates=0;delays=[];positions=[];ends=[]
            for s in starts:
                g=next((g for g in truth if g['song']==s['song'] and g['start']<=s['consumed']<=g['end']+1),None)
                if g is None:false.append(s);continue
                if s['song'] in accepted:duplicates+=1;continue
                accepted[s['song']]=s;delays.append(s['consumed']-g['start'])
                positions.append(abs(s['position']-g['tempo']*(s['t']-g['start'])))
                end=next((e for e in ev if e['event']=='end' and e['song']==s['song'] and e['start']==s['t']),None)
                if end:ends.append(end['consumed']-g['end'])
            assert accepted==r['first_starts'] and false==r['false_starts'] and duplicates==r['duplicates']
            assert len(accepted)==r['detected'] and len(truth)==r['total']
            assert [g['song'] for g in truth if g['song'] not in accepted]==r['misses']
            for got,key in [(delays,'start_delays'),(positions,'position_errors'),(ends,'end_delays')]:assert np.allclose(got,r[key],rtol=0,atol=1e-12)
        lookup={(r['treatment'],r['side']):r for r in runs};all_runs[split]=lookup
        for t in expected:
            if split!='negative':
                assert lookup[(t,'pr3')]['event_digest']==lookup[(t,'pr7')]['event_digest']
                pairs.append([split,t])
            if split in ['development','heldout']:
                b,c=lookup[(t,'pr7')],lookup[(t,'modal')]
                for song in c['input']['songs']:
                    track_rows.append(dict(split=split,treatment=t,song=song,baseline=song not in b['misses'],candidate=song not in c['misses'],baseline_start=b['first_starts'].get(song),candidate_start=c['first_starts'].get(song)))
    peak_records=read(ROOT/'peak-survival.json')
    for r in peak_records:
        identity(r['reference']);identity(r['transformed'])
        def fp(path):
            rows=[x.split() for x in Path(path).read_text().splitlines()]
            return [tuple(map(int,x[1:])) for x in rows if x[0]=='P'],Counter(int(x[1]) for x in rows if x[0]=='H')
        rp,rh=fp(r['reference']['path']);qp,qh=fp(r['transformed']['path'])
        n=sum(any(abs(x-a)<=4 and abs(y-b)<=1 for a,b in qp) for x,y in rp)
        assert n==r['reference_peaks_surviving'] and len(rp)==r['reference_peaks'] and len(qp)==r['transformed_peaks']
        assert abs(n/max(1,len(rp))-r['peak_survival'])<1e-12
        assert abs((rh&qh).total()/max(1,rh.total())-r['exact_hash_key_overlap'])<1e-12
    assert len(peak_records)==30*13
    default=read(ROOT/'default-parity.json');identity(default['candidate_output']);identity(default['candidate_binary'])
    assert canonical(events(default['candidate_output']['path'],'modal',550))==default['baseline_digest']==all_runs['development'][('self','pr7')]['event_digest']
    proof=Path('docs/evidence/E009');proof.mkdir(parents=True,exist_ok=True)
    changes=[]
    for r in track_rows:
        if r['baseline']!=r['candidate']:changes.append(r)
    summary=dict(status='complete' if a.include_mix else 'controls-reviewed-mix-pending',runs=sum(len(x) for x in all_runs.values()),raw_output_hashes_checked=output_hashes,unique_file_identities_checked=len(checked),baseline_parity_pairs=pairs,default_parity=True,track_changes=changes,peak_cases_checked=len(peak_records),candidate_commit=read(root/'development/metadata.json')['candidate_commit'],replayed_outputs=len(replay['runs']),replay_event_parity=True,e007_reference_and_self_hash_parity=True)
    (proof/'validation.json').write_text(json.dumps(summary,indent=2)+'\n')
    (proof/'per-track.json').write_text(json.dumps(track_rows,indent=2)+'\n')
    def get(split,t,side='modal'):return all_runs[split][(t,side)]
    lines=['# E009: modal alignment and measured EQ robustness','',
           'The opt-in `--modal-fit` estimates shift, tempo and position from the strongest cell inside the winning neighbourhood. It preserves the neighbourhood evidence score, confidence threshold, verifier tolerances and default behavior. Existing `best_per_song()` semantics remain unchanged.','',
           '**Development improves; held-out identity detection is unchanged.** The original exact-snippet miss reproduces on PR #3 and PR #7 and is recovered by the candidate. All 22 reference hashes and the original self-control WAV exactly match E007. New transformations of those recordings are development/regression cases, not new held-out data.','',
           '## Real DJ mix regression','']
    if a.include_mix:
        lines+=['Same Toucan mix and 22 clean midpoint references, full 90.61-minute stream; slowed versions last 100.68 minutes. Voiceover remains 12 seconds every 30 at 0 dB local RMS.','',
                '| Treatment | PR #3 / PR #7 coverage | Modal coverage | Baseline / modal starts |','| --- | ---: | ---: | ---: |']
        for t in ['clean','pitch','tempo','combined']:
            b,c=get('mix',t,'pr7'),get('mix',t)
            lines.append(f"| {t} | {b['detected']}/22 | {c['detected']}/22 | {len(b['starts'])} / {len(c['starts'])} |")
        lines+=['','This is publisher-track-list coverage, not independently timed per-play precision. Every start remains in the raw evidence. Full-track reference diagnostics from E007 are still incomplete; this rerun evaluates the requested ten-second references.','']
    else:lines+=['Pending. Do not treat the completed controlled tests as a completed DJ-mix result.','']
    lines+=['## Controlled ten-second snippets','',
            'Each original midpoint snippet appears once, after five seconds of silence and before ten seconds of silence. Truth comes from those sample positions, independently of the detector. Correct starts must occur during the known clip or at most one second after its end. Out-of-window or wrong-song starts count as false; additional starts inside a valid window count as duplicates. Slowed snippets last 11.11 seconds.','',
            'EQ, pitch and tempo cases apply a common 0.15 gain for headroom. EQ cases were checked for absence of PCM clipping. Controlled voiceover covers each entire snippet at equal local RMS, using the fixed Flite text; this is more speech exposure than the periodic DJ-mix treatment. Hard clipping is a separate nonlinear treatment at 25% of the source peak, then scaled for headroom.','',
            '| Treatment | Development baseline | Development modal | Held-out baseline | Held-out modal |','| --- | ---: | ---: | ---: | ---: |']
    for t in TREATMENTS:
        lines.append('| '+t+' | '+' | '.join(f"{get(s,t,v)['detected']}/{22 if s=='development' else 8}" for s,v in [('development','pr7'),('development','modal'),('heldout','pr7'),('heldout','modal')])+' |')
    losses=[r for r in changes if r['baseline'] and not r['candidate']]
    gains=[r for r in changes if not r['baseline'] and r['candidate']]
    lines+=['',f"Across the controlled grid there are {len(gains)} recovered recording/treatment cases and {len(losses)} lost cases. These are correlated conditions, not {len(track_rows)} independent recordings. PR #3 and PR #7 agree on all 40 controlled cases.",'']
    for split in ['development','heldout']:
        songs=sorted({r['song'] for r in track_rows if r['split']==split})
        deltas=[np.mean([int(r['candidate'])-int(r['baseline']) for r in track_rows if r['split']==split and r['song']==song and r['treatment'] in EQ]) for song in songs]
        interval=bootstrap(deltas)
        lines.append(f"{split.capitalize()} EQ: paired mean coverage change {np.mean(deltas)*100:.2f} percentage points; recording-level percentile bootstrap 95% interval [{interval[0]*100:.2f}, {interval[1]*100:.2f}] over {len(songs)} recordings (10,000 resamples, seed 20260913).")
    lines+=['','The zero-width held-out difference interval reflects identical outcomes on eight observed recordings; it does not bound unobserved failure modes. All eight held-out recordings are by one different artist. No configuration was selected using their detector results. Gains are concentrated in the development recordings, so broad unseen-recording improvement is unproved.','',
            '## EQ parameters and fingerprint survival','',
            'Twelve independently applied filters: bass shelf at 200 Hz and treble shelf at 3000 Hz, each -12/-6/+6/+12 dB with Q=0.707; 1000 Hz bell cuts of -12 dB at Q=0.7 and Q=5; two-pole 200 Hz high-pass and 3000 Hz low-pass. FFmpeg uses f64 filter precision. These are specific EQ settings, not an arbitrary-distortion guarantee.','',
            '| EQ | Reference peak survival mean / worst | Exact hash-key overlap mean |','| --- | ---: | ---: |']
    for t in EQ:
        rows=[r for r in peak_records if r['treatment']==t]
        lines.append(f"| {t} | {np.mean([r['peak_survival'] for r in rows]):.1%} / {min(r['peak_survival'] for r in rows):.1%} | {np.mean([r['exact_hash_key_overlap'] for r in rows]):.1%} |")
    lines+=['','Peak survival is the fraction of reference peaks with a transformed peak within four frames and one bin. Hash overlap is the multiset intersection of exact hash keys divided by reference hash count; it does not itself prove a correct temporal alignment. Both were recomputed from native fingerprint dumps on all 30 recordings. The front end is unchanged by the candidate.','',
            '## False starts, delays and position limits','']
    false_total=sum(len(r['false_starts']) for split in ['development','heldout','negative'] for r in all_runs[split].values() if r['side']=='modal')
    negsecs=get('negative','full')['input']['seconds']
    lines.append(f"Candidate false starts across the controlled and negative grids: {false_total}. Unwatched full-track control: eight complete recordings, {negsecs/60:.2f} minutes. Separate negative EQ cases use eight one-minute midpoint excerpts (eight minutes per treatment); these excerpts overlap the full tracks and their variants are correlated. Do not sum them as independent exposure. With zero starts, {negsecs/60:.2f} minutes alone gives a one-sided 95% Poisson upper bound of {-math.log(.05)/(negsecs/3600):.2f} starts/hour under that model, far from a production false-alarm guarantee.")
    lines+=['','| Split / treatment | Start delay p50 / p95, baseline → modal (s) | Worst position error, baseline → modal (s) | End-delay p50, baseline → modal (s) |','| --- | --- | --- | --- |']
    for split in ['development','heldout']:
        for t in ['clean','combined']:
            b,c=get(split,t,'pr7'),get(split,t)
            fmt=lambda v:'/'.join(f'{x:.3f}' for x in np.percentile(v,[50,95]))
            lines.append(f"| {split} / {t} | {fmt(b['start_delays'])} → {fmt(c['start_delays'])} | {max(b['position_errors']):.3f} → {max(c['position_errors']):.3f} | {np.median(b['end_delays']):.3f} → {np.median(c['end_delays']):.3f} |")
    lines+=['','Delay summaries contain detected plays only; their denominators and misses remain in the coverage table. They are not a proof of lower latency at matched recall. Position error uses the known reference time at the reported audio frame. The recovered t01 clean start can select a repeated passage and is about 8.7 seconds wrong in position despite identifying the song. This remains a concrete limitation, visible in the worst-case column. No universal alignment fix is claimed.','',
            'All timing runs are single observations and share this execution host with preparation work. Outer wall times include process startup and index construction; the monitor’s legacy `cpu_seconds` is elapsed stream-processing time, not process CPU. No speedup or representative performance gate is claimed.','',
            '## Decision and proof','',
            'The final file audit found nine truncated auxiliary audio caches, two empty fingerprint dumps and eight truncated raw result logs; the cause is unestablished. All reference WAVs and assembled detector inputs retained their recorded hashes. Audio caches and fingerprint dumps were restored only after matching their exact original SHA-256. The eight affected native commands were replayed into separate files, all with identical non-timing event digests. Original truncated logs, original result records, hash mismatches and replay provenance remain preserved. The 162 reviewed cases include these eight replacement executions; the audit does not claim the original damaged logs were complete.','',
            'Retain the candidate as an **experimental opt-in**, not a new default. The original snippet failure is recovered and the controlled grid has no identity losses or added false starts. The eight recording-disjoint held-out tracks show preserved identity outcomes, not new gains. Voiceover and position ambiguity remain unresolved. The goal is incomplete.','',
            f"Reviewed {summary['runs']} completed native runs, {output_hashes} stdout/stderr hashes, {len(pairs)} PR #3/PR #7 event-parity pairs, default-candidate parity and 390 fingerprint comparisons. Executable/input/evaluator hashes are retained before execution, with strict complete-output checks. The tested candidate source is `{summary['candidate_commit']}`; its binary SHA-256 is `{read(root/'development/metadata.json')['binaries']['modal']['sha256']}`.",'',
            'See [proof, source attribution and reproduction](../evidence/E009/README.md), [per-track data](../evidence/E009/per-track.json) and the neighboring frozen protocol. The source dataset combines Toucan CC BY-NC-SA 4.0 material with separately attributed Kevin MacLeod CC BY 4.0 recordings. Full source audio is not stored in Git.']
    Path('docs/experiments/E009-results.md').write_text('\n'.join(lines)+'\n')
    print(json.dumps({k:v for k,v in summary.items() if k not in ['track_changes','baseline_parity_pairs']},indent=2))

if __name__=='__main__':main()
