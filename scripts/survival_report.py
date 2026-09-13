#!/usr/bin/env python3
"""Publish every E015 finding with compact GitHub proofs and a raw archive."""
import csv
import gzip
import hashlib
import io
import json
from pathlib import Path
import zipfile
import survival_extract as x


def gz(path,destination):
    data=Path(path).read_bytes();Path(destination).write_bytes(gzip.compress(data,mtime=0))
    assert gzip.decompress(Path(destination).read_bytes())==data


def main():
    root=x.ROOT;a=x.read(root/'audit.json');out=Path('docs/evidence/E015');out.mkdir(parents=True,exist_ok=True)
    assert len(a['plays'])==110 and len(a['extractions'])==27
    assert all(p['covered_by_serialization'] for p in a['parity']['mismatches'])
    gz(root/'audit.json',out/'audit.json.gz')
    x.save(out/'summary.json',dict(summary=a['summary'],parity=a['parity'],evaluator=a['evaluator'],protocol=a['protocol'],limits=a['limits']))
    for label,rows in [('plays',a['plays']),('windows',a['windows'])]:
        flat=[]
        for r in rows:
            row={k:v for k,v in r.items() if not isinstance(v,(list,dict))}
            if 'hash_survival' in r:row.update({'hash_'+k:v for k,v in r['hash_survival'].items()})
            flat.append(row)
        fields=list(dict.fromkeys(k for r in flat for k in r))
        with (out/(label+'.csv')).open('w') as f:
            w=csv.DictWriter(f,fields,lineterminator="\n");w.writeheader();w.writerows(flat)
    provenance=root/'extractions.json';x.save(provenance,a['extractions']);gz(provenance,out/'extractions.json.gz')
    for name in ('native-ci.json','python-tests.txt'):
        path=root/name
        if path.exists():(out/name).write_bytes(path.read_bytes())
    lines=['# E015: evidence survival at the known correct trajectory','',
        '**Diagnostic verified; matcher unchanged.** The next candidate experiment is a bounded pair-assisted retrieval fallback for sparse surviving music peaks. Voiceover also needs a separate verification experiment.', '',
        'The unchanged E014 native executable exported 27 files: 22 references and five programmes, covering every one of 110 controlled plays. All 5,693,564 native triplets were reproduced independently from the exported peaks. The diagnostic examines '+str(len(a['windows']))+' interior two-second intervals. Correct trajectories are supplied offline; these are not additional recognized plays.', '',
        '| Treatment | E014 detected | Plays with two oracle checks | Reference peak recovery | Matched-query fraction | Reference triplet recovery |',
        '| --- | ---: | ---: | ---: | ---: | ---: |']
    for s in a['summary']:
        lines.append(f"| {s['case']} | {s['detected']}/22 | {s['oracle_two_checks']}/22 | {100*s['reference_recall']:.1f}% | {100*s['query_precision']:.1f}% | {100*s['hash_recovery']:.1f}% |")
    lines += ['', 'Peak fractions pool counts within the native verifier’s query-defined reference spans. Triplet recovery uses known source geometry and the frozen key/anchor/span tolerances before the index cap. These denominators differ; neither metric is an independent vote count or recognition probability. Even the gain control has incomplete recovery which can reflect excerpt/programme analysis-grid and feature-context changes or PCM quantization; these causes are not isolated here. Relative treatment comparisons use that control.', '',
      '## What failed', '',
      '**Heavy noise: retrieval is the immediate bottleneck.** All 14 missed plays have at least two consecutive oracle start checks, while their correct-song retrieval confidence never reaches 70 in the measured interior windows. Peak recovery falls from 84.2% in gain controls to 31.8%, but triplet recovery falls further, from 52.9% to 3.3%. Surviving query peaks are mostly consistent with the song (84.8% matched), so lowering the verification threshold does not address these misses. There may be very little evidence: one interior observation has only one matching peak. Oracle passage does not justify accepting such a case in production.', '',
      '**Voiceover: evidence is both lost and diluted.** All six missed plays lack two consecutive oracle start checks. Matched-query fraction falls to 45.1%, while reference peak recovery is 51.4%. Some windows retain music correspondence but added speech peaks reduce the fraction; others lose music peaks too. Four of the six misses have at least two consecutive oracle hold checks at 0.3, but not two start checks at 0.4. This supports testing a verifier that accounts for interference after retrieval is fixed; it does not support merely lowering the threshold.', '',
      '**Combined effects: more than one mechanism fails.** Three misses lack two consecutive oracle start checks. For t04 the correct trajectory has two consecutive passing oracle checks and a retrieved candidate passes, but continuation/start timing still fails. For t16 passing retrieved candidates use a source position incompatible with the supplied trajectory. One detected combined play lacks two interior oracle start checks; the diagnostic excludes boundary windows and recognition can choose another repeated position, so oracle and recognition totals are not interchangeable.', '',
      '## Every missed play', '',
      '| Treatment | Song | Attribution order | Oracle start / hold streak | Max retrieval confidence | Reference triplet recovery |',
      '| --- | --- | --- | ---: | ---: | ---: |']
    for p in a['plays']:
        if not p['e014_detected']:
            lines.append(f"| {p['case']} | {p['song']} | {p['diagnostic']} | {p['oracle_start_streak']} / {p['oracle_hold_streak']} | {p['maximum_retrieval_confidence']:.2f} | {100*p['hash_survival']['reference_recovery_fraction']:.2f}% |")
    lines += ['', 'Attributions follow a fixed order: oracle start support, retrieval confidence, retrieved start gate, true-position compatibility, then continuation/timing. They locate an immediate blocking condition; they do not prove other stages are healthy. All successes and their errors are retained in the per-play data.', '',
      '## Index cap and validation', '',
      '| Treatment | Geometrically supported query hashes | After native per-song/key cap of 8 |',
      '| --- | ---: | ---: |']
    for s in a['summary']:lines.append(f"| {s['case']} | {s['hashes']['supported_raw']} | {s['hashes']['supported_after_cap']} |")
    lines += ['', 'The cap removes additional support but is not the principal heavy-noise collapse: only 4,034 geometrically supported query hashes remain before the cap, compared with 65,449 in gain controls. Increasing the cap alone cannot restore missing triplets.', '',
      'All reference index counts and all five programme peak/hash counts agree with E014. Of 1,917 recorded native verification observations, 1,885 recount exactly at serialized parameters. The remaining 32 differ by at most two counts; all native values lie inside the uncertainty envelope implied by six-decimal source positions and eight-decimal tempo. The nominal differences and bounds remain in the audit. No matcher tolerance was changed. Synthetic mapping, peak insertion/deletion, index-cap, streak and independent brute-force tests validate the diagnostic.', '',
      '## Next bounded experiment', '',
      'Freeze E016 before implementation: add a bounded pair-assisted candidate retrieval path using the same 10-second context and two-second checks, preserving pitch/tempo estimation. Measure whether it recovers the 14 heavy-noise misses, at matched false-start policy and unchanged clean/pitch/BPM/EQ regression outcomes. Use support from distinct anchors over time; a high fraction from one peak is insufficient. Keep candidate and memory budgets explicit. Pair survival and extra collisions must be measured; improvement is a hypothesis.', '',
      'Keep the verifier unchanged in that experiment so the retrieval effect is identifiable. Voiceover start failures are expected to remain possible. A later independent verifier experiment should compare matched support against chance/background density, calibrated on separate negatives, rather than treating every unrelated speech peak as contradictory evidence. Real human speech and recordings excluded from development are still required for a general accuracy claim.', '',
      '## Provenance and reproduction', '',
      'Protocol: [E015](E015-survival-protocol.md), published at `b3c6751e545ed600915f3a77023bc40405e4e0f9`. Evaluated executable source remains `748dcd0e4ed4d1431a39a6878e99f1ee7075ccae`, SHA-256 `'+x.BINARY_SHA+'`. No native code or matching thresholds changed. [Exact-source CI](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784380994) passed; final-head checks are tracked separately on PR #4.', '',
      'The original protocol incorrectly listed the index cap as 64; source inspection corrected it to 8 before extraction. The only recount addition after the initial results was an uncertainty envelope for already-rounded native log parameters; all nominal count differences remain visible. No audio labels, transforms or native outputs were altered.', '',
      'Recover E014’s exact inputs using [its reproduction instructions](E014-reproduction.md), then run:', '',
      '```sh','for i in $(seq -w 1 22); do python3 scripts/survival_extract.py reference-t$i; done',
      'for case in gain noise_10db noise_0db voice_0db combined; do python3 scripts/survival_extract.py "$case"; done',
      "python3 -m unittest discover -s scripts -p 'test_*.py' -v",'python3 scripts/survival_diagnostic.py','python3 scripts/survival_report.py','```','',
      'The extractor verifies input/binary identities and writes completed files atomically. Strict signature/hash checks protect reuse. [Evidence](../evidence/E015/README.md) includes every play/window, the compressed complete audit, extraction provenance and raw archive identity. This known-recording/synthetic-speech diagnostic does not validate arbitrary pitch/BPM/EQ/noise ranges, real DJ source trajectories, exact boundaries, speed or universal 100% recognition. E014’s default-switch rejection remains in force. No merge or release.','']
    report=Path('docs/experiments/E015-results.md');report.write_text('\n'.join(lines))
    (out/'README.md').write_text('''# E015 evidence

[Findings](../../experiments/E015-results.md).
`plays.csv` retains all 110 plays, including every miss; `windows.csv` retains
all interior two-second oracle windows. `audit.json.gz` contains the complete
JSON audit, including native observations and all 32 nominal parity differences
with their serialization uncertainty bounds. `extractions.json.gz` records
all 27 native commands, inputs, output hashes and exits. Compressed files are
ordinary gzip and can be read with Python gzip or `gzip -dc`.

The separately supplied `E015-evidence.zip` contains every raw native P/H export
(compressed text), extraction records, stderr, full audit, protocol, diagnostic
source and tests. It contains no audio or executables. See `archive.json` for
its SHA-256 and size. Every ZIP member is hashed in SHA256SUMS.
''')
    files={}
    for p in (root/'native').iterdir():
        if p.suffix in ('.json','.gz') or p.name.endswith('.stderr.txt'):files['native/'+p.name]=p
    for name in ('audit.json','diagnostic.log','python-tests.txt','extractions.json'):
        files[name]=root/name
    for p in [Path('scripts')/n for n in ('survival_extract.py','survival_diagnostic.py','survival_report.py','test_survival_diagnostic.py')]:files[str(p)]=p
    for p in (Path('docs/experiments/E015-survival-protocol.md'),report):files[str(p)]=p
    executed=root/'executed/survival_diagnostic.py'
    if executed.exists():
        assert x.sha(executed)==a['evaluator']['sha256']
        files['executed/survival_diagnostic.py']=executed
    else:
        assert x.sha('scripts/survival_diagnostic.py')==a['evaluator']['sha256']
    archive=root/'E015-evidence.zip'
    with zipfile.ZipFile(archive,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        hashes=[]
        for name,p in sorted(files.items()):
            data=p.read_bytes();hashes.append(hashlib.sha256(data).hexdigest()+'  '+name);z.writestr(name,data)
        z.writestr('SHA256SUMS','\n'.join(hashes)+'\n')
    with zipfile.ZipFile(archive) as z:
        assert z.testzip() is None
        for line in z.read('SHA256SUMS').decode().splitlines():
            expected,name=line.split('  ',1);assert hashlib.sha256(z.read(name)).hexdigest()==expected
    x.save(out/'archive.json',dict(**x.identity(archive),members=len(files)+1,native_extractions=27,plays=110,validation='ZIP CRC and every member SHA-256 passed'))
    print('Published',len(a['plays']),'plays and',len(a['windows']),'windows; archive',archive.stat().st_size,'bytes')


if __name__=='__main__':main()
