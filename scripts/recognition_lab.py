#!/usr/bin/env python3
"""Frozen E008/E009 audio cases, paired native runs and independently timed truth."""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import wave

import numpy as np
import real_mix_eval as common

RATE = 44100
ROOT = Path('target/lab')
VARIANTS = {'self': '', 'clean': 'volume=0.15'}
for band, frequency in [('bass', 200), ('treble', 3000)]:
    for gain in [-12, -6, 6, 12]:
        VARIANTS[f'{band}_{gain:+d}'] = f'volume=0.15,{band}=f={frequency}:t=q:w=0.707:g={gain}:p=2:r=f64'
VARIANTS.update({
    'notch_broad': 'volume=0.15,equalizer=f=1000:t=q:w=0.7:g=-12:r=f64',
    'notch_narrow': 'volume=0.15,equalizer=f=1000:t=q:w=5:g=-12:r=f64',
    'highpass': 'volume=0.15,highpass=f=200:p=2:r=f64',
    'lowpass': 'volume=0.15,lowpass=f=3000:p=2:r=f64',
    'pitch': 'volume=0.15,rubberband=pitch=1.122462048309373',
    'tempo': 'volume=0.15,rubberband=tempo=0.9',
    'pitch_tempo': 'volume=0.15,rubberband=tempo=0.9:pitch=1.122462048309373',
    'voice': 'voice', 'combined': 'combined', 'clipping': 'clipping',
})
EQ = [k for k in VARIANTS if k.startswith(('bass_', 'treble_', 'notch_')) or k in ('highpass','lowpass')]


def pcm(path):
    with wave.open(str(path)) as f:
        assert f.getnchannels() == 1 and f.getframerate() == RATE and f.getsampwidth() == 2
        return np.frombuffer(f.readframes(f.getnframes()), dtype='<i2').copy()


def write_pcm(path, data):
    with common.wav_writer(path) as f:
        f.writeframes(data.astype('<i2').tobytes())


def prepare():
    sources = json.loads((ROOT/'sources.json').read_text())
    output = ROOT/'prepared'; output.mkdir(exist_ok=True)
    log, manifest = [], {'variants': VARIANTS, 'sources': sources, 'streams': {}, 'clips': {}, 'voiceover': {}}
    voice = output/'voice.wav'
    text = Path('docs/evidence/E007/voice.txt').resolve()
    common.ffmpeg(['-f','lavfi','-i',f'flite=textfile={text}:voice=slt','-ac','1','-ar',str(RATE),'-c:a','pcm_s16le',voice],log)
    for song, record in sorted(sources.items()):
        if song.startswith('n'):
            continue
        original = Path(record['reference']['path'])
        for name, filt in VARIANTS.items():
            path = output/f'{song}-{name}.wav'
            if name == 'self':
                write_pcm(path,pcm(original))
            elif name in ('voice','combined'):
                base = output/f"{song}-{'clean' if name=='voice' else 'pitch_tempo'}.wav"
                manifest['voiceover'][f'{song}-{name}'] = common.voice_mix(base,voice,path)
            elif name == 'clipping':
                data = pcm(original).astype(float)/32768
                # Hard clipping is a separate nonlinear treatment, at 25% of original peak.
                limit = max(float(np.max(np.abs(data)))*0.25, 1e-6)
                write_pcm(path,np.rint(np.clip(data,-limit,limit)*0.15/limit*32767))
            else:
                common.ffmpeg(['-i',original,'-af',filt,'-c:a','pcm_s16le',path],log)
            data=pcm(path)
            if name not in ('self','clipping'):
                assert int(np.max(np.abs(data.astype(np.int32)))) < 32767, (song,name,'unexpected clipping')
            manifest['clips'][f'{song}-{name}'] = dict(**common.identity(path), **common.wav_info(path))
        print('Rendered',song,flush=True)
    for split,prefix in [('development','t'),('heldout','h')]:
        songs=sorted(k for k in sources if k.startswith(prefix))
        for name in VARIANTS:
            path=output/f'{split}-{name}.wav'; truth=[];cursor=0
            with common.wav_writer(path) as f:
                for song in songs:
                    f.writeframes(bytes(5*RATE*2));cursor+=5*RATE
                    data=pcm(output/f'{song}-{name}.wav');start=cursor/RATE
                    f.writeframes(data.tobytes());cursor+=len(data)
                    truth.append(dict(song=song,start=start,end=cursor/RATE,tempo=0.9 if name in ('tempo','pitch_tempo','combined') else 1.0))
                    f.writeframes(bytes(10*RATE*2));cursor+=10*RATE
            manifest['streams'][f'{split}-{name}']=dict(**common.identity(path),**common.wav_info(path),truth=truth,songs=songs)
    # Eight frozen, unwatched recordings: one 60-second midpoint per recording.
    negative=output/'negative-self.wav'
    with common.wav_writer(negative) as f:
        for song in sorted(k for k in sources if k.startswith('n')):
            data=pcm(sources[song]['original']['path']);start=(len(data)-60*RATE)//2
            assert start>=0;f.writeframes(data[start:start+60*RATE].tobytes())
    for name in ['self','clean',*EQ]:
        path=output/f'negative-{name}.wav'
        if name!='self':common.ffmpeg(['-i',negative,'-af',VARIANTS[name],'-c:a','pcm_s16le',path],log)
        manifest['streams'][f'negative-{name}']=dict(**common.identity(path),**common.wav_info(path),truth=[],songs=sorted(k for k in sources if not k.startswith('n')))
    # Longer untreated music control, with all eight complete originals.
    path=output/'negative-full.wav'
    with common.wav_writer(path) as f:
        for song in sorted(k for k in sources if k.startswith('n')):f.writeframes(pcm(sources[song]['original']['path']).tobytes())
    manifest['streams']['negative-full']=dict(**common.identity(path),**common.wav_info(path),truth=[],songs=sorted(k for k in sources if not k.startswith('n')))
    manifest.update(evaluator=common.identity(__file__),commands=log,voice=common.identity(voice),ffmpeg=subprocess.run(['ffmpeg','-version'],capture_output=True,text=True,check=True).stdout)
    common.save(ROOT/'prepared.json',manifest)
    print('Prepared all frozen cases.',flush=True)


