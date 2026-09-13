# Goal progress and evidence

## Current state

The user requested the goal workflow directly in this chat on 2026-09-13.
`AGENTS.md` is the standing work/proof protocol; `GOAL.md` defines the
objective and acceptance criteria. No slash command is needed to use it.

Overall goal: **incomplete**. Evaluation safeguards are implemented in
PR #5. Two Rust CLI defects are fixed and verified in PR #6. PR #7 implements a verifier optimization: 31–34% less time in large-track
synthetic cases, with exact exhaustive-oracle parity. This is exploratory
stage performance; independent accuracy and end-to-end gains are unproved.

PR #9 now implements an experimental `--modal-fit` and completes E008/E009.
The 90.61-minute mix improves from 14→15/22 clean, 12→12 pitch, 12→13 slowed,
and 5→8 combined. There are no lost baseline identities. Twelve isolated EQ
filters each recover all 22 development and eight held-out snippets; the
held-out baseline already recovers all eight. Held-out voice and combined
outcomes remain 6/8 and 5/8. The candidate stays opt-in because unseen-recording
gains, robust position estimates and production false-alarm rates are unproved.
See [E009 results](experiments/E009-results.md) and [proof](evidence/E009/README.md).

- Tracking issue: <https://github.com/F0rty-Tw0/cqt-rs/issues/4>
- CLI PR: <https://github.com/F0rty-Tw0/cqt-rs/pull/6>
- Verifier experiment: <https://github.com/F0rty-Tw0/cqt-rs/pull/7>
- Fixed baseline: `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`
- Validated CLI implementation: `73d19401fb68b3b167814ec18466ae76cd7dfd41`
- Validated verifier: `7e529dc8d73454e56b8c283d0a53ab7c56ca68eb`
- Tested modal candidate: `69c09c4be5fb391a9e3d93ff4465cc4864d0ddf4`
- CLI proof cache setup fix: `2ccb2f6b4259ffc8181021f5841b3c0f896d4481`
- Accuracy evidence belongs to the explicitly identified binary/source,
  not automatically HEAD. No default behavior change, merge or release.

## Evidence register

| ID | Status | Claim supported | Proof and limits |
| --- | --- | --- | --- |
| E001 | verified | Seven evaluator/provenance regressions pass; the existing Rust correctness/release gates pass at `938215e` | [CI run 34752565983](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34752565983); [check snapshot](evidence/2026-09-13-pr5-ci.json). Nine checks re-read as completed/success for the exact SHA. Replayed events exercise evaluation behavior, not audio recognition quality. |
| E002 | verified | Correct full stream durations and null empty-input timing ratio | Eight real-binary before/after cases and ten CI checks pass at `73d1940`. Downloaded artifact and input/log hashes verified. [Experiment and preserved evidence](experiments/E002-empty-stream.md). |
| E003 | blocked | Fresh representative runtime benchmark baseline | Local Cargo/rustc unavailable; the prior rustup attempt failed because `/proc/self/exe` is absent. CI compiled bench targets, but did not measure throughput. |
| E004 | planned | Independent recognition evaluation | Corpus/splits and executable run not yet prepared; existing radio results remain development validation. |
| E005 | verified | Verifier searches a bounded reference interval with exact count parity; large synthetic cases use 31–34% less time | Five alternating process pairs, 3,030 oracle combinations, ten checks at `7e529dc8`; all raw results/hash review in [experiment](experiments/E005-verifier-window.md). Whole-reference cases regress about 0.1%; real-monitor benefit remains unproved. |

E007 primary counts retain incomplete diagnostics and missing binary identities.
E008/E009 now have 162 fully audited cases, including eight replayed outputs
whose non-timing events match the original recorded digests. Nine auxiliary
audio caches and two fingerprint dumps required exact-hash restoration.
All actual detector inputs retained their hashes. The incident and original
damaged logs are preserved, alongside 324 checked output hashes, 44 baseline
parity pairs, default parity and 390 native fingerprint comparisons.

The CI snapshot records observed GitHub metadata, not copies of compiler
or test logs. Follow the run/job URLs for logs; preserve logs/artifacts
needed for later acceptance decisions before they expire.

## User-requested task: E007, real-mix recognition

