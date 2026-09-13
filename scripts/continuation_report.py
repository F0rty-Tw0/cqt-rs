#!/usr/bin/env python3
"""Independently recount E014 native starts, paired losses, positions and timing."""
import hashlib
import json
from pathlib import Path
import statistics
import zipfile
import numpy as np
import continuation_eval as c
from song_index_eval import outcome

ROOT=c.ROOT
GROUPS=('exact','clean','frozen','mix','negative')


def digest(data): return hashlib.sha256(data).hexdigest()
def good(r): return r['outcome'] in ('correct','correct-rejection')
def values(rows,field): return [r[field] for r in rows if r.get(field) is not None]
def stats(data):
    return dict(n=len(data),median=statistics.median(data),p95=float(np.quantile(data,.95)),
                minimum=min(data),maximum=max(data)) if data else dict(n=0)


def bootstrap(pairs):
    if not pairs:return None
    differences=np.array([int(good(b))-int(good(a)) for a,b in pairs])
    rng=np.random.default_rng(1402026)
    means=differences[rng.integers(len(pairs),size=(10000,len(pairs)))].mean(axis=1)
    return dict(delta=float(differences.mean()),low=float(np.quantile(means,.025)),
                high=float(np.quantile(means,.975)),scope='known-query paired descriptive bootstrap')


def paired(old,new):
    pairs=[(old[k],new[k]) for k in sorted(old.keys() & new.keys())]
    return dict(n=len(pairs),before=sum(good(a) for a,b in pairs),after=sum(good(b) for a,b in pairs),
                gains=[a['query_id'] for a,b in pairs if not good(a) and good(b)],
                losses=[a['query_id'] for a,b in pairs if good(a) and not good(b)],
                wrong_before=sum(len(a['wrong_parent_starts']) for a,b in pairs),
                wrong_after=sum(len(b['wrong_parent_starts']) for a,b in pairs),
                duplicates_before=sum(a['repeated_parent_starts'] for a,b in pairs),
                duplicates_after=sum(b['repeated_parent_starts'] for a,b in pairs),
                bootstrap=bootstrap(pairs))


def controlled(events,truth):
    # Pair events chronologically, then assign every start once to its known
    # song/play interval. This recount does not call the runner's scorer.
    active={};segments=[]
    for event in events:
        if event['event']=='start':
            assert event['song'] not in active, ('duplicate active start',event)
            active[event['song']]=event
        elif event['event']=='end':
            assert event['song'] in active, ('end without start',event)
            segments.append((active.pop(event['song']),event))
    assert not active, 'missing final ends'
    assigned={p['play_id']:[] for p in truth};false=[]
    for s,e in segments:
        matches=[p for p in truth if p['song']==s['song'] and p['start']<=s['t']<=p['end']]
        if not matches:false.append(s)
        else:
            assert len(matches)==1
            assigned[matches[0]['play_id']].append((s,e))
    plays=[]
    for p in truth:
        matches=sorted(assigned[p['play_id']],key=lambda pair:pair[0]['t'])
        s,e=matches[0] if matches else (None,None)
        plays.append(dict(p,detected=bool(matches),starts=len(matches),
            notification_start_delay=s['consumed']-p['start'] if s else None,
            notification_end_delay=e['consumed']-p['end'] if e else None,
            estimated_start_error=s['estimated_start']-p['start'] if s and 'estimated_start' in s else None,
            estimated_end_error=e['estimated_end']-p['end'] if e and 'estimated_end' in e else None,
            premature=bool(e and e['t']<p['end']),first=s,end_event=e))
    return dict(total=len(plays),detected=sum(p['detected'] for p in plays),
                misses=[p['play_id'] for p in plays if not p['detected']],
                duplicate_starts=sum(max(0,p['starts']-1) for p in plays),
                false_starts=false,premature_endings=sum(p['premature'] for p in plays),plays=plays,
                timing={field:stats(values(plays,field)) for field in
                        ('notification_start_delay','notification_end_delay','estimated_start_error','estimated_end_error')})