def score(events,truth,songs):
    starts=[e for e in events if e['event']=='start']; ends=[e for e in events if e['event']=='end']
    found={};false=[];duplicates=0;delays=[];position_errors=[];end_delays=[]
    for s in starts:
        matches=[g for g in truth if g['song']==s['song'] and g['start']<=s['consumed']<=g['end']+1]
        if not matches:
            false.append(s);continue
        g=matches[0]
        if s['song'] in found:duplicates+=1;continue
        found[s['song']]=s;delays.append(s['consumed']-g['start'])
        position_errors.append(abs(s['position']-(s['t']-g['start'])*g['tempo']))
        terminal=next((e for e in ends if e['song']==s['song'] and e['start']==s['t']),None)
        if terminal:end_delays.append(terminal['consumed']-g['end'])
    return dict(detected=len(found),total=len(truth),misses=[g['song'] for g in truth if g['song'] not in found],
                first_starts=found,starts=starts,false_starts=false,duplicates=duplicates,
                start_delays=delays,position_errors=position_errors,end_delays=end_delays)


def evaluate(args):
    prepared=json.loads((ROOT/'prepared.json').read_text()); evidence=ROOT/'evidence'/args.label/args.split;evidence.mkdir(parents=True,exist_ok=True)
    if (evidence/'results.json').exists():
        raise FileExistsError('Preserve the prior run; choose a fresh --label instead of overwriting evidence')
    binaries={'pr3':ROOT/'binaries/pr3','pr7':ROOT/'binaries/pr7','modal':Path(args.candidate)}
    meta=dict(status='running',evaluator=common.identity(__file__),prepared=common.identity(ROOT/'prepared.json'),
              binaries={k:common.identity(v) for k,v in binaries.items()},candidate_commit=Path(args.candidate).parent.joinpath('candidate-commit.txt').read_text().strip(),
              platform=platform.platform(),python=sys.version,numpy=np.__version__,rayon_threads=1,variants=VARIANTS)
    common.save(evidence/'metadata.json',meta)
    runs=[]
    cases=[(k,v) for k,v in prepared['streams'].items() if k.startswith(args.split+'-')]
    if args.only:cases=[(k,v) for k,v in cases if k[len(args.split)+1:] in args.only.split(',')]
    try:
        for case,(name,stream) in enumerate(cases):
            treatment=name[len(args.split)+1:]
            order=['pr3','pr7','modal'] if case%2==0 else ['modal','pr7','pr3']
            if args.split=='negative':order=[s for s in order if s!='pr3']
            for side in order:
                cmd=[str(binaries[side].resolve())]
                if side=='modal':cmd+=['--modal-fit']
                for song in stream['songs']:cmd+=['--watch',song+'='+prepared['sources'][song]['reference']['path']]
                cmd+=['--stream',stream['path']]
                out=evidence/f'{treatment}-{side}.jsonl';err=evidence/f'{treatment}-{side}.stderr.txt'
                started=time.perf_counter()
                with out.open('w') as o,err.open('w') as e:
                    r=subprocess.run(cmd,stdout=o,stderr=e,env=dict(os.environ,RAYON_NUM_THREADS='1'),timeout=300)
                elapsed=time.perf_counter()-started
                assert r.returncode==0,(cmd,r.returncode)
                events=common.parse_events(out,side=='pr3',stream['seconds'])
                row=dict(treatment=treatment,side=side,command=cmd,returncode=r.returncode,elapsed_seconds=elapsed,
                         stdout=common.identity(out),stderr=common.identity(err),input=stream,
                         done=events[-1],event_digest=common.stable_digest(events),**score(events,stream['truth'],stream['songs']))
                runs.append(row);common.save(evidence/'results.json',runs)
                print(f"{args.split}/{treatment}/{side}: {row['detected']}/{row['total']}, false {len(row['false_starts'])}, duplicates {row['duplicates']}, wall {elapsed:.3f}",flush=True)
            if args.split!='negative':
                pair=[r for r in runs if r['treatment']==treatment and r['side'] in ['pr3','pr7']]
                assert pair[0]['event_digest']==pair[1]['event_digest'],('baseline parity',treatment)
        meta.update(status='completed',runs=len(runs))
    except BaseException as error:
        meta.update(status='failed',error=repr(error));raise
    finally:common.save(evidence/'metadata.json',meta)


