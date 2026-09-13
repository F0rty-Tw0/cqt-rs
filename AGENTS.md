# cqt-rs goal protocol

## Standing objective

Use this repository and the current chat to pursue the research goal in
`docs/GOAL.md`, tracked in GitHub issue #4. Native `/goal` activation is not
required. Apply this protocol to work on that goal; the user's latest
instructions can refine, pause or replace it.

Start from PR #3's pinned baseline
`7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`. Preserve it for comparisons.
PR #5 contains the first follow-on work. Verify live branch/PR state before
updating it. Preserve the user's work and use separate branches/worktrees
for experiments. Do not merge, release or change access controls unless
the user authorizes that action. These instructions do not add permissions.

## Work loop

1. Read `docs/GOAL.md` and `docs/PROGRESS.md`. Inspect the worktree and the
   relevant source, tests and existing evidence before changing anything.
2. Pick the highest-value unblocked task. Prefer a concrete correctness or
   measurement gap before tuning an algorithm. Do not repeat a rejected
   experiment without stating what new evidence changes its hypothesis.
3. Before the experiment, record its hypothesis, baseline, candidate,
   inputs/split, metric, acceptance rule, commands and resource budget.
   Keep one principal experimental variable at a time.
4. Reproduce the problem or baseline. Make the smallest useful change,
   then test correctness and measure the intended effect. Debug failures
   rather than weakening the test, dropping cases or moving the threshold.
5. Validate the proof below. Record the result as verified, rejected or
   blocked. Keep unverified candidates explicitly marked in draft PRs.
6. Update the experiment record and exact next action. Continue with the
   next useful step while the requested work and execution budget permit;
   do not repeatedly ask whether to continue already-authorized work.

If one path is blocked, record the failing command and reason, try a
supported alternative, and continue independent work. For example, local
Rust failure does not prevent correctness checks on GitHub CI. Do not
retry the same installation failure without a relevant environment change.
Ask the user only when missing data, a decision, or access actually blocks
the next necessary step after useful authorized work has been done.

Before yielding, save the state, evidence locations, unfinished work and
exact next command in `docs/PROGRESS.md`. Resume from that checkpoint on
continuation. Do not imply a scheduler or background process exists after
the chat stops executing; the files preserve the objective and state.

## Proof required for claims

| Claim | Required evidence |
| --- | --- |
| Bug fixed | Reproducer fails on the stated baseline for the expected defect, passes on the candidate, and relevant regression gates pass. A build/setup failure is not reproduction of the bug. |
| Numerically accurate | Independent direct/reference oracle, aligned semantics, explicit tolerance and signal floor, boundary/parameter coverage, and worst-case errors. |
| Recognition improved | Frozen recording-disjoint evaluation, paired before/after results, recall and misses, false starts/exposure, detection/end delays, uncertainty and treatment breakdowns. |
| Faster or smaller | Comparable repeated measurements with the same workloads, environment and quality gates; distributions/variance, memory scope and worst regressions. Compile-only benches are not timing results. |
| CI passed | The relevant checks completed successfully for the exact stated commit; retain the run/job URLs. Old green checks do not validate a newer head. |
| Goal complete | Every completion criterion in `docs/GOAL.md` has linked evidence or an explicitly user-approved scope change. An open blocker is not completion. |

For every experiment, retain:

- Baseline/candidate commit SHAs, dirty state/patch identity, actual binary
  SHA-256 and hashes of evaluated data, annotations and evaluator code.
- Commands, exit codes, raw output or durable CI/artifact links, dependency
  resolution, compiler/runtime, machine, feature flags and thread count.
- Metric definition, sample count/exposure, treatment of failures/misses,
  before/after values, uncertainty, regressions and the decision rationale.
- An explicit statement of what the evidence does and does not establish.

Check that evidence identifies the tested implementation and inputs,
artifacts are readable and hashes match before marking a claim verified.
Recalculate headline deltas from the raw results where available. If an
artifact expires or cannot be recovered, mark that proof unavailable and
regenerate it before relying on it for a new acceptance decision.

Use at least five independent alternating before/after timing repetitions
as specified by `docs/GOAL.md`. Shared CI timing may support an exploratory
result with its limitations stated; it is not an authoritative speed gate.
Wall time, CPU time, algorithmic latency and audio duration are different
metrics. Keep them separate. Confidence is a score, not a probability.

Preserve public API semantics, stream/batch parity, the no-default-features
path and the unsafe-code prohibition. Freeze thresholds before final
evaluation. Do not tune on the held-out results and keep calling them
held-out. Test losses must stay visible in the report.

Use meaningful tests for behavioral changes and the repository's required
CI gates. Documentation-only edits need content/link/diff verification;
do not invent runtime measurements or add tests that merely mirror prose.
Avoid unrelated changes and repeated full test runs once the concrete
risks and required gates are resolved.

## Checkpoints and communication

Use `planned`, `running`, `verified`, `rejected` and `blocked` for individual
experiments; label a source-reading finding `suspected` until reproduced.
The overall goal remains incomplete while required experiments are open.
Give concise updates containing the finding, proof, limitation and next
action. A status question refines the active work rather than canceling it.
Record rejected ideas as carefully as accepted ones. Keep GitHub issue #4
and the relevant draft PR descriptions aligned with verified results.
