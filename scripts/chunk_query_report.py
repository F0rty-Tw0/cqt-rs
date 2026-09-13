#!/usr/bin/env python3
"""Recompute E010 outcomes from raw logs, validate identities, and write a report."""
import argparse
from collections import Counter
import json
from pathlib import Path
import numpy as np
from chunk_query_eval import score
import real_mix_eval as e

ROOT = Path('target/chunk-query')


def report(label):
    output = ROOT/'runs'/label
    meta = json.loads((output/'metadata.json').read_text())
    assert meta['status'] == 'completed', meta['status']
    rows = json.loads((output/'results.json').read_text())
    assert len(rows) == 44
    for key in ['binary', 'sources', 'queries']:
        assert e.sha(meta[key]['path']) == meta[key]['sha256'], key
    sources = json.loads((ROOT/'sources.json').read_text())
    frozen = json.loads((ROOT/'queries.json').read_text())
    refs = {
        'midpoint': {r['midpoint']['id']:r['midpoint'] for s,r in sources.items() if s != 'mix'},
        'chunks': {c['id']:c for s,r in sources.items() if s != 'mix' for c in r['chunks']},
    }
    checks = []
    for rows_to_check in [*refs.values(), {q['song']:q for q in frozen['queries']}]:
        for key,row in rows_to_check.items():
            assert e.sha(row['path']) == row['sha256'], key
    by_key = {}
    for row in rows:
        key = (row['song'],row['side'])
        assert key not in by_key
        for field in ['stdout','stderr']:
            assert e.sha(row[field]['path']) == row[field]['sha256'], (key,field)
        events = [json.loads(line) for line in Path(row['stdout']['path']).read_text().splitlines()]
        assert events[-1]['event'] == 'done' and events[-1]['audio_seconds'] == 10
        recalculated = score(events, refs[row['side']], row['song'])
        assert all(row[k] == value for k,value in recalculated.items()), key
        assert e.stable_digest(events) == row['event_digest'], key
        assert row['input']['sha256'] == next(q['sha256'] for q in frozen['queries'] if q['song']==row['song'])
        expected_names = set(refs[row['side']])
        indexed = [v for v in events if v['event']=='index']
        assert len(indexed) == len(expected_names)
        assert {v['song'] for v in indexed} == expected_names
        reports = [v for v in events if v['event']=='report']
        confident = [v for v in reports if v['confidence'] >= 70]
        diagnostics = dict(max_reported_evidence=max((v['evidence'] for v in reports), default=0),
                           max_reported_alignment_when_confident=max((v['verify_q'] for v in confident), default=None))
        by_key[key] = dict(**row, diagnostics=diagnostics)
        checks.append(dict(song=row['song'], side=row['side'], output_hashes=True, event_replay=True,
                           references=len(indexed), input_hash=row['input']['sha256']))
    supported = [q['song'] for q in frozen['queries'] if q['annotation']['status']=='algorithmically-supported']
    assert len(supported)==21
    counts = {side:dict(Counter(by_key[(song,side)]['outcome'] for song in supported)) for side in refs}
    gains = [s for s in supported if by_key[(s,'chunks')]['outcome']=='correct' and by_key[(s,'midpoint')]['outcome']!='correct']
    losses = [s for s in supported if by_key[(s,'midpoint')]['outcome']=='correct' and by_key[(s,'chunks')]['outcome']!='correct']
    delta = np.array([int(by_key[(s,'chunks')]['outcome']=='correct')-int(by_key[(s,'midpoint')]['outcome']=='correct') for s in supported])
    rng = np.random.default_rng(20260913)
    bootstrap = rng.choice(delta, size=(10000,len(delta)), replace=True).mean(axis=1)
    interval = [float(x) for x in np.quantile(bootstrap,[.025,.975])]
    passed = bool(gains) and not losses and counts['chunks'].get('wrong-match',0)<=counts['midpoint'].get('wrong-match',0)
    table = []
    for q in frozen['queries']:
        song = q['song']
        table.append(dict(song=song, title=q['title'], mix_start_seconds=q['mix_start_seconds'],
                          annotation=q['annotation']['status'],
                          midpoint={k:by_key[(song,'midpoint')][k] for k in ['outcome','predicted','best','diagnostics']},
                          chunks={k:by_key[(song,'chunks')][k] for k in ['outcome','predicted','best','diagnostics']}))
    summary = dict(status='verified-execution-with-one-annotation-gap', native_runs=44, supported_queries=21,
                   unverified_queries=['t05'], counts=counts, gains=gains, losses=losses,
                   paired_correct_fraction_delta=float(delta.mean()), paired_bootstrap_95_interval=interval,
                   uncertainty_scope='21 tracks in one known mix; not independent mixes or human-verified annotations',
                   exploratory_acceptance=passed, index={side:by_key[('t01',side)]['index'] for side in refs},
                   accepted_starts={side:sum(len(by_key[(s,side)]['starts']) for s in supported) for side in refs},
                   wrong_parent_starts={side:sum(len(by_key[(s,side)]['wrong_parent_starts']) for s in supported) for side in refs},
                   repeated_parent_starts={side:sum(by_key[(s,side)]['repeated_parent_starts'] for s in supported) for side in refs},
                   per_query=table, checks=checks, reporter=e.identity(__file__), metadata=e.identity(output/'metadata.json'))
    e.save(ROOT/'summary.json', summary)
    lines = ['# E010 results: full chunk coverage versus midpoint references', '',
             f"**{counts['midpoint'].get('correct',0)}/21 → {counts['chunks'].get('correct',0)}/21 supported song identities.** "
             'All 44 native runs completed on 22 actual ten-second mix clips. t05 has an unverified label and is excluded from this accuracy denominator.', '',
             '| Reference configuration | Correct | Wrong top identity | No match |',
             '| --- | ---: | ---: | ---: |']
    for side in refs:
        lines.append(f"| {side} | {counts[side].get('correct',0)} | {counts[side].get('wrong-match',0)} | {counts[side].get('no-match',0)} |")
    lines += ['', f"Gains: {', '.join(gains) or 'none'}. Lost correct identities: {', '.join(losses) or 'none'}. "
              f"The predeclared exploratory acceptance rule {'passes' if passed else 'fails'}.", '',
              f"Paired difference: {100*delta.mean():.1f} percentage points; descriptive track-bootstrap 95% interval "
              f"[{100*interval[0]:.1f}, {100*interval[1]:.1f}] points (10,000 resamples, seed 20260913). "
              'This conditions on 21 tracks from one known mix and algorithmic labels; it does not cover annotation error or generalization to new mixes.', '',
              '| Song | Mix cut starts | Midpoint | All chunks |', '| --- | ---: | --- | --- |']
    for row in table:
        song = row['song'] + (' (unverified)' if row['annotation']!='algorithmically-supported' else '')
        label = lambda side: row[side]['outcome'] + (f" ({row[side]['predicted']})" if row[side]['predicted'] else '')
        lines.append(f"| {song}: {row['title']} | {row['mix_start_seconds']:.1f}s | {label('midpoint')} | {label('chunks')} |")
    lines += ['', '## Interpretation and limits', '',
              'Both configurations use the same pinned executable, default gates and modal fit disabled. '
              'Every query is a fresh native process; each searches the entire reference set. '
              'The chunk configuration indexes 858 complete sections and 22 unpadded tails, mapped to 22 parent songs. '
              'The repeated-hash cap applies per chunk, so this treatment changes segmentation as well as reference coverage.', '',
              f"Accepted starts on supported queries: midpoint {summary['accepted_starts']['midpoint']}, chunks {summary['accepted_starts']['chunks']}. "
              f"Wrong-parent starts: {summary['wrong_parent_starts']['midpoint']} versus {summary['wrong_parent_starts']['chunks']}. "
              f"Repeated parent starts: {summary['repeated_parent_starts']['midpoint']} versus {summary['repeated_parent_starts']['chunks']}. "
              'Top prediction is the strongest accepted start, with deterministic ties; chunk scores are not summed.', '',
              f"Native index storage: {summary['index']['midpoint']['bytes']:,} bytes for midpoint and "
              f"{summary['index']['chunks']['bytes']:,} bytes for chunks. These are the executable's approximate index-byte estimates, not process peak RSS. "
              'Index construction occurs again in each process. Single-run timings and concurrent annotation/test work are not speed evidence.', '',
              'The mix clips are digital cuts, not microphone recordings. No negative music, crossfade-specific, pitch/EQ/voice treatment or fresh recording-disjoint evaluation is added here. '
              't05 remains an annotation gap; its provisional outcome must not be described as verified accuracy. '
              'Reported position includes the reference chunk offset, but repeated passages can make position ambiguous.', '',
              'Raw output hashes, terminal ten-second durations, index counts, frozen query identity and every outcome were checked again. '
              'Four truncated decoded caches were repaired to exact original hashes before final annotation; prepared native inputs kept their hashes. '
              'The overall research goal remains incomplete.', '',
              'See [protocol and reproduction](E010-chunk-queries.md) and [evidence](../evidence/E010/README.md).', '']
    Path('docs/experiments/E010-results.md').write_text('\n'.join(lines))
    print(json.dumps({k:v for k,v in summary.items() if k not in ['per_query','checks']}, indent=2))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--label', default='default-a')
    report(parser.parse_args().label)
