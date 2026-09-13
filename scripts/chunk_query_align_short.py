#!/usr/bin/env python3
"""Reproduce the bounded E010 short-template annotation diagnostics."""
import json
import numpy as np
import chunk_query_align as a
import real_mix_eval as e


def main():
    root = a.ROOT
    destination = root/'alignment-centered-short'
    destination.mkdir(exist_ok=True)
    mix = np.load(root/'alignment-centered/mix.npy')
    windows = {'t01': [0, 180], 't05': [700, 1000]}
    e.save(destination/'plan.json', dict(windows=windows, template_seconds=20, stride_seconds=10,
           reason='No coherent full-minute match; known adjacent songs constrain possible publisher-order intervals. Freeze before native queries.',
           parameters=a.PARAMETERS, evaluator=e.identity(a.__file__), driver=e.identity(__file__)))
    for song, (lo, hi) in windows.items():
        source = np.load(root/'alignment-centered'/(song+'.npy'))
        rows = a.search(mix[:, lo:hi], source, seconds=20, stride=10)
        for row in rows:
            row['mix_start'] += lo
            row['mix_end'] += lo
        path = destination/(song+'.json')
        if path.exists():
            assert json.loads(path.read_text()) == rows, ('annotation replay differs', song)
        else:
            e.save(path, rows)
        print(song, 'short annotations reproduced', flush=True)


if __name__ == '__main__':
    main()
