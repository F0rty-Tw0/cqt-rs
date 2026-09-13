#!/usr/bin/env python3
"""Package E014 raw evidence without source audio or native executables."""
import hashlib
import json
from pathlib import Path
import zipfile
import continuation_eval as c


def main():
    root=c.ROOT
    audit=c.read(root/'audit.json')
    assert audit['runs']==231
    assert audit['parity']['status']=='passed'
    output=root/'E014-evidence.zip'
    members={}
    # Whitelist text metadata and logs; do not sweep directories of media.
    for directory in [root/'runs',root/'preparation',root/'integrity-replay']:
        if not directory.exists(): continue
        for p in directory.iterdir():
            if p.suffix in ('.json','.jsonl','.txt'):
                members[str(p.relative_to(root))]=p
    for p in root.iterdir():
        if p.suffix in ('.json','.log','.txt','.md') and p.name not in ('evidence-archive.json','library-save.json'):
            members[str(p.relative_to(root))]=p
    for p in (root/'robustness').glob('*.json'):
        members[str(p.relative_to(root))]=p
    members['programme/manifest.json']=root/'programme/manifest.json'
    for folder in ['candidate-v3','baseline']:
        for p in (root/folder).iterdir():
            if p.suffix in ('.txt','.sha256','.patch'):
                members[str(p.relative_to(root))]=p
    for name in ['continuation_eval.py','continuation_report.py','continuation_inputs.py',
                 'continuation_package.py','continuation_repair_logs.py','continuation_publish.py','continuation_remaining.py','continuation_resume.py','sequence_eval.py','song_index_eval.py','chunk_query_eval.py',
                 'real_mix_eval.py','song_index_prepare.py','test_continuation_eval.py','test_continuation_report.py']:
        members['evaluator/'+name]=Path('scripts')/name
    executed=root/'frozen-evaluator/continuation_eval.py'
    if not executed.exists(): executed=Path('scripts/continuation_eval.py')
    evaluator_hashes={c.read(p)['signature']['evaluator'] for p in (root/'runs').glob('*.result.json')}
    assert c.e.sha(executed) in evaluator_hashes
    members['evaluator/executed/continuation_eval.py']=executed
    members['protocol/E014-long-context.md']=Path('docs/experiments/E014-long-context.md')
    members['protocol/E014-scheduling.md']=Path('docs/experiments/E014-scheduling.md')
    members['protocol/E014-reproduction.md']=Path('docs/experiments/E014-reproduction.md')
    hashes=[]
    with zipfile.ZipFile(output,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for name,p in sorted(members.items()):
            data=p.read_bytes()
            assert p.suffix not in ('.wav','.mp3','.npy')
            hashes.append(hashlib.sha256(data).hexdigest()+'  '+name)
            z.writestr(name,data)
        z.writestr('SHA256SUMS','\n'.join(hashes)+'\n')
        z.writestr('README.txt',
            f"E014 raw evidence: {audit['runs']} scored runs, {audit['total_native_invocations']} native invocations; no audio or executable bytes.\n"
            'Source/binary/input identities and commands are in each run record.\n'
            'The audit recounts every native start/end and retains all regressions.\n'
            'Full reproduction instructions and source are on F0rty-Tw0/cqt-rs PR #4.\n')
    with zipfile.ZipFile(output) as z:
        assert z.testzip() is None
        for line in z.read('SHA256SUMS').decode().splitlines():
            expected,name=line.split('  ',1)
            assert hashlib.sha256(z.read(name)).hexdigest()==expected
    record=dict(**c.e.identity(output),members=len(members)+2,runs=audit['runs'],total_native_invocations=audit['total_native_invocations'],
                excludes=['audio','executables'],validation='ZIP CRC and every member SHA-256 passed')
    c.save(root/'evidence-archive.json',record)
    print(json.dumps(record,indent=2))


if __name__=='__main__':main()
