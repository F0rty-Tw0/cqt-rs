#!/usr/bin/env python3
"""E015 independent geometry oracle over unchanged native peaks and triplets."""
from collections import Counter
import gzip
import itertools
import json
from pathlib import Path
import statistics
import numpy as np
from scipy.spatial import cKDTree
import survival_extract as x

FPS=44100/256
ROOT=x.ROOT
MARGIN=.15


def load_native(key,check_triplets=True):
    meta=x.read(ROOT/'native'/(key+'.json'))
    assert meta['status']=='completed' and meta['signature']['binary']==x.BINARY_SHA
    assert x.sha(meta['stdout']['path'])==meta['stdout']['sha256']
    assert x.sha(meta['input']['path'])==meta['input']['sha256']
    assert x.sha(meta['compressed']['path'])==meta['compressed']['sha256']
    with gzip.open(meta['compressed']['path'],'rb') as f:
        import hashlib
        assert hashlib.sha256(f.read()).hexdigest()==meta['stdout']['sha256']
    peaks=[];hashes=[]
    with open(meta['stdout']['path']) as f:
        for line in f:
            tag,*values=line.split();(peaks if tag=='P' else hashes).append(tuple(map(int,values)))
    assert len(peaks)==meta['peaks'] and len(hashes)==meta['hashes']
    assert peaks==sorted(set(peaks))
    if check_triplets:
        sentinel=object()
        for n,(a,b) in enumerate(itertools.zip_longest(triplets(peaks),hashes,fillvalue=sentinel)):
            assert a==b,(key,'triplet mismatch',n,a,b)
    return np.asarray(peaks,dtype=np.int64),np.asarray(hashes,dtype=np.int64),meta


def triplets(peaks):
    # Independent offline construction, compared against every emitted native H.
    for i,(t1,b1) in enumerate(peaks):
        partners=[p for p in peaks[i+1:i+13] if p[0]<=t1+320]
        for j,(t2,b2) in enumerate(partners):
            if t2==t1:continue
            for t3,b3 in partners[j+1:j+5]:
                if t3==t2:continue
                ratio=round(32*(t2-t1)/(t3-t1))
                yield ((b2-b1+512)<<20 | (b3-b2+512)<<10 | (ratio+8),t1,b1,t3-t1)


def nearest_mask(points,targets,scales):
    if not len(targets):return np.zeros(0,dtype=bool)
    if not len(points):return np.zeros(len(targets),dtype=bool)
    distances,_=cKDTree(np.asarray(points,dtype=float)/scales).query(np.asarray(targets,dtype=float)/scales,k=1,p=np.inf)
    return distances<=1


def verify(reference,query,tempo,offset,shift):
    if not len(query):return dict(query_peaks=0,query_matched=0,reference_peaks=0,reference_matched=0,verify_q=0.,verify_r=0.)
    lo=tempo*query[0,0]+offset-4;hi=tempo*query[-1,0]+offset+4
    ref=reference[np.searchsorted(reference[:,0],lo):np.searchsorted(reference[:,0],hi,side='right')]
    targets=query.astype(float).copy();targets[:,0]=tempo*targets[:,0]+offset;targets[:,1]-=shift
    qm=int(nearest_mask(ref,targets,np.array([4,1])).sum())
    targets=ref.astype(float).copy();targets[:,0]=(targets[:,0]-offset)/tempo;targets[:,1]+=shift
    rm=int(nearest_mask(query,targets,np.array([4,1])).sum())
    return dict(query_peaks=len(query),query_matched=qm,reference_peaks=len(ref),reference_matched=rm,
        verify_q=qm/len(query),verify_r=rm/len(ref) if len(ref) else 0.)


def serialization_envelope(reference,query,tempo,offset,shift,end):
    # Native logs round source position to 6 decimals (seconds), tempo to 8.
    # Bound both forward/inverse frame errors conservatively; this is a
    # recount uncertainty interval, never a new matcher tolerance.
    if not len(query):return {k:[0,0] for k in ('query_peaks','query_matched','reference_peaks','reference_matched')}
    spread=max(abs(query[0,0]-end),abs(query[-1,0]-end))+20/tempo
    epsilon=(.5e-6*FPS+spread*.5e-8)*(2+1/tempo)+1e-9
    values=[]
    for direction in (-1,1):
        tol=4+direction*epsilon
        lo=tempo*query[0,0]+offset-tol;hi=tempo*query[-1,0]+offset+tol
        ref=reference[np.searchsorted(reference[:,0],lo):np.searchsorted(reference[:,0],hi,side='right')]
        target=query.astype(float).copy();target[:,0]=tempo*target[:,0]+offset;target[:,1]-=shift
        qm=int(nearest_mask(ref,target,np.array([tol,1])).sum())
        target=ref.astype(float).copy();target[:,0]=(target[:,0]-offset)/tempo;target[:,1]+=shift
        rm=int(nearest_mask(query,target,np.array([tol,1])).sum())
        values.append(dict(query_peaks=len(query),query_matched=qm,reference_peaks=len(ref),reference_matched=rm))
    return {k:[values[0][k],values[1][k]] for k in values[0]}