[Draft PR #8](https://github.com/F0rty-Tw0/cqt-rs/pull/8) implements the
real-audio harness, frozen sources, transforms and report. The requested
short-reference matrix completed. [Full findings](experiments/E007-results.md)
and [proof/failure record](evidence/E007/README.md) retain every miss.

| Stream / ten-second references | PR #3 | PR #7 |
| --- | ---: | ---: |
| Clean | 14/22 | 14/22 |
| Pitch +2 semitones | 12/22 | 12/22 |
| Tempo 0.9 | 12/22 | 12/22 |
| Both plus recurring voiceover | 5/22 | 5/22 |

All six completed pairs have identical non-timing events. Self-control is
21/22; five minutes of speech/silence produces zero starts. This is track-list
coverage on one 90.61-minute mix, not independently timed per-play recall.
No recognition gain or real-monitor speedup is established.

E007 status: **blocked for full acceptance**. Thirteen of sixteen runs
finished at harness `64bc53d448697f860586139bb256c9b1764e982b`. Full-reference
PR #3 clean finds 20/22; PR #7 clean was interrupted by the 35-minute job
limit, and both combined full-reference cases never started. The artifact
ZIP hash and 26 completed raw-output hashes were checked; all counts and
six event-parity pairs were recomputed. Binary identities were lost when
cancellation skipped the old final metadata write. Pinned source/toolchain
logs survive but are not substitutes for missing executable hashes.

The metadata-loss defect is reproduced with SIGKILL: the original harness
fails and the repaired harness passes. Identities now persist before work
and JSON checkpoints are atomic. The workflow is manual, with a 60-minute
evaluation step inside a 65-minute job; the heavier experiment has not
been rerun. Normal correctness CI passed all nine checks at `64bc53d`.
Do not transfer that green status automatically to this later repair.

The later [E008/E009 EQ sweep](experiments/E009-results.md) supplies the
measurements missing from E007. Confidence remains an evidence score,
not a calibrated probability.

Exact proof recheck (download/extract the original linked artifact first):

```sh
python3 scripts/report_real_mix.py target/real-mix-first --allow-incomplete --report docs/experiments/E007-results.md --validation docs/evidence/E007/validation.json
python3 -m unittest discover -s scripts -p 'test_real_mix_checkpoint.py' -v
```

E009 repeats the requested short-reference mix matrix with recorded binary
identities; it does not complete E007's full-reference diagnostics. The t01
miss was reproduced and its competing alignment offsets investigated.
Modal fitting recovers the identity but can choose a repeated passage about
8.7 seconds away from the true position.

Next bounded experiment: freeze multiple reference excerpts per song and
new artist-disjoint recordings before evaluation. Compare one midpoint,
several distributed excerpts and full-track references at unchanged gates;
score song identities and duplicate starts after explicit excerpt-to-song
mapping, with independent mix cue times and held-out music negatives. Then
separately test speech-resistant peak selection/verification. Do not choose
the new setup on the existing eight held-out recordings; those outcomes are
now known. Full CQT numerical/performance goals remain open.

## Remaining gates and unblock actions

| Gate | State | Next action |
| --- | --- | --- |
| Evaluator identity and comparison guards | Verified at E001 commit | Preserve the existing regressions in subsequent experiments |
| Broader CQT numerical/streaming matrix | Planned | Add a bounded parameter grid against the dense oracle and identify uncovered boundaries |
| Process CPU, hop latency and peak RSS measurements | Planned | Define timing scopes and instrumentation independently of wall time |
| Repeated baseline runtime measurements | Blocked locally | Use an available representative Rust host; exploratory CI timing must be labeled accordingly |
| Recording-disjoint corpus | Small frozen test completed | Eight held-out and eight negative recordings; expand artists/genres and preserve a fresh split before the next candidate |
| Recognition/performance experiments | E009 recognition completed; broader goals open | Keep modal fit opt-in; test reference coverage and speech robustness with new data; no end-to-end speedup claim |

Do not stop other useful work just because the benchmark host is missing.
Do not claim that CI smoke measurements meet the representative-host gate.

## Record for each new experiment

Copy this compact record into this file or a linked experiment document:

```text
ID / status / date:
Hypothesis and acceptance rule (recorded before running):
Baseline / candidate commit; dirty state and patch identity:
Binary / evaluator / corpus / annotation hashes:
Data split, parameters, features, threads, seed:
Host / OS / toolchain / dependency resolution:
Commands, exit codes, raw output/artifact links:
Expected baseline failure or measured baseline:
Candidate results; uncertainty; misses and worst regressions:
Proof review: source/input identity, hashes, readable artifacts, scope:
Decision: accept / reject / blocked, with reason:
What is still unproved:
Exact next action and command:
```

If a record lacks the required proof, leave its claim unverified even when
the implementation looks correct. Documentation of an experiment is not
evidence that it ran.
