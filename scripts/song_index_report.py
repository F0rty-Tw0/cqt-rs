#!/usr/bin/env python3
"""Audit and recount all frozen E011 outcomes from native logs."""
import argparse
from collections import Counter
import json
from pathlib import Path
import numpy as np

import real_mix_eval as e
from song_index_eval import outcome, validate_events

ROOT = Path('target/song-index')
GROUPS = ['exact','clean','frozen','mix','negative']


def paired_interval(differences):
    """Descriptive track bootstrap, conditional on this known mix and labels."""
    values=np.asarray(differences,dtype=float)
    rng=np.random.default_rng(20260913)
    draws=values[rng.integers(0,len(values),size=(10000,len(values)))].mean(axis=1)*100
    return dict(delta_percentage_points=float(values.mean()*100),
                interval_95_percentage_points=[float(x) for x in np.quantile(draws,[.025,.975])],
                method='paired track bootstrap; 10000 resamples; seed 20260913; conditional descriptive interval')


def independent_prediction(events, refs):
    """Independent raw-event ordering; do not call the experiment's scorer."""
    selected = None
    selected_order = None
    starts = []
    for event in events:
        if event['event'] != 'start':
            continue
        parent = refs[event['song']]['song']
        starts.append(parent)
        order = (-event['evidence'], event['consumed'], event['song'])
        if selected_order is None or order < selected_order:
            selected_order = order
            selected = parent
    return selected, starts


