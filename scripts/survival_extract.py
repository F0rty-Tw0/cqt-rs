#!/usr/bin/env python3
"""Export frozen E015 peaks/hashes from the unchanged, verified E014 executable."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT=Path('target/survival')
BINARY=Path('target/continuation/candidate-v3/candidate').resolve()
BINARY_SHA='1b1c5968ce134bda1b559da9b8bdade829c30070c4960700da6015988c0ed9d7'
SOURCE='748dcd0e4ed4d1431a39a6878e99f1ee7075ccae'
CASES=('gain','noise_10db','noise_0db','voice_0db','combined')


def sha(path):
    h=hashlib.sha256()
    with Path(path).open('rb') as f:
        for chunk in iter(lambda:f.read(1048576),b''):h.update(chunk)
    return h.hexdigest()


def identity(path):return dict(path=str(Path(path).resolve()),bytes=Path(path).stat().st_size,sha256=sha(path))
def read(path):return json.loads(Path(path).read_text())
def save(path,row):
    path=Path(path);path.parent.mkdir(parents=True,exist_ok=True)
    temp=path.with_suffix(path.suffix+'.tmp');temp.write_text(json.dumps(row,indent=2)+'\n');temp.replace(path)


def inputs():
    m=read('target/continuation/manifest.json')
    return [dict(r,id='reference-'+r['id']) for r in m['references']]+[
        dict(read('target/continuation/runs/'+case+'-continuation.result.json')['input'],id=case) for case in CASES]


def extract(key):
    row=next(r for r in inputs() if r['id']==key)
    assert sha(BINARY)==BINARY_SHA
    assert sha(row['path'])==row['sha256']
    command=[str(BINARY),'fingerprint',row['path']]
    signature=dict(command=command,binary=BINARY_SHA,input=row['sha256'],extractor=sha(__file__))
    output=ROOT/'native';output.mkdir(parents=True,exist_ok=True)
    record=output/(key+'.json')
    if record.exists():
        old=read(record);assert old['signature']==signature and old['status']=='completed'
        for name in ('stdout','stderr'):assert sha(old[name]['path'])==old[name]['sha256']
        return old
    stdout=output/(key+'.txt');stderr=output/(key+'.stderr.txt')
    start=time.perf_counter()
    with stdout.with_suffix('.partial').open('wb') as out,stderr.open('wb') as err:
        run=subprocess.run(command,stdout=out,stderr=err,timeout=180,env=dict(os.environ,RAYON_NUM_THREADS='1'))
    assert run.returncode==0,stderr.read_text()
    stdout.with_suffix('.partial').replace(stdout)
    # Fingerprint CLI has no done event: require well-formed records and conserve
    # counts against E014's independent watch/stream event counts later.
    peaks=hashes=0
    with stdout.open() as f:
        for line in f:
            fields=line.split();assert fields[0] in ('P','H')
            assert len(fields)==(3 if fields[0]=='P' else 5)
            tuple(map(int,fields[1:]));peaks+=fields[0]=='P';hashes+=fields[0]=='H'
    assert peaks>0 and hashes>0
    gz=stdout.with_suffix('.txt.gz')
    with stdout.open('rb') as src,gz.with_suffix('.partial').open('wb') as raw:
        with gzip.GzipFile(fileobj=raw,mode='wb',mtime=0) as dst:
            for chunk in iter(lambda:src.read(1048576),b''):dst.write(chunk)
    gz.with_suffix('.partial').replace(gz)
    result=dict(id=key,input=row,signature=signature,source=SOURCE,binary=identity(BINARY),
        status='completed',returncode=run.returncode,elapsed_seconds=time.perf_counter()-start,
        peaks=peaks,hashes=hashes,stdout=identity(stdout),stderr=identity(stderr),compressed=identity(gz))
    save(record,result);print(key,peaks,hashes,round(result['elapsed_seconds'],2),flush=True)
    return result


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('key');args=parser.parse_args();extract(args.key)
