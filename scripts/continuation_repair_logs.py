#!/usr/bin/env python3
"""Preserve two truncated E014 logs and replay their exact frozen commands."""
import concurrent.futures
import importlib.util
import json
from pathlib import Path
import shutil
import sys
sys.path.insert(0,str(Path('scripts').resolve()))
import continuation_eval as current
root=current.ROOT
frozen=root/'frozen-evaluator/continuation_eval.py'
spec=importlib.util.spec_from_file_location('executed',frozen)
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
backup=root/'integrity-replay';backup.mkdir(exist_ok=True)
keys=['clean-t21-continuation','mix-t21-continuation']
records=[]
for key in keys:
 p=root/'runs'/(key+'.result.json')
 if (backup/p.name).exists():
  records.append(c.read(backup/p.name));continue
 r=c.read(p)
 assert c.e.sha(r['stdout']['path'])!=r['stdout']['sha256']
 for suffix in ['.result.json','.jsonl','.stderr.txt']:
  source=root/'runs'/(key+suffix);shutil.move(source,backup/source.name)
 records.append(r)
def replay(r):
 m=c.read(root/'manifest.json')
 cached=root/'runs'/(r['id']+'.result.json')
 row=c.read(cached) if cached.exists() else c.run_one(Path(r['command'][0]),m,r['input'],r['arm'])
 assert row['status']=='completed'
 current.validate_resume(row,r['signature'])
 return dict(id=r['id'],original_record=c.e.identity(backup/(r['id']+'.result.json')),
   preserved_stdout=c.e.identity(backup/(r['id']+'.jsonl')),
   expected_original_stdout=r['stdout'],replay_stdout=row['stdout'],
   byte_identical_to_original_recorded_hash=row['stdout']['sha256']==r['stdout']['sha256'],
   original_outcome=r.get('outcome'),replay_outcome=row.get('outcome'))
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
 rows=list(pool.map(replay,records))
c.save(root/'integrity-replay.json',dict(reason='Two stdout files were truncated after completed records were written; cause not established. Original files and records retained; strict hashes were not waived.',
 original_native_runs=231,additional_exact_command_replays=2,total_native_invocations=233,
 evaluator=c.e.identity(frozen),replay_script=c.e.identity(__file__),runs=rows))
print(json.dumps(rows,indent=2))
