#!/usr/bin/env python3
"""Reuse the two full-mix worker slots for remaining E014 programmes."""
import concurrent.futures
import time
from pathlib import Path
import continuation_eval as c

root=c.ROOT
while not (root/'full-mix-results.json').exists():
    time.sleep(2)
assert all(r['status']=='completed' for r in c.read(root/'full-mix-results.json'))
binary=(root/'candidate-v3/candidate').resolve()
m=c.prepare()
jobs=[(m['programme'],a) for a in c.ARMS]+[(m['phase'],a) for a in ('long','continuation')]
jobs += [(q,a) for q in c.prepare_robustness()['cases'] for a in ('long','continuation')]
c.save(root/'remaining-schedule.json',dict(workers=2,maximum_total_processes=6,
    condition='full-mix stage completed before these workers start',
    scripts=c.e.identity(__file__),binary=c.e.identity(binary),jobs=[(q['id'],a) for q,a in jobs],
    note='resource-only scheduling amendment; unchanged audio, native flags, gates and sample-time metrics'))
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    futures=[pool.submit(c.run_one,binary,m,q,a) for q,a in jobs]
    rows=[f.result() for f in concurrent.futures.as_completed(futures)]
for stage,group in [('programme','programme'),('robustness','robustness')]:
    c.save(root/(stage+'-results.json'),sorted([r for r in rows if r['group']==group],key=lambda r:r['id']))
assert all(r['status']=='completed' for r in rows)
# Query workers may still run; wait before parity reads their cached clean case.
while not (root/'queries-results.json').exists():
    time.sleep(2)
assert all(r['status']=='completed' for r in c.read(root/'queries-results.json'))
c.run(binary,'parity')
