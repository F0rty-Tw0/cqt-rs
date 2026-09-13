# E005: restrict verifier searches to the query's reference interval

Status: running, 2026-09-13. This is an exploratory stage benchmark.

Baseline: PR #3, `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`.
Candidate parent: `73d19401fb68b3b167814ec18466ae76cd7dfd41`.

## Hypothesis and acceptance rule, before execution

`PeakTrack::verify` binary-searches the entire reference for every query
peak although all possible matches fall in the interval already computed
for reverse verification. Compute that interval once and search its slice.
The positive finite tempo and sorted-query contract make transformed frame
positions monotone; inclusive tolerance boundaries must remain unchanged.
Keep all public fields, matching thresholds and reverse verification exact.

Require exact equality of all four counts against an independent exhaustive
oracle across seeded tracks, shifts, tempo/offset fractions, tolerances,
duplicate frames, empty/out-of-range inputs and inclusive boundaries.
Run the same added integration test against the baseline and candidate,
plus the normal CI gates. A failed test rejects the implementation.

Time only warmed `verify` calls, excluding fixture construction, with
`black_box` preventing dead-code elimination. Use deterministic peak tracks
of 128, 4096 and 65536 peaks, matched and wrong-shift queries, and a query
spanning the full reference. Run five separate alternating baseline and
candidate processes on one GitHub runner, identical toolchain, lockfile,
features, query data and harness. Record per-case raw ns/call, all four
counts, binary/source/harness identities, host and commands.

Exploratory retention rule: oracle/CI pass, at least 10% lower median time
in the 65536-peak cases, and no case with a median regression greater than
5%. Report every case and paired range, including a rejection if it fails.
These thresholds cannot be changed after seeing the timings. Budget:
one CI run plus debugging an identified failure, 15 minutes per workflow.

This synthetic workload isolates verifier cost; it does not establish the
verifier's share of real monitor runtime, recognition generalization, CQT
speed, or the goal's representative-host throughput target. No new
allocation is introduced into verification.

Reproduction: `.github/workflows/verifier-proof.yml`, also usable locally
with the two release `verify_probe` executables and
`python3 scripts/benchmark_verify.py --baseline OLD --candidate NEW`.

## Result

Pending real compilation, oracle execution and measurements. No performance
gain is currently claimed.
