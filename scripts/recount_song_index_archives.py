#!/usr/bin/env python3
"""Stdlib-only independent E011/E012 recount; needs no audio or native binary."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import zipfile

ROOT=Path('docs/evidence/E011')


def main():
    packages=json.loads((ROOT/'archives.json').read_text())
    outputs=[]
    counts=Counter()
    for package in packages:
        path=ROOT/package['file']
        assert hashlib.sha256(path.read_bytes()).hexdigest()==package['sha256'],path
        with zipfile.ZipFile(path) as inp:
            assert inp.testzip() is None
            for line in inp.read('SHA256SUMS').decode().splitlines():
                sha,name=line.split('  ',1)
                assert hashlib.sha256(inp.read(name)).hexdigest()==sha,(path,name)
            if path.name=='inputs-and-provenance.zip':
                manifest=json.loads(inp.read('manifest.json'))
    refs={side:{row['id']:row for row in rows} for side,rows in manifest['references'].items()}
    queries={row['id']:row for row in manifest['queries']}
    seen=set()
    for package in packages:
        path=ROOT/package['file']
        with zipfile.ZipFile(path) as inp:
            for name in inp.namelist():
                if not name.endswith('.result.json'):continue
                row=json.loads(inp.read(name))
                q=queries[row['query_id']]
                side=row['side']
                reference=refs['full' if side=='full-modal' else side]
                assert row['id'] not in seen
                seen.add(row['id'])
                raw=inp.read(row['id']+'.jsonl')
                assert hashlib.sha256(raw).hexdigest()==row['stdout']['sha256']
                assert hashlib.sha256(inp.read(row['id']+'.stderr.txt')).hexdigest()==row['stderr']['sha256']
                assert row['returncode']==0 and row['status']=='completed'
                assert row['input']==q
                events=[json.loads(line) for line in raw.splitlines()]
                assert events[-1]['event']=='done' and events[-1]['audio_seconds']==10
                indexed=[event['song'] for event in events if event['event']=='index']
                assert len(indexed)==len(reference) and set(indexed)==set(reference)
                starts=[event for event in events if event['event']=='start']
                winner=min(starts,key=lambda event:(-event['evidence'],event['consumed'],event['song']),default=None)
                predicted=reference[winner['song']]['song'] if winner else None
                if q['song'] is None:
                    outcome='false-accept' if starts else 'correct-rejection'
                else:
                    outcome='no-match' if winner is None else ('correct' if predicted==q['song'] else 'wrong-match')
                wrong=sum(reference[event['song']]['song']!=q['song'] for event in starts)
                assert outcome==row['outcome'] and predicted==row['predicted']
                assert wrong==len(row['wrong_parent_starts'])
                counts[(q['group'],side,outcome)]+=1
                outputs.append(dict(id=row['id'],query_id=q['id'],side=side,group=q['group'],outcome=outcome,
                                    predicted=predicted,accepted_starts=len(starts),wrong_parent_starts=wrong))
    assert len(outputs)==288
    expected={(q,s) for q in queries for s in ['chunks','full','full-modal']}
    assert {(r['query_id'],r['side']) for r in outputs}==expected
    result=dict(status='verified',native_runs=288,query_count=len(queries),
                counts=[dict(group=g,side=s,outcome=o,count=n) for (g,s,o),n in sorted(counts.items())],
                wrong_parent_starts=sum(r['wrong_parent_starts'] for r in outputs),
                cases=sorted(outputs,key=lambda r:r['id']))
    (ROOT/'independent-recount.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:result[k] for k in ['status','native_runs','query_count','counts','wrong_parent_starts']},indent=2))


if __name__=='__main__':
    main()
