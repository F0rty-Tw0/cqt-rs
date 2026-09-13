# Goal progress and evidence

## Current state

The user requested the goal workflow directly in this chat on 2026-09-13.
`AGENTS.md` is the standing work/proof protocol; `GOAL.md` defines the
objective and acceptance criteria. No slash command is needed to use it.

Overall goal: **incomplete**. Evaluation safeguards are implemented in
PR #5. Two Rust CLI defects are fixed and verified in PR #6. PR #7 implements a verifier optimization: 31–34% less time in large-track
synthetic cases, with exact exhaustive-oracle parity. This is exploratory
stage performance; independent accuracy and end-to-end gains are unproved.

- Tracking issue: <https://github.com/F0rty-Tw0/cqt-rs/issues/4>
- CLI PR: <https://github.com/F0rty-Tw0/cqt-rs/pull/6>
- Verifier experiment: <https://github.com/F0rty-Tw0/cqt-rs/pull/7>
- Fixed baseline: `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`
- Validated CLI implementation: `73d19401fb68b3b167814ec18466ae76cd7dfd41`
- Validated verifier: `7e529dc8d73454e56b8c283d0a53ab7c56ca68eb`
- This checkpoint changes documentation/evidence only; evidence belongs
  to the explicitly identified implementation commit, not automatically HEAD.

## Evidence register

| ID | Status | Claim supported | Proof and limits |
| --- | --- | --- | --- |
| E001 | verified | Seven evaluator/provenance regressions pass; the existing Rust correctness/release gates pass at `938215e` | [CI run 34752565983](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34752565983); [check snapshot](evidence/2026-09-13-pr5-ci.json). Nine checks re-read as completed/success for the exact SHA. Replayed events exercise evaluation behavior, not audio recognition quality. |
| E002 | verified | Correct full stream durations and null empty-input timing ratio | Eight real-binary before/after cases and ten CI checks pass at `73d1940`. Downloaded artifact and input/log hashes verified. [Experiment and preserved evidence](experiments/E002-empty-stream.md). |
| E003 | blocked | Fresh representative runtime benchmark baseline | Local Cargo/rustc unavailable; the prior rustup attempt failed because `/proc/self/exe` is absent. CI compiled bench targets, but did not measure throughput. |
| E004 | planned | Independent recognition evaluation | Corpus/splits and executable run not yet prepared; existing radio results remain development validation. |
| E005 | verified | Verifier searches a bounded reference interval with exact count parity; large synthetic cases use 31–34% less time | Five alternating process pairs, 3,030 oracle combinations, ten checks at `7e529dc8`; all raw results/hash review in [experiment](experiments/E005-verifier-window.md). Whole-reference cases regress about 0.1%; real-monitor benefit remains unproved. |

The CI snapshot records observed GitHub metadata, not copies of compiler
or test logs. Follow the run/job URLs for logs; preserve logs/artifacts
needed for later acceptance decisions before they expire.

## Active user-requested task: E007, real-mix recognition

The user requested an actual royalty-free stream comparison with ten-second
references, raised pitch, reduced BPM and voiceover. This supersedes the
next profiling step. The frozen protocol is in
[experiments/E007-real-mix.md](experiments/E007-real-mix.md).

Random seed 20260913 selected Toucan Music 2005 to 2020: 22 listed tracks,
about 90 minutes. References are clean ten-second midpoint clips; the full
mix is tested clean, pitch-only +2 semitones, tempo-only 0.9, and combined
with 12-second voiceovers every 30 seconds at equal local RMS. Full-track
references, original-clip self-recognition and speech/silence are controls.
No detector settings are tuned. Source licenses and all hashes are recorded.

Local audio-transform validation passed: independent tempo/pitch on a known
tone, equal-RMS speech and partial-block sample counts. Real-binary results
are pending. CI builds pinned PR #3 and current PR #7 with one toolchain
and lockfile, then runs the same audio through both. Inspect every miss,
raw events and artifacts before claiming recognition coverage. Cue times
are unavailable, so exact onset latency and in-mix false-start precision
are not independently established.

Next: inspect the real-mix workflow and retrieve its evidence. Do not change
reference excerpts or thresholds after opening the results.

## Remaining gates and unblock actions

| Gate | State | Next action |
| --- | --- | --- |
| Evaluator identity and comparison guards | Verified at E001 commit | Preserve the existing regressions in subsequent experiments |
| Broader CQT numerical/streaming matrix | Planned | Add a bounded parameter grid against the dense oracle and identify uncovered boundaries |
| Process CPU, hop latency and peak RSS measurements | Planned | Define timing scopes and instrumentation independently of wall time |
| Repeated baseline runtime measurements | Blocked locally | Use an available representative Rust host; exploratory CI timing must be labeled accordingly |
| Recording-disjoint corpus | Planned | Assemble source/recording/license/hash manifest and freeze splits before tuning |
| Recognition/performance experiments | Planned | Follow the order and acceptance rules in GOAL.md after their prerequisites |

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
