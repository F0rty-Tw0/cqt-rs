#!/usr/bin/env python3
"""Bounded post-outcome controls for the four low-evidence random mix misses."""
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess

import real_mix_eval as e
from song_index_prepare import cut, atomic_bytes
from song_index_eval import outcome, validate_events, BINARY_SHA

ROOT=Path('target/song-index')


def main():
    manifest=json.loads((ROOT/'manifest.json').read_text())
    meta=json.loads((ROOT/'runs/modal-a/metadata.json').read_text())
    assert meta['status']=='completed'
    binary=meta['binary']['path']
    assert e.sha(binary)==BINARY_SHA
    output=ROOT/'matched-source-controls'
    output.mkdir(exist_ok=False)
    refs={r['id']:r for r in manifest['references']['full']}
    cases=[]
    for song in ['t04','t10','t13','t22']:
        mix=next(q for q in manifest['queries'] if q['id']=='mix-'+song)
        failed=json.loads((ROOT/'runs/modal-a'/('mix-'+song+'-full-modal.result.json')).read_text())
        assert failed['outcome']=='no-match' and failed['diagnostics']['strongest_report']['confidence'] < 70
        anchors=[a for a in mix['annotation']['anchors']
                 if a['mix_start'] <= mix['start_seconds'] and a['mix_end'] >= mix['start_seconds']+10]
        anchor=max(anchors,key=lambda a:(a['similarity'],-a['mix_start']))
        source_start=anchor['source_start']+(mix['start_seconds']-anchor['mix_start'])*anchor['tempo']
        assert e.sha(refs[song]['path'])==refs[song]['sha256']
        query=cut(refs[song]['path'],round(source_start*44100),441000,output/(song+'.wav'))
        cases.append(dict(id='matched-'+song,song=song,anchor=anchor,mix_query=mix,
                          query=query,approximate_source_start_seconds=source_start))
    plan=dict(status='frozen-before-four-diagnostic-runs',evaluator=e.identity(__file__),binary=meta['binary'],
              source_commit=meta['source_commit'],manifest=e.identity(ROOT/'manifest.json'),cases=cases,
              options=['--modal-fit'],thresholds_unchanged=True,
              selection='The four remaining random mix no-matches whose highest reported confidence is below 70.',
              source_mapping='Highest-similarity covering frozen STFT anchor; source_start + mix_delta * tempo.',
              limitations='Approximate one-second-resolution cue alignment; clean clip is ten untransformed seconds. '
                          'Different tempo can change exact content extent. These adaptive controls are not new held-out accuracy.',
              budget='Four fresh full-database queries, at most four concurrent, 600 seconds per query.')
    e.save(output/'plan.json',plan)
    def run(case):
        command=[binary]
        for ref in manifest['references']['full']:
            command+=['--watch',ref['id']+'='+ref['path']]
        command+=['--stream',case['query']['path'],'--modal-fit']
        row=dict(case=case,command=command,status='running')
        path=output/(case['id']+'.result.json')
        e.save(path,row)
        proc=subprocess.run(command,capture_output=True,env=dict(os.environ,RAYON_NUM_THREADS='1'),timeout=600)
        stdout,stderr=output/(case['id']+'.jsonl'),output/(case['id']+'.stderr.txt')
        atomic_bytes(stdout,proc.stdout);atomic_bytes(stderr,proc.stderr)
        assert proc.returncode==0
        events=[json.loads(line) for line in proc.stdout.splitlines()]
        validate_events(events,refs,dict(id=case['id']))
        row.update(status='completed',returncode=proc.returncode,stdout=e.identity(stdout),stderr=e.identity(stderr),
                   **outcome(events,refs,case['song']))
        e.save(path,row)
        print(case['id'],row['outcome'],row['predicted'],flush=True)
        return row
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results=list(pool.map(run,cases))
    e.save(output/'results.json',results)
    for row in results:
        for record in [row['stdout'],row['stderr'],row['case']['query']]:
            assert e.sha(record['path'])==record['sha256']
    e.save(output/'summary.json',dict(status='verified-execution',runs=4,
        correct=sum(r['outcome']=='correct' for r in results),
        wrong_parent_starts=sum(len(r['wrong_parent_starts']) for r in results),
        plan=e.identity(output/'plan.json'),results=e.identity(output/'results.json')))


if __name__=='__main__':
    main()