def report(args):
    run_dir = ROOT/'runs'/args.label
    metadata = json.loads((run_dir/'metadata.json').read_text())
    assert metadata['status'] == 'completed'
    for field in ['binary','manifest','evaluator','scorer','helpers']:
        record = metadata[field]
        assert e.sha(record['path']) == record['sha256'], (field,'identity changed')
    manifest = json.loads(Path(metadata['manifest']['path']).read_text())
    assert e.sha(manifest['evaluator']['path'])==manifest['evaluator']['sha256']
    for rows in [*manifest['references'].values(),manifest['queries']]:
        for row in rows:
            assert e.sha(row['path']) == row['sha256'], row['path']
    references = {side:{r['id']:r for r in refs} for side,refs in manifest['references'].items()}
    queries = {q['id']:q for q in manifest['queries']}
    results = json.loads((run_dir/'results.json').read_text())
    assert len(results)==2*len(queries)==metadata['completed_runs']
    assert {(r['query_id'],r['side']) for r in results} == {(q,s) for q in queries for s in references}
    recounted = Counter()
    for row in results:
        assert row['status']=='completed' and row['returncode']==0
        for field in ['stdout','stderr']:
            assert e.sha(row[field]['path'])==row[field]['sha256'], row['id']
        query = queries[row['query_id']]
        assert row['input']==query
        assert row['supported']==query['supported']
        assert e.wav_info(query['path'])['samples']==441000
        expected_command = [metadata['binary']['path']]
        for ref in manifest['references'][row['side']]:
            expected_command += ['--watch',ref['id']+'='+ref['path']]
        expected_command += ['--stream',query['path']]
        assert row['command']==expected_command, ('command differs',row['id'])
        events = [json.loads(line) for line in Path(row['stdout']['path']).read_text().splitlines()]
        refs = references[row['side']]
        validate_events(events,refs,query)
        replay = outcome(events,refs,query['song'])
        for key,value in replay.items():
            assert row[key]==value, (row['id'],key)
        predicted,starts = independent_prediction(events,refs)
        if query['song'] is None:
            label = 'false-accept' if starts else 'correct-rejection'
        else:
            label = 'no-match' if predicted is None else ('correct' if predicted==query['song'] else 'wrong-match')
        assert label==row['outcome'] and predicted==row['predicted']
        assert len(starts)==len(row['starts'])
        assert sum(song!=query['song'] for song in starts)==len(row['wrong_parent_starts'])
        recounted[(row['group'],row['side'],label)] += 1

    groups = {}
    lookup = {(r['query_id'],r['side']):r for r in results}
    for group in GROUPS:
        qs = [q for q in manifest['queries'] if q['group']==group and q['supported']]
        pair = dict(count=len(qs), gains=[],losses=[], sides={})
        for side in references:
            rows = [lookup[(q['id'],side)] for q in qs]
            pair['sides'][side] = dict(outcomes=dict(Counter(r['outcome'] for r in rows)),
                accepted_starts=sum(len(r['starts']) for r in rows),
                wrong_parent_starts=sum(len(r['wrong_parent_starts']) for r in rows),
                repeated_parent_starts=sum(r['repeated_parent_starts'] for r in rows))
        for q in qs:
            before,after = (lookup[(q['id'],s)]['outcome'] in ['correct','correct-rejection'] for s in ['chunks','full'])
            if after and not before: pair['gains'].append(q['id'])
            if before and not after: pair['losses'].append(q['id'])
        if group!='negative':
            pair['uncertainty']=paired_interval([
                int(lookup[(q['id'],'full')]['outcome']=='correct')-
                int(lookup[(q['id'],'chunks')]['outcome']=='correct') for q in qs])
        groups[group]=pair
    more_mix_correct = sum(len(groups[g]['gains'])-len(groups[g]['losses']) for g in ['frozen','mix']) > 0
    no_losses = all(not group['losses'] for group in groups.values())
    no_added_wrong = all(
        Counter(s['parent_song'] for s in lookup[(q,'full')]['wrong_parent_starts']) <=
        Counter(s['parent_song'] for s in lookup[(q,'chunks')]['wrong_parent_starts'])
        for q in queries)
    index = {}
    for side in references:
        rows = [r['index'] for r in results if r['side']==side]
        index[side] = {k:rows[0][k] for k in ['songs','hashes','dropped','bytes']}
        assert all(all(row[k]==value for k,value in index[side].items()) for row in rows)
    summary = dict(status='verified-execution',native_runs=len(results),query_count=len(queries),
        groups=groups,index=index,source_commit=metadata['source_commit'],binary_sha256=metadata['binary']['sha256'],
        manifest=e.identity(ROOT/'manifest.json'),
        exploratory_preference_for_full=more_mix_correct and no_losses and no_added_wrong,
        acceptance_checks=dict(more_mix_correct=more_mix_correct,no_losses=no_losses,no_added_wrong_starts=no_added_wrong),
        clean_target_met={s:all(groups[g]['sides'][s]['outcomes'].get('correct',0)==22 for g in ['exact','clean']) for s in references},
        independent_recount=[dict(group=g,side=s,outcome=o,count=n) for (g,s,o),n in sorted(recounted.items())])
    e.save(ROOT/'summary.json',summary)

    text = ['# E011 results: independent chunks versus complete songs','',
        'Both layouts use the corrected 22-song catalogue, the same pinned executable and unchanged default gates. '
        'Every query is ten seconds in a fresh process. All 192 native runs completed.','',
        '| Query set | Queries | Chunks: correct / wrong / no match | Full songs: correct / wrong / no match |',
        '| --- | ---: | ---: | ---: |']
    for group in GROUPS:
        p=groups[group]
        values=[]
        for side in ['chunks','full']:
            counts=p['sides'][side]['outcomes']
            if group=='negative':
                values.append(f"{counts.get('correct-rejection',0)} rejected / {counts.get('false-accept',0)} falsely accepted")
            else:
                values.append(' / '.join(str(counts.get(k,0)) for k in ['correct','wrong-match','no-match']))
        text.append(f"| {group} | {p['count']} | {values[0]} | {values[1]} |")
    text += ['', '**Exploratory preference rule:** '+('passes' if summary['exploratory_preference_for_full'] else 'fails')+'. '
             'Requires more correct supported mix queries, no lost correct query, and no added wrong-parent starts in any set.','',
             '## Source correction and interpretation','',
             'E010’s t05 download was mislabeled: the linked file is Exodus (original mix) by Marc Burt. '
             'The corrected Roots and Shoots by Dave Kent comes from the archive embedded on the publisher’s release page. '
             'Five consistent independent STFT anchors support its occurrence around 791–852 seconds. '
             'All 22 mix labels now have algorithmic support; these are not human-verified annotations.','',
             'Correcting t05 changes the catalogue from E010. The paired E011 layouts use identical corrected recordings; '
             'do not ascribe a difference from the historical E010 score solely to reference layout. '
             'The complete-song treatment changes evidence grouping, boundary context and repeated-hash cap scope together. '
             'It does not independently prove which mechanism causes a particular miss.','',
             'Random-clean cuts are uniform across legal source sample positions. Random-mix cuts are uniform across '
             'legal start samples inside supported anchor intervals, not across entire publisher track intervals. '
             'The frozen set preserves E010’s actual query bytes, including its interpolated t01 cue. '
             'Exact clips are the middle complete chunk of each source. No query was discarded for being difficult.','',
             'The random t01 query at 59.594195 seconds is scored against its frozen expected identity t01. '
             'A post-outcome STFT diagnostic finds consistent t01 and t02 correspondences near this query: '
             't01 anchors include [52,72], while adjacent t02 anchors cover [51,69] and [60,80] seconds. '
             'This is compatible with overlapping tracks or cue ambiguity. The wrong top identity under the frozen '
             'single-label protocol must not be interpreted as a proven unrelated-song false alarm. '
             'The query and its scored failure remain visible; no post-hoc relabeling or exclusion is applied.','',
             'Negative exposure is eight known nonmatching music clips, 80 seconds per layout. '
             'This is a small development control, not a production false-alarm-rate estimate. '
             'The known mix, originals and related queries are not a recording-disjoint final evaluation.','',
             '## Paired changes','']
    for group in GROUPS:
        p=groups[group]
        text += [f"- {group}: gains {', '.join(p['gains']) or 'none'}; losses {', '.join(p['losses']) or 'none'}."]
    text += ['','Descriptive paired track-bootstrap intervals condition on the 22 tracks and algorithmic labels '
             'in this one known mix (10,000 resamples, seed 20260913). They do not cover annotation error or generalization.','',
             '| Set | Full minus chunks, percentage points | Descriptive 95% interval |','| --- | ---: | ---: |']
    for group in GROUPS[:-1]:
        u=groups[group]['uncertainty'];lo,hi=u['interval_95_percentage_points']
        text.append(f"| {group} | {u['delta_percentage_points']:+.1f} | [{lo:+.1f}, {hi:+.1f}] |")
    text += ['','## Index and accepted starts','', '| Layout | Records | Stored hashes | Dropped occurrences | Approximate index bytes |',
             '| --- | ---: | ---: | ---: | ---: |']
    for side,row in index.items():
        text.append(f"| {side} | {row['songs']:,} | {row['hashes']:,} | {row['dropped']:,} | {row['bytes']:,} |")
    text += ['','Index bytes are the native index estimate, not peak process memory. Concurrent run times are not speed evidence.','',
             '| Set | Chunk starts / wrong / repeated | Full starts / wrong / repeated |','| --- | ---: | ---: |']
    for group,p in groups.items():
        values=[' / '.join(str(p['sides'][s][k]) for k in ['accepted_starts','wrong_parent_starts','repeated_parent_starts']) for s in ['chunks','full']]
        text.append(f"| {group} | {values[0]} | {values[1]} |")
    text += ['','## Every query','', '| Query | Source or mix start | Chunks | Full songs |','| --- | ---: | --- | --- |']
    for q in sorted(queries.values(),key=lambda q:(GROUPS.index(q['group']),q['id'])):
        values=[]
        for side in ['chunks','full']:
            r=lookup[(q['id'],side)]
            values.append(r['outcome']+(f" ({r['predicted']})" if r['predicted'] else ''))
        text.append(f"| {q['id']}: {q['title']} | {q['start_seconds']:.6f}s | {values[0]} | {values[1]} |")
    text += ['','See [protocol](E011-song-index.md) and [raw evidence](../evidence/E011/README.md).','']
    Path(args.report).write_text('\n'.join(text))
    print(json.dumps({k:summary[k] for k in ['native_runs','groups','acceptance_checks','clean_target_met']},indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label',default='default-a')
    parser.add_argument('--report',default='docs/experiments/E011-results.md')
    report(parser.parse_args())