def hash_points(h):
    key=h[:,0]
    return np.column_stack((h[:,1],h[:,2],h[:,3],(key>>20)-512,((key>>10)&1023)-512,(key&1023)-8)).astype(float)


def hash_survival(reference,query,play,shift):
    tempo=play['tempo'];offset=(play['source_start']-tempo*play['start'])*FPS
    begin=(play['start']+MARGIN)*FPS;end=(play['end']-MARGIN)*FPS
    eligible=(query[:,1]>=begin)&(query[:,1]+query[:,3]<=end)
    q=query[eligible];qp=hash_points(q);qp[:,0]=tempo*qp[:,0]+offset;qp[:,1]-=shift;qp[:,2]*=tempo
    rp=hash_points(reference);scales=np.array([4,1,8,1,1,1])
    freq=Counter(reference[:,0]);keep=np.array([freq[k]<=8 for k in reference[:,0]])
    inside=(reference[:,1]>=tempo*begin+offset)&(reference[:,1]+reference[:,3]<=tempo*end+offset)
    raw=nearest_mask(rp,qp,scales);capped=nearest_mask(rp[keep],qp,scales)
    recovered=nearest_mask(qp,rp[inside],scales)
    anchor_total=len({tuple(a) for a in q[:,1:3]})
    return dict(query_hashes=len(q),supported_raw=int(raw.sum()),supported_after_cap=int(capped.sum()),
        query_supported_fraction=float(raw.mean()) if len(q) else 0.,
        query_supported_fraction_after_cap=float(capped.mean()) if len(q) else 0.,
        reference_hashes=int(inside.sum()),reference_hashes_recovered=int(recovered.sum()),
        reference_recovery_fraction=float(recovered.mean()) if len(recovered) else 0.,
        query_anchors=anchor_total,supported_anchors=len({tuple(a) for a in q[raw,1:3]}),
        supported_anchors_after_cap=len({tuple(a) for a in q[capped,1:3]}))


def longest(rows,field):
    best=count=0;previous=None
    for r in rows:
        count=count+1 if r[field] and previous==r['begin_frame'] else int(r[field])
        best=max(best,count);previous=r['end_frame']
    return best


def native_events(case):
    record=x.read('target/continuation/runs/'+case+'-continuation.result.json')
    assert x.sha(record['stdout']['path'])==record['stdout']['sha256']
    events=[json.loads(line) for line in Path(record['stdout']['path']).read_text().splitlines()]
    assert events[-1]['event']=='done'
    return events,record


