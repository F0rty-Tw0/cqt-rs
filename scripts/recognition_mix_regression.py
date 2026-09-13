#!/usr/bin/env python3
"""Repeat the E007 real-mix short-reference comparison with retained binaries."""
import argparse,json,os,subprocess,time
from pathlib import Path
import real_mix_eval as e

ROOT=Path('target/lab')

def prepare():
    dest=ROOT/'mix';dest.mkdir(exist_ok=True);log=[];streams={}
    source=Path('/workspace/scratch/bb1c840e177b/cqt-stream/target/source-probe-mix.bin')
    if not source.exists():
        _,record=e.download(('mix',json.loads(Path('experiments/toucan2020.json').read_text())['mix_url']),dest)
        source=Path(record['file']['path'])
    assert e.sha(source)=='39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103'
    for name,filt in [('clean','volume=0.25'),('pitch','volume=0.25,rubberband=pitch=1.122462048309373'),('tempo','volume=0.25,rubberband=tempo=0.9'),('combined_music','volume=0.25,rubberband=tempo=0.9:pitch=1.122462048309373')]:
        path=dest/(name+'.wav');print('Rendering mix',name,flush=True)
        e.ffmpeg(['-i',source,'-ac','1','-ar','44100','-af',filt,'-c:a','pcm_s16le',path],log)
        streams[name]=dict(**e.identity(path),**e.wav_info(path))
        e.save(dest/'preparation-checkpoint.json',dict(streams=streams,commands=log))
    voice=dest/'voice.wav';voice_text=Path('docs/evidence/E007/voice.txt').resolve()
    e.ffmpeg(['-f','lavfi','-i',f'flite=textfile={voice_text}:voice=slt','-ac','1','-ar','44100','-c:a','pcm_s16le',voice],log)
    combined=dest/'combined.wav';spans=e.voice_mix(streams.pop('combined_music')['path'],voice,combined)
    streams['combined']=dict(**e.identity(combined),**e.wav_info(combined))
    e.save(dest/'prepared.json',dict(evaluator=e.identity(__file__),source=e.identity(source),streams=streams,commands=log,voice=e.identity(voice),spans=spans))
    print('Mix prepared',flush=True)


def evaluate(args):
    prep=json.loads((ROOT/'mix/prepared.json').read_text());refs=json.loads((ROOT/'sources.json').read_text())
    output=ROOT/'evidence'/args.label/'mix';output.mkdir(parents=True,exist_ok=True)
    assert not (output/'results.json').exists(),'Use a fresh label to preserve prior evidence'
    binaries={'pr3':ROOT/'binaries/pr3','pr7':ROOT/'binaries/pr7','modal':Path(args.candidate)}
    meta=dict(status='running',evaluator=e.identity(__file__),prepared=e.identity(ROOT/'mix/prepared.json'),binaries={s:e.identity(p) for s,p in binaries.items()},candidate_commit=Path(args.candidate).parent.joinpath('candidate-commit.txt').read_text().strip(),rayon_threads=1)
    e.save(output/'metadata.json',meta);runs=[]
    try:
        for i,(name,stream) in enumerate(prep['streams'].items()):
            for side in (['pr3','pr7','modal'] if i%2==0 else ['modal','pr7','pr3']):
                cmd=[str(binaries[side].resolve())]
                if side=='modal':cmd+=['--modal-fit']
                for song,ref in sorted(refs.items()):
                    if song.startswith('t'):cmd+=['--watch',song+'='+ref['reference']['path']]
                cmd+=['--stream',stream['path']]
                out=output/f'{name}-{side}.jsonl';err=output/f'{name}-{side}.stderr.txt'
                start=time.perf_counter()
                with out.open('w') as a,err.open('w') as b:r=subprocess.run(cmd,stdout=a,stderr=b,env=dict(os.environ,RAYON_NUM_THREADS='1'),timeout=300)
                assert r.returncode==0
                elapsed=time.perf_counter()-start;events=e.parse_events(out,side=='pr3',stream['seconds'])
                starts=[v for v in events if v['event']=='start'];found=sorted(set(v['song'] for v in starts))
                row=dict(side=side,treatment=name,command=cmd,returncode=r.returncode,elapsed_seconds=elapsed,stdout=e.identity(out),stderr=e.identity(err),input=stream,done=events[-1],starts=starts,found=found,detected=len(found),event_digest=e.stable_digest(events))
                runs.append(row);e.save(output/'results.json',runs)
                print(name,side,len(found),'/22, starts',len(starts),'wall',round(elapsed,3),flush=True)
            pair=[r for r in runs if r['treatment']==name and r['side'] in ['pr3','pr7']]
            assert pair[0]['event_digest']==pair[1]['event_digest']
        meta.update(status='completed',runs=len(runs))
    except BaseException as ex:meta.update(status='failed',error=repr(ex));raise
    finally:e.save(output/'metadata.json',meta)


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--prepare',action='store_true')
    p.add_argument('--candidate',default='target/lab/final-binaries/candidate');p.add_argument('--label',default='modal-a')
    a=p.parse_args()
    if a.prepare:prepare()
    else:evaluate(a)

if __name__=='__main__':main()
