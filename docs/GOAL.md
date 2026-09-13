# Goal: improve cqt-rs accuracy and performance from PR #3

## Objective and baseline

Make `cqt-rs` and `cqt-monitor` measurably more accurate, faster, and easier
to validate through repeatable research, regression tests, debugging and
benchmarks. Treat numerical transform accuracy and song-recognition
accuracy as separate outcomes.

Run this goal directly in the current chat using the standing instructions
in [AGENTS.md](../AGENTS.md). Maintain the evidence register and exact next
action in [PROGRESS.md](PROGRESS.md). This instruction-driven workflow
does not require native Goal mode. A saved checkpoint preserves continuity;
it does not create an unattended scheduler.

Baseline: [PR #3](https://github.com/F0rty-Tw0/cqt-rs/pull/3), commit
`7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`, branch
`claude/cqt-library-analysis-update-87jyuu`. At inspection on 2026-09-13 it
was open, not merged, with eight passing
[CI checks](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34726543009).
No later PR was present. Keep this commit as a fixed comparison even if
PR #3 advances. Stack follow-on changes on its branch until it merges;
then retarget subsequent work to main after checking ancestry.

The PR reports approximately 10.1 us per fingerprint CQT frame, 1.8 s
median programme detection, and 8–9 s delayed end events. The reference
comparison and recognition results are development measurements from a
small shared recording pool, not independently reproduced results here.
The monitor's reported 0.4% is wall time/audio duration, not measured CPU
utilization. Confidence 70 is a decision score, not a 70% probability.

## Completion criteria for the first research cycle

1. Reproduce the pinned baseline's test gates and benchmark distributions,
   with executable/input hashes, dependency lockfile, compiler, CPU model,
   OS, feature flags, thread count and exact commands recorded.
2. Build a recording-disjoint evaluation corpus. Freeze configuration and
   metrics before opening the final test split. Report exposure, misses,
   false alarms, uncertainty, and every failed case; new seeds or excerpts
   of the development recordings do not make an independent test.
3. Run at least one numerical-accuracy experiment, one recognition
   experiment and one profiled performance experiment. Retain changes only
   when their measured tradeoff meets the gates below. Record rejected
   experiments and negative results.
4. Produce a small, reviewable PR for each accepted change with a regression
   case, before/after results and reproduction instructions. Finish the
   cycle with measured results and explicitly unresolved blockers. A
   missing corpus or toolchain is incomplete work, not a passing gate.

The research cycle is complete when these deliverables exist, even if
some ambitious targets prove unattainable. Never substitute a claimed
speedup for a measured result. Merging and releasing are separate actions.

## Scorecard and initial targets

These are engineering targets to test, not promises or current results.
Choose a fixed Linux x86-64 host for the first measurements; add ARM64 when
a representative target is available. Compare the same host to itself.

| Dimension | What to measure | Gate / target |
| --- | --- | --- |
| CQT correctness | Complex error against dense f64 direct evaluation; per-bin absolute and relative magnitude error; peak location, gain and boundary behavior | Preserve current `2e-4` direct-oracle tolerance and existing multirate `<1% of peak` fixtures; extend coverage before generalizing these bounds |
| CQT tradeoff | Error, latency and runtime versus sparsity, gamma, filter scale and kernel cap | Publish a Pareto table; any relaxed tolerance is an explicit opt-in configuration |
| Recognition | Per-play recall, false starts/hour, duplicate starts, misses by treatment | No additional false starts or misses on the fixed regression corpus; compare paired outcomes and uncertainty on independent data |
| Start delay | p50/p95 from first audible sample and, separately, after talk-over ends | Seek at least 25% lower p95 in the difficult treatment group at matched recall and false-alarm policy; retain misses in the scorecard |
| End delay | p50/p95 plus premature endings and quiet-passage dropouts | Explore p95 <=3 s while preserving continuity; baseline reports roughly 8–9 s |
| Recognition estimates | Shift error in cents/bins, relative tempo error, position error in seconds | Report p50/p95 and worst cases, including changing tempo and fractional-bin shifts |
| Throughput | Build, fresh/reused batch, warmed stream, index lookup and whole monitor separately | Seek >=20% improvement in a profiled dominant cost; stretch target 2x end-to-end, with no material accuracy regression |
| Streaming resources | Hop service p50/p95/p99, allocations after warmup, peak RSS, index bytes per second of watched audio | No new steady-state allocations in the CQT path; no unbounded growth; seek 30% less index memory at matched quality |
| Scale | Watch lists of 2, 8, 32, 128 songs, then larger if needed | Record memory, collisions, candidate counts, false alarms and query runtime at every size |

Use both relative error above a fixed signal floor and absolute error near
silence; a global peak-normalized error can hide a bad low-energy bin.
Keep API semantics, stream/batch parity, feature modes, and `unsafe_code =
"forbid"`. Avoid changing defaults on the basis of a single corpus result.

## Ordered experiments

1. **Measurement and evaluation integrity.** Fingerprint the actual tested
   binary and all inputs, correct wall/CPU labels, prevent cross-corpus
   comparisons, separate index construction from streaming, and add
   replayable evaluator tests. Still needed: direct process CPU accounting,
   per-hop tail latency, peak RSS, and valid CLI JSON on empty live input
   (the baseline computes `elapsed / audio` when audio can be zero).
2. **Independent recognition evaluation.** Start with new artists and
   recordings plus annotated real DJ mixes. Keep development, calibration
   and final test recordings disjoint. Group related releases/remixes by
   source recording to avoid leakage. Include nonmatching songs, same
   artist, speech, silence, repetitive music, loops and crossfade boundaries.
   Define whether a sampled loop should count as a song before scoring it.
3. **CQT stress grid.** Sweep 8/16/22.05/44.1/48 kHz; 12/24/36 bins per
   octave; gamma 0/10/25; caps and sparsity extremes; random chunk sizes,
   odd hops, impulses at edges, octave boundaries, Nyquist-adjacent tones,
   off-bin tones, chirps, noise and silence. Align window, normalization,
   center/padding and resampling semantics for an external reference.
   Debug failures against the direct oracle before changing the algorithm.
4. **Speech and vocoder robustness.** Measure peak survival before hashes.
   Compare local whitening, frequency-dependent prominence, sub-bin peak
   refinement and query-side tolerances separately. Test pair-assisted
   candidate generation followed by the existing verifier, with a bounded
   candidate budget. Do not blindly combine two windows by taking the
   maximum confidence: the false-alarm rule must be recalibrated.
5. **Earlier decisions and endings.** Compare sequential evidence and
   verifier-driven releases with the current tracker. Explicitly test
   quiet passages, alternating hypotheses, repeated plays, abrupt cuts,
   tempo drift and rebase boundaries. Preserve exact expiry behavior.
6. **Profile then optimize.** Measure decimation, FFT, sparse product,
   peak picking, lookup, voting and verification costs. Try incremental
   matcher slab totals with oracle parity on push/expiry/rebase, compact
   index layouts, and contiguous filter loops. Retest realistic hit/miss
   distributions and larger watch lists. PR #3 already rejected a grouped
   ratio directory and distinct-anchor evidence; revisit only with a new
   hypothesis and a workload that explains why the result could change.
7. **Larger architectural experiment.** Prototype a constant-Q
   nonstationary Gabor transform behind a separate benchmark interface.
   Judge peak stability and end-to-end recognition per unit compute,
   memory and latency. A different transform need not be numerically
   identical to the current CQT and should not silently replace its API.

## Corpus and statistical protocol

Record source URL, recording ID, artist, license/attribution, checksum,
sample rate, channel mix, excerpt boundaries, transformation parameters,
random seed, split and ground-truth intervals in a versioned manifest.
Use legitimately available recordings; keep large audio out of Git.
The held-out watch references may be indexed for recognition, but the
recordings and their query variants must not have informed tuning.

Test pitch and tempo independently and jointly: fractional semitone
changes, +/-2 semitones, +/-12% key-locked tempo, resampling, codec passes,
EQ, compression, clipping, noise, room response and speech/music ratios.
Include continuous mixes with changing tempo and crossfades, not only
isolated transformed excerpts. Attribute every start once, including
transition alarms; do not omit missed plays from headline latency results.

Use play/recording-level paired bootstrap intervals for recall/latency
comparisons, not correlated frame-level samples. For zero false alarms
over T hours, the one-sided 95% Poisson upper bound is `-ln(0.05)/T`.
Thirty alarm-free hours support roughly 0.1/hour under that model; a
0.01/hour bound requires about 300 independent alarm-free hours. Repeating
the same audio adds playback time but does not demonstrate generalization.
With observed alarms, compute the count-dependent interval and show the
raw count/exposure. Do not infer a rare-event probability from confidence.

## Reproduction and comparison

Run the CI gates from `.github/workflows/ci.yml`: format, clippy and docs;
release tests on stable and Rust 1.98, both library feature sets; monitor
tests; bench compilation; and the library package dry run. Run Python
regressions with `python -m unittest discover -s scripts -p 'test_*.py' -v`
after installing numpy, scipy and matplotlib.

For benchmarks, use isolated worktrees and target directories. Record the
resolved Cargo.lock even though the library ignores it in Git; use a shared
compatible dependency resolution for before/after comparisons and document
any required differences. Exclude compilation from runtime timings.
Use Criterion baselines for `--bench bench` and `-p cqt-monitor --bench
index`. Record default versus `--no-default-features` and one versus fixed
multiple Rayon threads. Alternate before/after runs with at least five
independent repetitions on a quiet host. Report distributions and variance;
shared GitHub runners are correctness gates, not authoritative speed gates.

Build both monitor binaries first. Run the same evaluator version against
each explicit absolute `--monitor` path with identical watch lists,
programme and negative streams. Set distinct `--label` and `--out` values;
use `--no-plots` during timing repetitions. Then compare the summaries with
`scripts/radio_compare.py`. Do not include different negative-stream sets
in the two runs. Preserve raw events, summary JSON, manifests, benchmark
output and the exact source revision in the experiment record.

Per experiment, record: hypothesis; baseline/candidate SHA and binary hash;
data/config identity; machine/toolchain; commands; correctness result;
accuracy and performance deltas with uncertainty; worst regressions;
accept/reject decision; and the next question.

## Research starting points

- [Schorkhuber and Klapuri: Constant-Q Transform Toolbox for Music Processing](https://zenodo.org/record/849741)
  motivates the existing multirate baseline and parameter/normalization checks.
- [Six: Panako 2.0 (ISMIR 2021)](https://archives.ismir.net/ismir2021/latebreaking/000039.pdf)
  describes a nonstationary Gabor front end, near-exact hashing and an
  ordered persistent index. Its improvements motivate experiments here;
  they are not expected speedups for this implementation.
- [Sonnleitner, Arzt and Widmer: Landmark-Based Audio Fingerprinting for DJ Mix Monitoring (ISMIR 2016)](https://www.jku.at/fileadmin/gruppen/173/Research/187_Paper.pdf)
  studies real DJ mixes and supplies a dataset with reference tracks and
  border annotations. It motivates independent real-mix evaluation and
  testing robustness against retrieval cost. Verify dataset availability
  and recording licenses when assembling the corpus.
- [Panako implementation](https://github.com/JorenSix/Panako) is a candidate
  external recognition baseline. Pin a revision and equalize the corpus,
  query lengths and tuning budget before making comparative claims.

## Initial checkpoint: 2026-09-13

- Baseline source and existing GitHub CI inspected; no AGENTS.md found.
- Added binary/input/evaluator content identities, guarded comparisons,
  corrected evaluator timing labels and documented historical timing limits.
- Added replay and provenance regressions. These test evaluation behavior,
  not actual recognition accuracy or DSP throughput.
- Local Rust execution is blocked: no installed Cargo/rustc; workspace
  rustup installation fails because this runtime lacks `/proc/self/exe`.
  Existing baseline CI is evidence of its earlier run, not a local rerun.
- Independent corpus evaluation, fresh Rust benchmarks and algorithmic
  speed/accuracy improvements are still outstanding. No new speedup or
  recognition improvement is claimed by this checkpoint.

## Standing instruction for this chat

```text
Use AGENTS.md to pursue the cqt-rs goal in this chat. Resume from docs/PROGRESS.md, apply docs/GOAL.md, choose the next unblocked experiment, establish its baseline and acceptance criteria, implement and verify it, and record proof before claiming success. Continue useful authorized work without repeated permission requests. Preserve failed cases and rejected ideas; never weaken evidence or count a blocker as completion. Checkpoint the exact next action before yielding. Deliver measured improvements as focused draft PRs; do not merge or release automatically.
```