def detail_lines(results, refs):
    lines=['', '| Song | Long segments | Continuation segments | Single-hypothesis segments |',
           '| --- | ---: | ---: | ---: |']
    for song in refs:
        lines.append('| '+song+' | '+' | '.join(str(results.get('full-mix-'+a,{}).get('counts',{}).get(song,'missing'))
                     for a in ('long','continuation','single'))+' |')
    lines += ['', '## Timing and source-position limits', '',
              '| Programme / arm | Start notification median / p95 (s) | End notification median / p95 (s) | Supported start error min / max (s) | Supported end error min / max (s) |',
              '| --- | ---: | ---: | ---: | ---: |']
    for key,r in results.items():
        if r['group']!='programme':continue
        timing=r['score']['timing'];cells=[]
        for field in ('notification_start_delay','notification_end_delay','estimated_start_error','estimated_end_error'):
            z=timing[field]
            cells.append(f"{z['median']:.3f} / {z['p95']:.3f}" if z['n'] and field.startswith('notification') else
                         f"{z['minimum']:.3f} / {z['maximum']:.3f}" if z['n'] else '—')
        lines.append('| '+key+' | '+' | '.join(cells)+' |')
    lines += ['', '| Clean source positions / arm | Correct starts evaluated | Within 2 s | Median absolute error (s) | Worst absolute error (s) |',
              '| --- | ---: | ---: | ---: | ---: |']
    for group in ('exact','clean'):
        for arm in ('long','continuation'):
            selected=[r for k,r in results.items() if r['group']==group and k==r['query_id']+'-'+arm]
            errors=[r['source_position_error'] for r in selected if r.get('source_position_error') is not None]
            if errors:lines.append(f"| {group} / {arm} | {len(errors)} | {sum(x<=2 for x in errors)} | {statistics.median(errors):.3f} | {max(errors):.3f} |")
    lines += ['', 'Repeated source passages can produce the right identity at another location. '
              'Supported interval edges are coarse evidence estimates, not audible ground truth.', '',
              '## Robustness misses and paired regressions', '',
              '| Treatment | Long misses | Continuation misses | Lost previously detected plays |',
              '| --- | --- | --- | --- |']
    for case in c.TREATMENTS:
        old=results.get(case+'-long',{}).get('score');new=results.get(case+'-continuation',{}).get('score')
        if old and new:
            lost=sorted(set(new['misses'])-set(old['misses']))
            lines.append('| '+case+' | '+(', '.join(old['misses']) or '—')+' | '+(', '.join(new['misses']) or '—')+' | '+(', '.join(lost) or '—')+' |')
    return lines


