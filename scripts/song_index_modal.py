#!/usr/bin/env python3
"""E012: change only modal fitting on all frozen E011 full-song queries."""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import time

import real_mix_eval as e
from song_index_eval import BINARY_SHA, outcome, validate_events
from song_index_prepare import atomic_bytes

ROOT = Path('target/song-index')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label',default='modal-a')
    parser.add_argument('--workers',type=int,choices=range(1,5),default=4)
    args = parser.parse_args()
    baseline_dir = ROOT/'runs/default-a'
    baseline_meta = json.loads((baseline_dir/'metadata.json').read_text())
    assert baseline_meta['status']=='completed' and baseline_meta['completed_runs']==192
    manifest = json.loads((ROOT/'manifest.json').read_text())
    assert e.sha(ROOT/'manifest.json')==baseline_meta['manifest']['sha256']
    binary = baseline_meta['binary']
    assert e.sha(binary['path'])==binary['sha256']==BINARY_SHA
    baseline = [r for r in json.loads((baseline_dir/'results.json').read_text()) if r['side']=='full']
    assert len(baseline)==96 and all(r['status']=='completed' for r in baseline)
    refs = {r['id']:r for r in manifest['references']['full']}
    for rows in [list(refs.values()),manifest['queries']]:
        for row in rows:
            assert e.sha(row['path'])==row['sha256'], row['path']
    for row in baseline:
        for field in ['stdout','stderr']:
            assert e.sha(row[field]['path'])==row[field]['sha256'], row['id']
    output = ROOT/'runs'/args.label
    output.mkdir(parents=True,exist_ok=False)
    metadata = dict(status='running',binary=binary,source_commit=baseline_meta['source_commit'],
        evaluator=e.identity(__file__), scorer=e.identity(Path(__file__).with_name('song_index_eval.py')),
        baseline=e.identity(baseline_dir/'results.json'), baseline_metadata=e.identity(baseline_dir/'metadata.json'),
        manifest=e.identity(ROOT/'manifest.json'), workers=args.workers, rayon_threads_per_process=1,
        native_options=['--modal-fit'], expected_runs=96)
    e.save(output/'metadata.json',metadata)
    def run(before):
        key=before['query_id']+'-full-modal'
        command=before['command']+['--modal-fit']
        row=dict(id=key,query_id=before['query_id'],group=before['group'],song=before['song'],
                 supported=before['supported'],side='full-modal',input=before['input'],
                 command=command,baseline_id=before['id'],baseline_stdout=before['stdout'],status='running')
        e.save(output/(key+'.active.json'),row)
        begin=time.perf_counter()
        stdout,stderr=output/(key+'.jsonl'),output/(key+'.stderr.txt')
        try:
            try:
                proc=subprocess.run(command,capture_output=True,timeout=600,
                                    env=dict(os.environ,RAYON_NUM_THREADS='1'))
            except subprocess.TimeoutExpired as ex:
                atomic_bytes(stdout,ex.stdout or b'')
                atomic_bytes(stderr,ex.stderr or b'')
                raise
            atomic_bytes(stdout,proc.stdout)
            atomic_bytes(stderr,proc.stderr)
            row.update(returncode=proc.returncode,elapsed_seconds=time.perf_counter()-begin,
                       stdout=e.identity(stdout),stderr=e.identity(stderr))
            assert proc.returncode==0,(key,proc.returncode)
            events=[json.loads(line) for line in proc.stdout.splitlines()]
            validate_events(events,refs,before['input'])
            index=next(r for r in events if r['event']=='index_done')
            assert all(index[k]==before['index'][k] for k in ['songs','hashes','dropped','bytes'])
            row.update(status='completed',done=events[-1],index=index,event_digest=e.stable_digest(events),
                       **outcome(events,refs,before['song']))
        except Exception as ex:
            row.update(status='failed',error=repr(ex),elapsed_seconds=time.perf_counter()-begin)
        e.save(output/(key+'.result.json'),row)
        (output/(key+'.active.json')).unlink()
        print(key,row['status'],row.get('outcome'),row.get('predicted'),
              f"{row['elapsed_seconds']:.2f}s",flush=True)
        return row
    results=[]
    order=['exact','clean','frozen','mix','negative']
    baseline.sort(key=lambda r:(order.index(r['group']),r['query_id']))
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
            futures=[pool.submit(run,row) for row in baseline]
            for future in concurrent.futures.as_completed(futures):
                results.append(future.result())
                e.save(output/'results.json',sorted(results,key=lambda r:r['id']))
        assert len(results)==96
        metadata.update(status='completed' if all(r['status']=='completed' for r in results) else 'failed',
                        completed_runs=sum(r['status']=='completed' for r in results))
    except BaseException as ex:
        metadata.update(status='failed',error=repr(ex),completed_runs=len(results))
        raise
    finally:
        e.save(output/'metadata.json',metadata)
    if metadata['status']!='completed':
        raise RuntimeError('Candidate failures retained in per-query results')


if __name__=='__main__':
    main()
