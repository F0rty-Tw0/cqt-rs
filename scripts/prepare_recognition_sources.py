#!/usr/bin/env python3
"""Download frozen E009 sources, verify original hashes and prepare midpoint references."""
from pathlib import Path
import sys,json,concurrent.futures,wave
sys.path.insert(0,'scripts')
import real_mix_eval as e
root=Path('target/lab');media=root/'media';media.mkdir(exist_ok=True)
old=json.loads(Path('docs/evidence/E007/downloads.json').read_text())
tracks=json.loads(Path('experiments/toucan2020.json').read_text())['tracks']+json.loads(Path('experiments/recognition-holdout.json').read_text())['tracks']
records={}
def prepare(t):
 name=t['id'];dest=media/(name+'.mp3')
 if not dest.exists():_,record=e.download((name,t['url']),media)
 else:record=dict(url=t['url'],file=e.identity(dest),reused=True)
 if name in old:assert e.sha(dest)==old[name]['file']['sha256'],name
 full=media/f'{name}-full.wav';short=media/f'{name}-10s.wav'
 if not full.exists():e.ffmpeg(['-i',dest,'-ac','1','-ar','44100','-c:a','pcm_s16le',full],[])
 n=e.wav_info(full)['samples'];start=(n-441000)//2
 with wave.open(str(full)) as a,e.wav_writer(short) as b:
  a.setpos(start);pcm=a.readframes(441000);assert len(pcm)==882000;b.writeframes(pcm)
 record.update(track=t,reference=e.identity(short),original=e.identity(full),source_seconds=n/44100,clip_start=start/44100)
 return name,record
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
 for future in concurrent.futures.as_completed([pool.submit(prepare,t) for t in tracks]):
  name,record=future.result();records[name]=record;e.save(root/'sources.json',records)
  print('Prepared',name,record['track']['title'],flush=True)
