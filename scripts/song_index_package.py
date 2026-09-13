#!/usr/bin/env python3
"""Package compact, auditable E011/E012 evidence without audio or executables."""
import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT=Path('target/song-index')
DEST=Path('docs/evidence/E011')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def archive(path, entries):
    checksums={}
    with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as out:
        for name,source in sorted(entries.items()):
            data=Path(source).read_bytes()
            checksums[name]=digest(data)
            info=zipfile.ZipInfo(name,date_time=(1980,1,1,0,0,0))
            info.compress_type=zipfile.ZIP_DEFLATED
            info.external_attr=0o100644 << 16
            out.writestr(info,data)
        out.writestr('SHA256SUMS',''.join(f'{sha}  {name}\n' for name,sha in sorted(checksums.items())))
    with zipfile.ZipFile(path) as inp:
        assert inp.testzip() is None
        for name,sha in checksums.items():
            assert digest(inp.read(name))==sha
    return dict(file=path.name,bytes=path.stat().st_size,sha256=digest(path.read_bytes()),members=len(checksums))


def main():
    assert json.loads((ROOT/'summary.json').read_text())['status']=='verified-execution'
    assert json.loads((ROOT/'modal-summary.json').read_text())['status']=='verified-execution'
    tests=json.loads((ROOT/'python-tests.json').read_text())
    assert tests['returncode']==0 and digest((ROOT/'python-tests.txt').read_bytes())==tests['output_sha256']
    for path,sha in tests['test_files'].items():
        assert digest(Path(path).read_bytes())==sha
    DEST.mkdir(parents=True,exist_ok=True)
    for name in ['summary.json','modal-summary.json','python-tests.json','python-tests.txt','attribution.txt']:
        shutil.copyfile(ROOT/name,DEST/name)
    entries={path.name:path for path in ROOT.glob('*.json') if path.name not in ['summary.json','modal-summary.json','python-tests.json']}
    for path in (ROOT/'overlap-diagnostic').glob('*.json'):
        entries['overlap-diagnostic/'+path.name]=path
    entries['frozen-evaluator-sources/song_index_overlap.py']=Path('scripts/song_index_overlap.py')
    entries['frozen-evaluator-sources/song_index_prepare.py']=Path('scripts/song_index_prepare.py')
    if (ROOT/'preparation-v1.py').exists():
        entries['frozen-evaluator-sources/preparation-v1.py']=ROOT/'preparation-v1.py'
    entries['frozen-evaluator-sources/song_index_eval.py']=Path('scripts/song_index_eval.py')
    entries['frozen-evaluator-sources/song_index_modal.py']=Path('scripts/song_index_modal.py')
    for name in ['Cargo.lock.txt','rustc.txt','linked-libraries.txt','candidate-commit.txt','candidate-status.txt','binaries.sha256']:
        entries['native-build/'+name]=Path('target/chunk-query/binaries')/name
    packages=[archive(DEST/'inputs-and-provenance.zip',entries)]
    for label,prefix in [('default-a','default'),('modal-a','modal')]:
        run_dir=ROOT/'runs'/label
        shutil.copyfile(run_dir/'metadata.json',DEST/(prefix+'-metadata.json'))
        rows=json.loads((run_dir/'results.json').read_text())
        for group in ['exact','clean','frozen','mix','negative']:
            entries={}
            for row in rows:
                if row['group']!=group:continue
                for suffix in ['.jsonl','.stderr.txt','.result.json']:
                    path=run_dir/(row['id']+suffix)
                    entries[path.name]=path
            packages.append(archive(DEST/(prefix+'-'+group+'.zip'),entries))
    (DEST/'archives.json').write_text(json.dumps(packages,indent=2)+'\n')
    diagnostic=ROOT/'matched-source-controls'
    if (diagnostic/'summary.json').exists():
        assert json.loads((diagnostic/'summary.json').read_text())['status']=='verified-execution'
        entries={p.name:p for p in diagnostic.iterdir() if p.suffix in ['.json','.jsonl','.txt']}
        extra=archive(DEST/'matched-source-diagnostics.zip',entries)
        (DEST/'diagnostic-archives.json').write_text(json.dumps([extra],indent=2)+'\n')
        shutil.copyfile(diagnostic/'summary.json',DEST/'matched-source-summary.json')
    print(json.dumps(packages,indent=2))


if __name__=='__main__':
    main()
