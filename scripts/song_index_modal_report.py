#!/usr/bin/env python3
"""Audit E012's paired modal-fitting experiment without hiding regressions."""
import argparse
from collections import Counter
import json
from pathlib import Path

import real_mix_eval as e
from song_index_eval import outcome, validate_events
from song_index_report import independent_prediction, paired_interval, GROUPS

ROOT=Path('target/song-index')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label',default='modal-a')
    args=parser.parse_args()
    folder=ROOT/'runs'/args.label
    meta=json.loads((folder/'metadata.json').read_text())
    assert meta['status']=='completed' and meta['completed_runs']==96
    for field in ['binary','evaluator','scorer','baseline','baseline_metadata','manifest']:
        assert e.sha(meta[field]['path'])==meta[field]['sha256'],field
    manifest=json.loads(Path(meta['manifest']['path']).read_text())
    queries={q['id']:q for q in manifest['queries']}
    refs={r['id']:r for r in manifest['references']['full']}
    for row in [*queries.values(),*refs.values()]:
        assert e.sha(row['path'])==row['sha256'],row['path']
    before={r['query_id']:r for r in json.loads(Path(meta['baseline']['path']).read_text()) if r['side']=='full'}
    results=json.loads((folder/'results.json').read_text())
    after={r['query_id']:r for r in results}
    assert len(after)==len(results)==96 and set(before)==set(after)==set(queries)
    recount=Counter()
    for qid,row in after.items():
        baseline=before[qid]
        assert row['status']=='completed' and row['returncode']==0
        assert row['input']==baseline['input']==queries[qid]
        assert row['command']==baseline['command']+['--modal-fit']
        assert row['baseline_stdout']==baseline['stdout']
        for item in [row['stdout'],row['stderr'],baseline['stdout'],baseline['stderr']]:
            assert e.sha(item['path'])==item['sha256']
        events=[json.loads(line) for line in Path(row['stdout']['path']).read_text().splitlines()]
        baseline_events=[json.loads(line) for line in Path(baseline['stdout']['path']).read_text().splitlines()]
        unchanged_fields=['t','consumed','song','evidence','confidence','votes']
        def evidence_trace(rows):
            return [{k:r[k] for k in unchanged_fields} for r in rows if r['event']=='report']
        assert evidence_trace(events)==evidence_trace(baseline_events), ('evidence changed',qid)
        validate_events(events,refs,queries[qid])
        assert e.wav_info(queries[qid]['path'])['samples']==441000
        index=next(r for r in events if r['event']=='index_done')
        assert all(index[k]==baseline['index'][k] for k in ['songs','hashes','dropped','bytes'])
        replay=outcome(events,refs,queries[qid]['song'])
        assert all(row[k]==value for k,value in replay.items())
        predicted,starts=independent_prediction(events,refs)
        expected=queries[qid]['song']
        label=('false-accept' if starts else 'correct-rejection') if expected is None else (
            'no-match' if predicted is None else ('correct' if predicted==expected else 'wrong-match'))
        assert label==row['outcome'] and predicted==row['predicted']
        assert sum(s!=expected for s in starts)==len(row['wrong_parent_starts'])
        recount[(row['group'],label)]+=1
    groups={}
    for group in GROUPS:
        ids=sorted(q for q,r in queries.items() if r['group']==group and r['supported'])
        pair=dict(count=len(ids),gains=[],losses=[],sides={})
        for side,rows in [('default',before),('modal',after)]:
            selected=[rows[q] for q in ids]
            pair['sides'][side]=dict(outcomes=dict(Counter(r['outcome'] for r in selected)),
                accepted_starts=sum(len(r['starts']) for r in selected),
                wrong_parent_starts=sum(len(r['wrong_parent_starts']) for r in selected),
                repeated_parent_starts=sum(r['repeated_parent_starts'] for r in selected))
        for q in ids:
            a,b=(rows[q]['outcome'] in ['correct','correct-rejection'] for rows in [before,after])
            if b and not a:pair['gains'].append(q)
            if a and not b:pair['losses'].append(q)
        if group!='negative':
            pair['uncertainty']=paired_interval([int(after[q]['outcome']=='correct')-
                                                 int(before[q]['outcome']=='correct') for q in ids])
        groups[group]=pair
    checks=dict(
        triggering_misses_recovered=all(after[q]['outcome']=='correct' for q in ['exact-t18','exact-t22']),
        clean_targets_met=all(groups[g]['sides']['modal']['outcomes'].get('correct',0)==22 for g in ['exact','clean']),
        more_mix_correct=sum(len(groups[g]['gains'])-len(groups[g]['losses']) for g in ['frozen','mix'])>0,
        no_lost_correct=all(not p['losses'] for p in groups.values()),
        no_added_wrong_starts=all(Counter(s['parent_song'] for s in after[q]['wrong_parent_starts']) <=
                                 Counter(s['parent_song'] for s in before[q]['wrong_parent_starts']) for q in queries))
    summary=dict(status='verified-execution',native_runs=96,groups=groups,acceptance_checks=checks,
                 exploratory_acceptance=all(checks.values()),binary=meta['binary'],manifest=meta['manifest'],
                 unchanged_report_evidence_pairs=96,
                 independent_recount=[dict(group=g,outcome=o,count=n) for (g,o),n in sorted(recount.items())])
    e.save(ROOT/'modal-summary.json',summary)
    e011=json.loads((ROOT/'summary.json').read_text())
    lines=['# E012 results: modal fitting with complete-song references','',
        'The only candidate change is the existing `--modal-fit` option. All 96 candidate runs use '
        'the same executable, complete-song database, query bytes and thresholds as their E011 full-song baselines.','',
        '| Query set | Queries | Independent chunks | Complete songs, default | Complete songs, modal |',
        '| --- | ---: | ---: | ---: | ---: |']
    for group,pair in groups.items():
        values=[]
        for counts in [e011['groups'][group]['sides']['chunks']['outcomes'],
                       pair['sides']['default']['outcomes'],pair['sides']['modal']['outcomes']]:
            if group=='negative':
                values.append(f"{counts.get('correct-rejection',0)} rejected; {counts.get('false-accept',0)} false")
            else:
                values.append(f"{counts.get('correct',0)} correct; {counts.get('wrong-match',0)} wrong; {counts.get('no-match',0)} no match")
        lines.append(f"| {group} | {pair['count']} | {' | '.join(values)} |")
    lines += ['','**Predeclared exploratory acceptance:** '+('passes' if summary['exploratory_acceptance'] else 'fails')+'.','',
              '| Gate | Result |','| --- | --- |']
    lines += [f"| {name} | {'pass' if passed else 'fail'} |" for name,passed in checks.items()]
    lines += ['','## Paired gains and losses','']
    for group,pair in groups.items():
        lines.append(f"- {group}: gains {', '.join(pair['gains']) or 'none'}; losses {', '.join(pair['losses']) or 'none'}.")
    lines += ['','Descriptive paired track bootstrap: 10,000 resamples, seed 20260913; conditional on this known '
              'mix and its algorithmic labels. Annotation error and generalization are not covered.','',
              '| Set | Modal minus default, percentage points | Descriptive 95% interval |','| --- | ---: | ---: |']
    for group in GROUPS[:-1]:
        u=groups[group]['uncertainty'];lo,hi=u['interval_95_percentage_points']
        lines.append(f"| {group} | {u['delta_percentage_points']:+.1f} | [{lo:+.1f}, {hi:+.1f}] |")
    lines += ['','## Scope','',
        'These are development results on one known mix. The random mix timestamps were frozen before E011, '
        'but E012 was motivated by E011’s exposed clean-control failures. This is not a held-out final evaluation. '
        'The 22 mix labels have independent algorithmic alignment support, not human cue verification. '
        'The corrected t05 catalogue and all sampling restrictions are documented in E011.','',
        'The first random mix query has possible overlapping-track/cue ambiguity: independent audio alignment '
        'supports both t01 and t02 near its timestamp. Its frozen expected label t01 and resulting scores '
        'remain unchanged; see E011 for the post-outcome diagnostic.','',
        'Modal fitting changes point estimates, keeping the winning neighbourhood’s evidence and the confidence/alignment gates. '
        'Recovery would support an alignment-estimation explanation for affected cases; it would not prove every default miss '
        'has the same cause or that reported positions are always the correct occurrence of repeated music. '
        'The option remains opt-in and no default/native implementation is changed.','',
        'Eight known negative clips provide only 80 seconds of nonmatching music per configuration. '
        'Every wrong-parent accepted start is retained below, including those accompanying a correct top prediction. '
        'No production false-alarm-rate or speed claim follows.','',
        '## Accepted starts','',
        '| Set | Default starts / wrong / repeated | Modal starts / wrong / repeated |','| --- | ---: | ---: |']
    for group,pair in groups.items():
        values=[' / '.join(str(pair['sides'][side][k]) for k in ['accepted_starts','wrong_parent_starts','repeated_parent_starts'])
                for side in ['default','modal']]
        lines.append(f"| {group} | {values[0]} | {values[1]} |")
    lines += ['','## Every paired query','', '| Query | Default | Modal |','| --- | --- | --- |']
    for q in sorted(queries,key=lambda q:(GROUPS.index(queries[q]['group']),q)):
        labels=[]
        for rows in [before,after]:
            r=rows[q]
            labels.append(r['outcome']+(f" ({r['predicted']})" if r['predicted'] else ''))
        lines.append(f"| {q} | {labels[0]} | {labels[1]} |")
    lines += ['','See [E012 protocol](E012-full-modal.md), [E011 results](E011-results.md) '
              'and [preserved raw evidence](../evidence/E011/README.md).','']
    diagnostic=ROOT/'matched-source-controls/summary.json'
    if diagnostic.exists():
        d=json.loads(diagnostic.read_text())
        assert d['status']=='verified-execution'
        lines += ['## Matched-source diagnostic','',
                  f"All {d['correct']}/{d['runs']} approximately corresponding clean-source clips are recognized; "
                  'their random mix queries remain no-match. This supports investigating fingerprint survival in mixed audio, '
                  'without isolating a particular DJ effect. These adaptive controls are separate from the main matrix. '
                  'See [positions, method and evidence](E012-matched-source.md).','']
    Path('docs/experiments/E012-results.md').write_text('\n'.join(lines))
    print(json.dumps(dict(groups=groups,acceptance_checks=checks),indent=2))


if __name__=='__main__':
    main()
