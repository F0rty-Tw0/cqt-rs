#!/usr/bin/env python3
"""Validate the first E007 artifact and render its run-specific findings."""

import argparse
import hashlib
import json
import math
import re
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('evidence', type=Path)
    p.add_argument('--report', type=Path, required=True)
    p.add_argument('--validation', type=Path, required=True)
    p.add_argument('--allow-incomplete', action='store_true')
    args = p.parse_args()
    root = args.evidence
    read = lambda name: json.loads((root / name).read_text())
    assert (root / 'harness-commit.txt').read_text().strip() == '64bc53d448697f860586139bb256c9b1764e982b', 'this narrative describes the first E007 run only'
    plan, runs, streams = map(read, ['plan.json', 'results.json', 'streams.json'])
    meta = read('metadata.json') if (root / 'metadata.json').exists() else None
    complete = meta is not None and meta['status'] == 'completed'
    assert complete or args.allow_incomplete, 'incomplete evidence needs explicit review'
    expected_cases = {(mode, treatment, side) for mode, treatments in
                      [('short', ['self', 'negative', 'clean', 'pitch', 'tempo', 'combined']),
                       ('full', ['clean', 'combined'])]
                      for treatment in treatments for side in ('baseline', 'candidate')}
    actual_cases = {(r['mode'], r['treatment'], r['side']) for r in runs}
    assert len(actual_cases) == len(runs) and actual_cases <= expected_cases
    if complete:
        assert meta['runs'] == len(runs) == 16 and actual_cases == expected_cases
    assert all(('short', t, s) in actual_cases for t in
               ['self', 'negative', 'clean', 'pitch', 'tempo', 'combined']
               for s in ['baseline', 'candidate']), 'primary matrix is incomplete'
    for side in ('baseline', 'candidate'):
        assert (root / f'{side}-commit.txt').read_text().strip() == plan[f'{side}_commit']
        assert (root / f'{side}-status.txt').read_text() == ''
    assert read('plan.json') == json.loads(Path('experiments/toucan2020.json').read_text())
    truth = read('self-truth.json')
    hashes, checked = {}, 0
    for r in runs:
        assert r['returncode'] == 0
        for stream in ('stdout', 'stderr'):
            assert digest(root / Path(r[stream]['path']).name) == r[stream]['sha256']
            checked += 1
        events = []
        for line in (root / Path(r['stdout']['path']).name).read_text().splitlines():
            if r['side'] == 'baseline' and line.startswith('{"event":"stream",'):
                value = f"{streams[r['treatment']]['seconds']:.2f}"
                line, count = re.subn(r'("seconds":)' + re.escape(value[:2]) + '(?=,)',
                                      lambda m: m[1] + value, line)
                assert count == 1
            events.append(json.loads(line, parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x))))
        assert events[-1] == r['done'] and events[-1]['event'] == 'done'
        starts = [e for e in events if e['event'] == 'start']
        assert starts == r['starts']
        h = hashlib.sha256()
        for e in events:
            e = dict(e)
            if e['event'] == 'index_done':
                del e['seconds']
            if e['event'] == 'done':
                del e['cpu_seconds'], e['realtime_fraction']
            h.update(json.dumps(e, sort_keys=True).encode() + b'\n')
        assert h.hexdigest() == r['event_digest']
        hashes[(r['mode'], r['treatment'], r['side'])] = h.hexdigest()
        found = 0
        for track in plan['tracks']:
            song = track['id']
            hits = [s for s in starts if s['song'] == song]
            if r['treatment'] == 'self':
                seg = next(s for s in truth if s['song'] == song)
                hits = [s for s in hits if seg['start'] <= s['consumed'] <= seg['end'] + 1]
            stored = r['tracks'][song]
            assert (bool(hits), len(hits), hits[0] if hits else None) == (
                stored['detected'], stored['starts'], stored['first_start'])
            reports = [e for e in events if e['event'] == 'report' and e['song'] == song]
            assert stored['max_confidence'] == max((e['confidence'] for e in reports), default=0)
            assert stored['max_verify_q'] == max((e['verify_q'] for e in reports), default=0)
            found += bool(hits)
        assert found == r['detected']
    pairs = []
    for mode, treatment in sorted({(m,t) for m,t,_ in actual_cases}):
        a, b = (mode, treatment, 'baseline'), (mode, treatment, 'candidate')
        if a in hashes and b in hashes:
            assert hashes[a] == hashes[b]
            pairs.append([mode, treatment])
    lookup = {(r['mode'], r['treatment'], r['side']): r for r in runs}
    selected = lambda mode, treatment: lookup[(mode, treatment, 'candidate')]
    incomplete = []
    for mode, treatment, side in sorted(expected_cases - actual_cases):
        path = root / f'{mode}-{treatment}-{side}.jsonl'
        incomplete.append(dict(mode=mode, treatment=treatment, side=side,
                               raw_sha256=digest(path) if path.exists() else None,
                               complete=False))
    validation = dict(status='complete' if complete else 'partial', completed_runs=len(runs),
                      expected_runs=16, checked_raw_hashes=checked, identical_pairs=pairs,
                      unfinished=incomplete, metadata_present=meta is not None,
                      binary_hashes_available=meta is not None and 'binaries' in meta,
                      harness_commit=(root / 'harness-commit.txt').read_text().strip(),
                      plan_sha256=digest(Path('experiments/toucan2020.json')),
                      artifact_files={f.name: dict(bytes=f.stat().st_size, sha256=digest(f))
                                      for f in sorted(root.iterdir()) if f.is_file()})
    args.validation.parent.mkdir(parents=True, exist_ok=True)
    args.validation.write_text(json.dumps(validation, indent=2) + '\n')
    lines = ['# E007 findings: a real DJ mix with ten-second references', '',
             f"[Toucan Music 2005 to 2020]({plan['source_page']}): {streams['clean']['seconds']/60:.2f} minutes and 22 listed tracks. A seeded draw selected this mix from ten entries in the label's catalogue before recognition results.", '',
             '**The requested short-reference matrix completed. The full experiment did not:** the 35-minute job limit interrupted the extra full-track controls. The table retains every completed case; missing diagnostics are not counted as misses or successes.', '',
             'References are clean ten-second midpoint excerpts from the exact original track versions. Pitch is raised two semitones; tempo is independently reduced 10%. The combined treatment adds 12 seconds of synthetic voice every 30 seconds, matched to local music RMS (0 dB during speech). Slowed streams last 100.68 minutes.', '',
             '| Stream | PR #3 found | PR #7 found | Coverage (each) | Start events (each) |',
             '| --- | ---: | ---: | ---: | ---: |']
    for t in ['clean', 'pitch', 'tempo', 'combined']:
        a, b = lookup[('short', t, 'baseline')], selected('short', t)
        lines.append(f"| {t} | {a['detected']}/22 | {b['detected']}/22 | {b['detected']/22:.1%} | {len(b['starts'])} |")
    lines += ['', 'A listed identity counts once if the monitor emits a start event. Extra starts remain visible; they can represent fragmentation, repetitions or errors. This is track-list coverage, not verified per-play recall or in-mix precision.', '',
              f"Original-snippet control: **{selected('short','self')['detected']}/22** in the known playback windows. Speech/silence negative control: **{len(selected('short','negative')['starts'])} starts over five minutes**, for both versions. The two versions process the same negative audio, so this is five minutes of unique exposure. With zero starts, the one-sided 95% Poisson upper rate bound is {(-math.log(.05)/(5/60)):.1f}/hour under that model; the small control cannot establish a low field false-alarm rate.", '',
              f"All non-timing events agree exactly in **{len(pairs)} completed baseline/candidate pairs**. PR #7 therefore shows no recognition improvement on these cases. Its earlier verifier microbenchmark improvement must not be presented as an end-to-end speedup here.", '',
              '## Every track', '',
              'Y = at least one start for that identity; — = none. Short-reference columns are identical for both versions. The final column is the completed PR #3 full-track diagnostic only.', '',
              '| # | Artist — exact version | Clean | Pitch | Slower | Combined | PR #3 full clean |',
              '| --- | --- | --- | --- | --- | --- | --- |']
    for i, track in enumerate(plan['tracks'], 1):
        rows = [selected('short',t) for t in ['clean','pitch','tempo','combined']]
        rows.append(lookup.get(('full','clean','baseline')))
        cells = ['pending' if r is None else 'Y' if r['tracks'][track['id']]['detected'] else '—' for r in rows]
        lines.append(f"| {i} | {track['artist']} — {track['title']} | " + ' | '.join(cells) + ' |')
    lines += ['', '## What the failures show', '',
              'The combined treatment recovers 5/22 identities: Redmann — Scratching The Surface (Phish Funk Disco Mix); Frau Holle — Chasing Rainbows (Marc Burt and Notch remix); Redmann — Turn The Corner (Notch remix); Beat Doctor — Beast; and Silverknight & Beat Doctor — Dancefloor Virus 2007.', '',
              'Sleep Tight (Sergio’s Last Remix), t01, also fails its exact-original snippet control. In the emitted winning-candidate reports its confidence reaches 94.8, but whenever confidence is at least 70 its maximum query alignment is only 0.217 (the start gate is 0.4). Reports reaching alignment 0.4 have confidence no higher than 23.1. High confidence and high alignment occur at different times. The start gate explains the observed rejection; why the matcher selects those positions remains a debugging question. Reports show the highest-evidence song, not every internal candidate.', '',
              'The completed PR #3 full-reference clean diagnostic finds 20/22 identities (77 starts), versus 14/22 for short references. It misses Roots and Shoots and Pioneers. Longer references help coverage in this case, but the comparison does not isolate omitted excerpts from additional fingerprint evidence or version/alignment problems.', '',
              'Pitch and tempo separately yield 12/22, but they do not miss precisely the same tracks. Slowing recovers t01 while losing other clean detections. The 5/22 combined result cannot isolate voiceover damage: a pitch-plus-tempo-only treatment and a voice-only treatment would be needed. EQ has not been measured; see [E008](E008-eq-plan.md).', '',
              '## Timing and unfinished diagnostics', '',
              'Single-run short-reference wall times are about 39–44 seconds for a 90.61–100.68-minute mix. These are descriptive elapsed times, not process CPU, algorithmic latency, memory measurements or repeated speed benchmarks. Full-reference PR #3 clean processing took 505.863 seconds; indexing/matching costs also change with longer references.', '',
              'Thirteen of sixteen planned runs finished. Full-reference PR #7 clean was interrupted at approximately 4504.5 seconds of consumed audio; both full-reference combined runs never started. The cancelled run is preserved and excluded from completed counts. No full-reference parity or combined full-reference result is claimed.', '',
              '## Proof and limitations', '',
              f"Validated {checked} completed-run stdout/stderr hashes, all per-track counts and first starts, all reported maxima, and {len(pairs)} full non-timing event-parity pairs against raw JSONL. The known PR #3 stream-duration formatting defect is repaired only for parsing; raw output remains unchanged. The original artifact ZIP digest also matches GitHub's recorded digest.", '',
              'The executing harness is commit `64bc53d448697f860586139bb256c9b1764e982b`; baseline is `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`; candidate is `176bce2fef1d1463ff7b815f4f0ee62a21deb435`. CI logs record clean pinned builds, the shared Cargo.lock, Rust 1.98.1 and one Rayon thread. Source/transform hashes, commands, host details and every completed raw output are retained.', '',
              '**Proof gap:** cancellation prevented the original script’s final metadata write, losing actual executable hashes and its runtime identity document. Pinned build logs identify the source, but cannot reconstruct the exact lost binary hashes. The full acceptance gate remains blocked. The harness now writes identities before work and atomically checkpoints JSON; a hard-kill regression fails on the executed harness and passes after the fix. This repair does not retroactively supply the missing hashes.', '',
              'The independently downloaded mix has SHA-256 `39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103`, matching the CI source manifest. Its MD5 `9aa5133f8cc5917e6f09ef1f6bef0391` also matches the Internet Archive source metadata. Other source and generated-audio hashes were recorded by the harness, not all independently redownloaded.', '',
              'The publisher supplies track identities but no independent cue times. Some midpoint excerpts may be omitted from the DJ edit; exact onset delay and correctness of every in-mix start remain unverified. One mix from one catalogue cannot establish generalization, and confidence scores are not probabilities. No thresholds or excerpts were retuned after results.', '',
              'See [evidence and reproduction](../evidence/E007/README.md) for the artifact, validation manifest, raw numerical summaries and failure logs. Audio is CC BY-NC-SA 4.0 with Toucan/artist attribution for this noncommercial experiment; this is not blanket permission for commercial use.']
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text('\n'.join(lines) + '\n')
    print(json.dumps({k:v for k,v in validation.items() if k != 'artifact_files'}, indent=2))


if __name__ == '__main__':
    main()