def main():
    m=c.read(ROOT/'manifest.json');refs={r['id']:r for r in m['references']}
    queries={q['id']:q for q in m['queries']};results={};raw={}
    for path in sorted((ROOT/'runs').glob('*.result.json')):
        record=c.read(path)
        c.validate_resume(record,record['signature'])
        data=Path(record['stdout']['path']).read_bytes()
        events=list(map(json.loads,data.splitlines()))
        assert events[-1]['event']=='done'
        raw[record['id']]=events
        q=record['input'];result=dict(query_id=q['id'],id=record['id'],group=q['group'],arm=record['arm'])
        if q['group'] in GROUPS:
            result.update(outcome(events,refs,q['song']))
            if q['group'] in ('exact','clean'):
                correct=[s for s in result['starts'] if s['song']==q['song']]
                best=sorted(correct,key=lambda s:(-s['evidence'],s['consumed']))[0] if correct else None
                result['source_position_error']=abs(best['position']-(q['start_seconds']+best['t'])) if best else None
        elif q['group'] in ('programme','robustness'):
            result['score']=controlled(events,q['intervals'])
            for k in ('total','detected','duplicate_starts','premature_endings'):
                assert result['score'][k]==record['score'][k],(record['id'],k)
            assert sorted(result['score']['false_starts'],key=lambda x:(x['t'],x['song']))==sorted(record['score']['false_starts'],key=lambda x:(x['t'],x['song']))
        else:
            score=controlled(events,[]) # pairs all events, but no cue truth
            starts=[e for e in events if e['event']=='start'];ends=[e for e in events if e['event']=='end']
            spans=[]
            for s in starts:
                e=next(e for e in ends if e['song']==s['song'] and e['t']>=s['t'])
                spans.append(dict(song=s['song'],start=s['t'],end=e['t']))
            coverage={}
            for group in ('frozen','mix'):
                coverage[group]=[q['song'] for q in queries.values() if q['group']==group and
                    any(p['song']==q['song'] and p['start']<q['start_seconds']+q['seconds']
                        and p['end']>q['start_seconds'] for p in spans)]
            result.update(segments=spans,songs=sorted({s['song'] for s in starts}),coverage=coverage,
                          counts={song:sum(s['song']==song for s in starts) for song in refs})
        results[record['id']]=result
    expected={q['id']+'-'+a for q in m['queries'] for a in ('long','continuation')}
    expected.update('programme-'+a for a in c.ARMS)
    expected.update('phase-'+a for a in ('long','continuation'))
    expected.update('full-mix-'+a for a in ('long','continuation','single'))
    expected.update(q+'-'+a for q in c.TREATMENTS for a in ('long','continuation'))
    expected.update(['clean-t01-legacy-baseline','programme-legacy-baseline','clean-t01-legacy',
                     'programme-continuation-block257','programme-continuation-block65536',
                     'clean-t01-continuation-live'])
    assert set(results)==expected, ('incomplete or unexpected run set', sorted(expected-set(results)), sorted(set(results)-expected))
    assert (ROOT/'parity.json').exists(), 'parity gate incomplete'
    legacy={}
    for group in GROUPS:
        with zipfile.ZipFile('docs/evidence/E011/modal-'+group+'.zip') as z:
            for name in z.namelist():
                if not name.endswith('.result.json'):continue
                r=json.loads(z.read(name));key=r['query_id'];q=queries[key]
                assert r['input']['sha256']==q['sha256']
                data=z.read(Path(r['stdout']['path']).name)
                assert digest(data)==r['stdout']['sha256']
                scored=outcome(list(map(json.loads,data.splitlines())),refs,q['song'])
                assert scored['outcome']==r['outcome']
                legacy[key]=dict(scored,query_id=key)
    comparisons=[]
    for group in GROUPS:
        old={k:r for k,r in legacy.items() if queries[k]['group']==group}
        arms={a:{q['id']:results[q['id']+'-'+a] for q in queries.values()
                 if q['group']==group and q['id']+'-'+a in results} for a in ('long','continuation')}
        for a,b in [('legacy','long'),('legacy','continuation'),('long','continuation')]:
            comparisons.append(dict(group=group,before_arm=a,after_arm=b,
                                    **paired(old if a=='legacy' else arms[a],arms[b])))
    summary=dict(runs=len(results),comparisons=comparisons,results=results,
                 legacy=legacy,parity=c.read(ROOT/'parity.json') if (ROOT/'parity.json').exists() else None,
                 hashes={p.name:c.e.sha(p) for p in (ROOT/'runs').glob('*.jsonl')})
    c.save(ROOT/'audit.json',summary)
    lines=['# E014 results: longer retrieval and two-second continuation', '',
           'Status: native experiment completed; acceptance decision below.', '',
           'All results use the frozen corrected catalogue and query labels. '
           'These are known recordings; synthetic speech is not a test of general human voiceover.', '',
           '## Frozen ten-second queries', '',
           '| Group | Legacy modal | Long context | Continuation |',
           '| --- | ---: | ---: | ---: |']
    for group in GROUPS:
        old=sum(good(r) for k,r in legacy.items() if queries[k]['group']==group)
        cells=[str(old)]
        for a in ('long','continuation'):
            selected=[r for k,r in results.items() if r['group']==group and k==r['query_id']+'-'+a]
            cells.append(f'{sum(good(r) for r in selected)}/{len(selected)}')
        lines.append('| '+group+' | '+' | '.join(cells)+' |')
    lines += ['', 'Negative cells count rejected queries. Other cells count the strongest accepted '
              'identity under the unchanged scoring rule. Correct identity does not guarantee position.', '',
              '| Comparison / group | Gains | Lost identities | Wrong starts before / after | Duplicates before / after |',
              '| --- | --- | --- | ---: | ---: |']
    for row in comparisons:
        lines.append(f"| {row['before_arm']} → {row['after_arm']} / {row['group']} | {', '.join(row['gains']) or '—'} | {', '.join(row['losses']) or '—'} | {row['wrong_before']} / {row['wrong_after']} | {row['duplicates_before']} / {row['duplicates_after']} |")
    lines += ['', '## Continuous programmes and robustness', '',
              '| Case / arm | Detected plays | Duplicates | False starts | Premature endings |',
              '| --- | ---: | ---: | ---: | ---: |']
    for key,r in results.items():
        if 'score' not in r:continue
        s=r['score'];lines.append(f"| {key} | {s['detected']}/{s['total']} | {s['duplicate_starts']} | {len(s['false_starts'])} | {s['premature_endings']} |")
    lines += ['', 'All start events are assigned once. Misses remain in the denominator. '
              'Timing summaries are conditional on a detected play and remain separate from recall.', '',
              '## Complete real DJ mix', '',
              '| Arm | Songs detected | Segments | Frozen locations covered | Random locations covered |',
              '| --- | ---: | ---: | ---: | ---: |']
    for a in ('long','continuation','single'):
        r=results.get('full-mix-'+a)
        if r:lines.append(f"| {a} | {len(r['songs'])}/22 | {len(r['segments'])} | {len(r['coverage']['frozen'])}/22 | {len(r['coverage']['mix'])}/22 |")
    lines += detail_lines(results, refs)
    lines += ['', 'Coverage means a logical active interval overlaps the frozen query location; '
              'it is not an audible-boundary annotation. Every segment is retained in the audit.', '',
              '## Every frozen and random mix-query outcome', '',
              '| Query | Legacy | Long context | Continuation |', '| --- | --- | --- | --- |']
    for q in queries.values():
        if q['group'] not in ('frozen','mix'):continue
        key=q['id'];lines.append('| '+key+' | '+legacy[key]['outcome']+' | '+' | '.join(results.get(key+'-'+a,{}).get('outcome','missing') for a in ('long','continuation'))+' |')
    rejected=any(r['losses'] or r['wrong_after']>r['wrong_before'] or r['duplicates_after']>r['duplicates_before']
                 for r in comparisons if r['after_arm']=='continuation')
    for case in ['programme','phase',*c.TREATMENTS]:
        old=results.get(case+'-long',{}).get('score');new=results.get(case+'-continuation',{}).get('score')
        if old and new: rejected |= new['detected']<old['detected'] or new['duplicate_starts']>old['duplicate_starts'] or len(new['false_starts'])>len(old['false_starts'])
    lines += ['', '## Decision', '', 'Reject a default switch; retain the prototype as opt-in.' if rejected else
              'The measured no-regression comparison passes; this development corpus does not justify a default switch or general accuracy claim.', '',
              '100% matching under the requested range of mix transformations and voiceover remains unproved. '
              'Confidence is a decision score, not a probability. No performance, exact-boundary or production false-alarm claim.', '']
    summary['default_switch']='rejected' if rejected else 'development-gates-pass-generalization-unproved'
    c.save(ROOT/'audit.json',summary)
    (ROOT/'results.md').write_text('\n'.join(lines))
    print(json.dumps(dict(runs=len(results),decision=summary['default_switch'],comparisons=comparisons),indent=2))


if __name__=='__main__':main()
