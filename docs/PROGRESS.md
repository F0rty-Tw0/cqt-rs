# Goal progress and evidence

## Current state

The user requested the goal workflow directly in this chat on 2026-09-13.
`AGENTS.md` is the standing work/proof protocol; `GOAL.md` defines the
objective and acceptance criteria. No slash command is needed to use it.

Overall goal: **incomplete**. Instruction setup is ready for use. The
initial evaluator change is implemented in draft PR #5; it has not been
merged. No algorithmic speedup or independent recognition gain is verified.

- Tracking issue: <https://github.com/F0rty-Tw0/cqt-rs/issues/4>
- Working PR: <https://github.com/F0rty-Tw0/cqt-rs/pull/5>
- Working branch: `codex/pr3-accuracy-performance`
- Fixed baseline: `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`
- Validated implementation: `938215eac9398e9e78f7ecda32c73e97ff7b3567`
- The subsequent instruction/checkpoint edit changes documentation only.
  Check its eventual commit separately; the CI evidence below belongs to
  the explicitly named implementation commit, not automatically to HEAD.

## Evidence register

| ID | Status | Claim supported | Proof and limits |
| --- | --- | --- | --- |
| E001 | verified | Seven evaluator/provenance regressions pass; the existing Rust correctness/release gates pass at `938215e` | [CI run 34752565983](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34752565983); [check snapshot](evidence/2026-09-13-pr5-ci.json). Nine checks re-read as completed/success for the exact SHA. Replayed events exercise evaluation behavior, not audio recognition quality. |
| E002 | running | Fix truncated stream metadata and undefined empty-input timing JSON | The first real CI run exposed an additional `seconds:nu` formatting bug. Both Rust fixes are now in the candidate; eight real-binary before/after cases are pending. See [experiment record](experiments/E002-empty-stream.md). |
| E003 | blocked | Fresh representative runtime benchmark baseline | Local Cargo/rustc unavailable; the prior rustup attempt failed because `/proc/self/exe` is absent. CI compiled bench targets, but did not measure throughput. |
| E004 | planned | Independent recognition evaluation | Corpus/splits and executable run not yet prepared; existing radio results remain development validation. |

The CI snapshot records observed GitHub metadata, not copies of compiler
or test logs. Follow the run/job URLs for logs; preserve logs/artifacts
needed for later acceptance decisions before they expire.

## Next executable task: E002

**Hypothesis:** the empty-stdin path completes with zero consumed audio but
formats an infinite or non-finite realtime fraction, violating JSON syntax.

**Acceptance rule, set before the run:** reproduce the JSON defect on the
pinned baseline; choose and document an empty-duration representation; fix
it with the smallest compatible change; prove every output line parses as
strict JSON on empty input and remains valid with positive audio duration.
Preserve event behavior and the existing finite-duration field semantics.
Do not describe this as an accuracy or speed improvement.

**First command on continuation:**

```sh
git status --short
sed -n '482,510p' monitor/src/bin/monitor.rs
```

Then inspect the argument parser and existing CLI tests. Create a small
deterministic mono WAV fixture with the Python standard library and a
strict JSON reproducer. Build the pinned baseline and candidate with the
same supported Rust toolchain on a GitHub CI runner if local execution is
still unavailable. Use `--watch fixture=<fixture.wav> --stream -` with an
empty input pipe, then a nonempty PCM pipe. Capture commands, exit status,
stdout, stderr, executable hashes and the expected failure. A build error,
invalid fixture or network failure must not count as proof of the JSON bug.

Verify the candidate against the same reproducer and applicable monitor
gates. Record the actual baseline/candidate commits, run URLs and output
before changing E002 from suspected to verified. Keep the change in a
focused draft PR or clearly separate commit; avoid folding unrelated DSP
tuning into this regression fix.

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