def peak_survival():
    prepared=json.loads((ROOT/'prepared.json').read_text());output=ROOT/'fingerprints';output.mkdir(exist_ok=True)
    results=[]
    def fingerprint(path,key):
        dest=output/(key+'.txt')
        cmd=[str((ROOT/'binaries/pr7').resolve()),'fingerprint',str(path)]
        with dest.open('w') as f:subprocess.run(cmd,stdout=f,check=True,timeout=30)
        rows=[l.split() for l in dest.read_text().splitlines()]
        return [tuple(map(int,r[1:])) for r in rows if r[0]=='P'],Counter(int(r[1]) for r in rows if r[0]=='H'),common.identity(dest)
    for song,record in sorted(prepared['sources'].items()):
        if song.startswith('n'):continue
        original,keys,identity=fingerprint(record['reference']['path'],song+'-reference')
        for name in ['clean',*EQ]:
            peaks,hashes,ident=fingerprint(prepared['clips'][song+'-'+name]['path'],song+'-'+name)
            survived=sum(any(abs(a-c)<=4 and abs(b-d)<=1 for c,d in peaks) for a,b in original)
            results.append(dict(song=song,treatment=name,reference_peaks=len(original),transformed_peaks=len(peaks),
                                reference_peaks_surviving=survived,peak_survival=survived/max(1,len(original)),
                                reference_hashes=keys.total(),exact_hash_key_overlap=(keys&hashes).total()/max(1,keys.total()),
                                reference=identity,transformed=ident))
        common.save(ROOT/'peak-survival.json',results)
        print('Fingerprinted',song,flush=True)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--prepare',action='store_true');p.add_argument('--peaks',action='store_true')
    p.add_argument('--split',choices=['development','heldout','negative'])
    p.add_argument('--candidate',default='target/lab/modal-binaries/candidate')
    p.add_argument('--label',default='modal-a',help='Separate evidence directory for each candidate attempt')
    p.add_argument('--only',help='Comma-separated treatments for diagnosis; preserve output separately before full run')
    args=p.parse_args();ROOT.mkdir(exist_ok=True,parents=True)
    if args.prepare:prepare()
    elif args.peaks:peak_survival()
    elif args.split:evaluate(args)
    else:p.error('choose --prepare, --peaks or --split')


if __name__=='__main__':main()
