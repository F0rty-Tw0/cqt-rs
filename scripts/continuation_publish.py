#!/usr/bin/env python3
"""Write lightweight, auditable E014 GitHub evidence after the full recount."""
import json
from pathlib import Path
import continuation_eval as c


def main():
    root=c.ROOT; audit=c.read(root/'audit.json')
    assert audit['runs']==231 and audit['parity']['status']=='passed'
    output=Path('docs/evidence/E014');output.mkdir(parents=True,exist_ok=True)
    events=[];identities=[]
    for p in sorted((root/'runs').glob('*.result.json')):
        r=c.read(p);c.validate_resume(r,r['signature'])
        native=list(map(json.loads,Path(r['stdout']['path']).read_text().splitlines()))
        kept=[x for x in native if x['event'] in ('start','end','done')]
        q=r['input']
        ground_truth={k:q[k] for k in ('id','group','song','seconds','start_seconds','intervals') if k in q}
        events.append(dict(run=r['id'],arm=r['arm'],truth=ground_truth,events=kept,
                           full_stdout_sha256=r['stdout']['sha256']))
        identities.append(dict(id=r['id'],command=r['command'],source_commit=r['source_commit'],
            signature=r['signature'],returncode=r['returncode'],elapsed_seconds=r['elapsed_seconds'],
            stdout=r['stdout'],stderr=r['stderr']))
    (output/'accepted-events.jsonl').write_text(''.join(json.dumps(x,separators=(',',':'))+'\n' for x in events))
    c.save(output/'runs.json',identities)
    # Compact JSON retains every per-query/per-play error without a large
    # whitespace expansion. Full observations remain in the separate archive.
    (output/'audit.json').write_text(json.dumps(audit,separators=(',',':'))+'\n')
    for name in ['manifest.json','robustness.json','native-ci.json','parity.json','decoded-repair.json',
                 'integrity-replay.json','remaining-schedule.json','resume-adapter.json','resume-regression.json','python-tests.txt']:
        if (root/name).exists(): (output/name).write_bytes((root/name).read_bytes())
    for name in ['Cargo.lock.txt','rustc.txt','binaries.sha256','candidate-commit.txt','candidate-status.txt']:
        (output/name).write_bytes((root/'candidate-v3'/name).read_bytes())
    inventory={p.name:c.e.identity(p) for p in output.iterdir() if p.is_file() and p.name!='inventory.json'}
    c.save(output/'inventory.json',inventory)
    report=Path('docs/experiments/E014-results.md')
    report.write_bytes((root/'results.md').read_bytes())
    print('Prepared',len(events),'replayable accepted-event traces;',sum(p.stat().st_size for p in output.iterdir()),'bytes')


if __name__=='__main__':main()
