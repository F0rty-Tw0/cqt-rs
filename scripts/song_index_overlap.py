#!/usr/bin/env python3
"""Post-outcome independent-audio diagnostic of E011's first random mix cue."""
import json
from pathlib import Path
import numpy as np
import chunk_query_align as a
from chunk_query_freeze import supported_group
import real_mix_eval as e


def main():
    root=Path('target/song-index')
    manifest=json.loads((root/'manifest.json').read_text())
    query=next(q for q in manifest['queries'] if q['id']=='mix-t01')
    output=root/'overlap-diagnostic'
    output.mkdir(exist_ok=False)
    plan=dict(status='running',query=query,tracks=['t01','t02'],mix_window=[0,120],template_seconds=20,
              stride_seconds=10,parameters=a.PARAMETERS,driver=e.identity(__file__),aligner=e.identity(a.__file__),
              scope='Post-outcome diagnostic; frozen expected identity and headline scoring remain unchanged.')
    e.save(output/'metadata.json',plan)
    mix=np.load(Path('target/chunk-query/alignment-centered/mix.npy'))[:,:120]
    groups={}
    for song in ['t01','t02']:
        path=Path('target/chunk-query/alignment-centered')/(song+'.npy')
        anchors=a.search(mix,np.load(path),seconds=20,stride=10)
        e.save(output/(song+'.json'),anchors)
        try:
            group=supported_group(anchors)
        except ValueError as ex:
            groups[song]=dict(status='unsupported',error=str(ex))
            continue
        groups[song]=dict(status='algorithmically-supported',anchors=group,
            spans_query=any(x['mix_start']<=query['start_seconds'] and x['mix_end']>=query['start_seconds']+10 for x in group))
    e.save(output/'groups.json',groups)
    plan.update(status='completed',groups=e.identity(output/'groups.json'))
    e.save(output/'metadata.json',plan)
    print(json.dumps(groups,indent=2))


if __name__=='__main__':
    main()
