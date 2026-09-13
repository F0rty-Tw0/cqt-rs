#!/usr/bin/env python3
"""Replay E009 outputs whose on-disk identity failed; retain originals and compare events."""
import json
import os
from pathlib import Path
import subprocess
import time

import real_mix_eval as common
from recognition_lab import score


def main():
    root = Path('target/lab')
    audit = json.loads((root/'output-cache-mismatches.json').read_text())
    directory = root/'evidence/modal-a/replays'
    directory.mkdir(exist_ok=True)
    records = []
    for item in audit:
        group, treatment, side = (item[k] for k in ['group', 'treatment', 'side'])
        result_path = root/'evidence/modal-a'/group/'results.json'
        rows = json.loads(result_path.read_text())
        index = next(i for i, r in enumerate(rows) if (r['treatment'], r['side']) == (treatment, side))
        old = rows[index]
        stem = directory/f'{group}-{treatment}-{side}'
        out, err = Path(str(stem)+'.jsonl'), Path(str(stem)+'.stderr.txt')
        if out.exists():
            raise FileExistsError(out)
        common.save(Path(str(stem)+'.original-record.json'), old)
        started = time.perf_counter()
        with out.open('w') as o, err.open('w') as e:
            run = subprocess.run(old['command'], stdout=o, stderr=e,
                                 env=dict(os.environ, RAYON_NUM_THREADS='1'), timeout=300)
        elapsed = time.perf_counter()-started
        assert run.returncode == 0
        events = common.parse_events(out, side == 'pr3', old['input']['seconds'])
        digest = common.stable_digest(events)
        assert digest == old['event_digest'], (group, treatment, side, 'replay changed events')
        row = dict(old, elapsed_seconds=elapsed, stdout=common.identity(out),
                   stderr=common.identity(err), done=events[-1], event_digest=digest,
                   **score(events, old['input']['truth'], old['input']['songs']))
        rows[index] = row
        common.save(result_path, rows)
        records.append(dict(group=group, treatment=treatment, side=side,
                            original=old, observed_failure=item, replay=row,
                            identical_non_timing_events=True))
        common.save(directory/'replay-record.json', dict(evaluator=common.identity(__file__), runs=records))
        print('Replayed with identical non-timing events:', group, treatment, side, flush=True)


if __name__ == '__main__':
    main()
