# E005: restrict verifier searches to the query's reference interval

Status: verified exploratory improvement, 2026-09-13. Retain in draft PR #7.

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

Candidate: `7e529dc8d73454e56b8c283d0a53ab7c56ca68eb`. All ten checks
passed, including full Rust release gates, monitor end-to-end detection,
evaluation tests and the [uncached proof run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34754375559).
[Exact check snapshot](../evidence/E005/checks.json).

The independent oracle passed all 2,400 seeded parameter combinations and
630 empty/extreme combinations, plus an explicit inclusive-boundary check.
It ran against the baseline and candidate, and the candidate's
no-default-features build. Every timed case preserved all four counts.

Five alternating process pairs on the same runner gave:

| Case | Baseline median ns | Candidate median ns | Time reduction | Paired reduction range |
| --- | ---: | ---: | ---: | ---: |

| large_match | 28555.3 | 18758.8 | 34.31% | 34.23% to 34.35% |
| large_wrong_shift | 29653.3 | 20565.5 | 30.65% | 30.52% to 30.76% |
| medium_match | 24097.9 | 18667.2 | 22.54% | 21.95% to 22.66% |
| medium_wrong_shift | 25271.9 | 20498.0 | 18.89% | 18.86% to 19.01% |
| small_match | 3583.3 | 3284.7 | 8.34% | 5.75% to 9.24% |
| small_wrong_shift | 3880.0 | 3425.0 | 11.73% | 11.58% to 12.53% |
| whole_match | 476764.4 | 477428.3 | -0.14% | -0.25% to 0.24% |
| whole_wrong_shift | 483648.5 | 484183.5 | -0.11% | -0.19% to 0.28% |

Positive reduction means less elapsed time per verify call. The two
full-reference cases regress by 0.14% and 0.11%; they remain visible and
within the predeclared 5% limit. Both large-track cases exceed the 10%
minimum. Decision: retain the optimization as an exploratory improvement.
The paired ranges are observed ranges of five process pairs, not confidence
intervals. No statistical population or end-to-end speed claim is made.

[Raw timing report](../evidence/E005/timing.json) preserves all repetitions,
iterations, elapsed times, counts, commands and binary hashes. Raw stdout,
stderr, test logs, host, compiler, source identities and shared Cargo.lock
(as `Cargo.lock.txt`) are stored alongside it. The downloaded
[artifact](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34754375559/artifacts/10315944230)
had verified ZIP SHA-256
`9cd99ecb70279d69db1e7e68f9871e6c18ef7d302e91cde556e3762e72b4e4f8`.
All process output hashes, source SHAs, harness/lockfile hashes and raw
rows matched; headline reductions and the acceptance rule were recalculated
from those rows. Candidate checkout was clean; baseline was overlaid only
with the identical added test and example files, listed in its status log.
Both use default features and `RAYON_NUM_THREADS=1`; verification is serial.
Compilation and fixture construction are outside the timed region.

The initial commit `335119b2` passed the oracle and measured 33–36% lower
large-case time, but its format gate failed and cache cleanup reported
errors ([run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34754167654)).
Formatting was repaired without changing the algorithm. The next run at
`38d6e5bb` failed setup because a cached baseline worktree already existed
([run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34754251859)); that
failure is not correctness or timing evidence. Removing caching from this
proof job fixed the source of contamination. Acceptance uses the final
successful uncached run, not the earlier incomplete gates.

This is an allocation-free search-scope optimization, not a change to CQT
numerics or matching thresholds. Remaining work: profile real monitor
stage shares and repeat on a representative host/corpus before claiming
whole-monitor benefit. Independent recognition quality remains unmeasured.