def main():
    refs={};extractions=[]
    for r in x.inputs():
        if not r['id'].startswith('reference-'):continue
        peaks,hashes,meta=load_native(r['id']);refs[r['id'][10:]]=(peaks,hashes);extractions.append(meta)
        print('Validated',r['id'],meta['hashes'],'native hashes',flush=True)
    audit=x.read('target/continuation/audit.json');plays=[];windows=[];parity=[];counts=[]
    for case in x.CASES:
        peaks,hashes,meta=load_native(case);extractions.append(meta)
        events,record=native_events(case)
        assert (len(peaks),len(hashes))==(record['done']['peaks'],record['done']['lookups'])
        for event in events:
            if event['event']=='index':
                rp,rh=refs[event['song']];assert (len(rp),len(rh))==(event['peaks'],event['hashes'])
        native_windows=[e for e in events if e['event']=='window' and e['complete']]
        for play in record['input']['intervals']:
            song=play['song'];reference,rhashes=refs[song];tempo=play['tempo'];shift=4 if case=='combined' else 0
            offset=(play['source_start']-tempo*play['start'])*FPS
            local=[];observations=[]
            for w in native_windows:
                begin=round(w['begin']*FPS);end=round(w['t']*FPS)
                if begin/FPS<play['start']+MARGIN or end/FPS>play['end']-MARGIN:continue
                query=peaks[np.searchsorted(peaks[:,0],begin):np.searchsorted(peaks[:,0],end)]
                assert len(query)==w['query_peaks']
                oracle=verify(reference,query,tempo,offset,shift)
                rows=[r for r in events if r.get('event')=='observation' and r.get('song')==song and r['t']==w['t']]
                for o in rows:
                    # Position is source position at the end of this window.
                    native=verify(reference,query,o['tempo'],o['position']*FPS-o['tempo']*end,o['shift'])
                    mismatch={k:[native[k],o[k]] for k in ('query_peaks','query_matched','reference_peaks','reference_matched') if native[k]!=o[k]}
                    if mismatch:
                        bounds=serialization_envelope(reference,query,o['tempo'],o['position']*FPS-o['tempo']*end,o['shift'],end)
                        covered=all(bounds[k][0]<=o[k]<=bounds[k][1] for k in bounds)
                        parity.append(dict(case=case,play=play['play_id'],origin=o['origin'],t=o['t'],mismatch=mismatch,serialization_envelope=bounds,covered_by_serialization=covered))
                    o=dict(o,oracle_position=tempo*end/FPS+offset/FPS)
                    o['position_error']=abs(o['position']-o['oracle_position'])
                    o['compatible_truth']=o['position_error']<=.5 and abs(o['tempo']-tempo)<=.05 and abs(o['shift']-shift)<=2
                    observations.append(o)
                retrieval=[o for o in rows if o['origin']=='retrieval']
                row=dict(case=case,play=play['play_id'],song=song,begin_frame=begin,end_frame=end,
                    begin=begin/FPS,end=end/FPS,**oracle,oracle_start=oracle['verify_q']>=.4,oracle_hold=oracle['verify_q']>=.3,
                    maximum_retrieval_confidence=max((o['confidence'] for o in retrieval),default=0.),
                    native_start_gate_candidates=sum(o['confidence']>=70 and o['verify_q']>=.4 for o in retrieval),
                    native_hold_predictions=sum(o['origin']=='active' and o['verify_q']>=.3 for o in rows),
                    observations=rows)
                local.append(row);windows.append(row)
            existing=audit['results'][case+'-continuation']['score']
            failed=play['play_id'] in existing['misses']
            retrieval=[o for o in observations if o['origin']=='retrieval']
            maximum=max((o['confidence'] for o in retrieval),default=0.)
            start_streak=longest(local,'oracle_start');hold_streak=longest(local,'oracle_hold')
            high=[o for o in retrieval if o['confidence']>=70]
            passing=[o for o in high if o['verify_q']>=.4]
            aligned=[o for o in passing if o['compatible_truth']]
            label='detected'
            if failed:
                label=('oracle_start_support_insufficient' if start_streak<2 else
                       'retrieval_confidence_below_gate' if maximum<70 else
                       'no_retrieved_start_gate' if not passing else
                       'retrieved_wrong_trajectory' if not aligned else 'continuation_or_start_timing')
            counts.append(dict(case=case,song=song,observations=len(observations)))
            row=dict(case=case,**play,e014_detected=not failed,diagnostic=label,windows=len(local),
                oracle_start_streak=start_streak,oracle_hold_streak=hold_streak,
                oracle_start_checks=sum(w['oracle_start'] for w in local),oracle_hold_checks=sum(w['oracle_hold'] for w in local),
                query_peaks=sum(w['query_peaks'] for w in local),query_matched=sum(w['query_matched'] for w in local),
                reference_peaks=sum(w['reference_peaks'] for w in local),reference_matched=sum(w['reference_matched'] for w in local),
                maximum_retrieval_confidence=maximum,retrieval_high_checks=len(high),retrieval_start_checks=len(passing),
                retrieval_correct_trajectory_start_checks=len(aligned),
                active_failed_checks=sum(o['origin']=='active' and o['verify_q']<.3 for o in observations),
                pending_failed_checks=sum(o['origin']=='pending' and o['verify_q']<.4 for o in observations),
                high_candidate_position_errors=[o['position_error'] for o in high],
                hash_survival=hash_survival(rhashes,hashes,play,shift))
            plays.append(row)
        print('Diagnosed',case,flush=True)
    assert len(plays)==110 and len(extractions)==27
    summary=[]
    for case in x.CASES:
        p=[p for p in plays if p['case']==case]
        totals={k:sum(r[k] for r in p) for k in ('windows','query_peaks','query_matched','reference_peaks','reference_matched')}
        hh={k:sum(r['hash_survival'][k] for r in p) for k in ('query_hashes','supported_raw','supported_after_cap','reference_hashes','reference_hashes_recovered','query_anchors','supported_anchors','supported_anchors_after_cap')}
        summary.append(dict(case=case,detected=sum(r['e014_detected'] for r in p),oracle_two_checks=sum(r['oracle_start_streak']>=2 for r in p),
            misses_with_oracle_support=sum(not r['e014_detected'] and r['oracle_start_streak']>=2 for r in p),
            failure_classes=dict(Counter(r['diagnostic'] for r in p if not r['e014_detected'])),
            **totals,query_precision=totals['query_matched']/totals['query_peaks'],reference_recall=totals['reference_matched']/totals['reference_peaks'],
            hashes=hh,hash_recovery=hh['reference_hashes_recovered']/hh['reference_hashes'],
            median_hash_recovery=statistics.median(r['hash_survival']['reference_recovery_fraction'] for r in p)))
    result=dict(protocol=x.identity('docs/experiments/E015-survival-protocol.md'),evaluator=x.identity(__file__),
        extractions=extractions,plays=plays,summary=summary,windows=windows,
        parity=dict(checked_observations=sum(r['observations'] for r in counts),mismatches=parity,
                    extraction_count_parity='all five programme done events and 22 reference index counts agree',
                    triplets_reproduced=sum(m['hashes'] for m in extractions)),
        limits=['supplied source trajectory, not recognition','known recordings and synthetic speech','serialized native trajectories may cause count boundary differences',
                'interior windows only; diagnostic classes are overlapping mechanisms with a fixed attribution order'])
    assert all(p['covered_by_serialization'] for p in parity), 'native count differences outside serialization uncertainty'
    x.save(ROOT/'audit.json',result)
    print(json.dumps(dict(summary=summary,parity=result['parity']),indent=2))


if __name__=='__main__':main()
