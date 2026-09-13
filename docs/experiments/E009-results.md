# E009: modal alignment and measured EQ robustness

The opt-in `--modal-fit` estimates shift, tempo and position from the strongest cell inside the winning neighbourhood. It preserves the neighbourhood evidence score, confidence threshold, verifier tolerances and default behavior. Existing `best_per_song()` semantics remain unchanged.

**Development improves; held-out identity detection is unchanged.** The original exact-snippet miss reproduces on PR #3 and PR #7 and is recovered by the candidate. All 22 reference hashes and the original self-control WAV exactly match E007. New transformations of those recordings are development/regression cases, not new held-out data.

## Real DJ mix regression

Same Toucan mix and 22 clean midpoint references, full 90.61-minute stream; slowed versions last 100.68 minutes. Voiceover remains 12 seconds every 30 at 0 dB local RMS.

| Treatment | PR #3 / PR #7 coverage | Modal coverage | Baseline / modal starts |
| --- | ---: | ---: | ---: |
| clean | 14/22 | 15/22 | 19 / 22 |
| pitch | 12/22 | 12/22 | 13 / 16 |
| tempo | 12/22 | 13/22 | 13 / 16 |
| combined | 5/22 | 8/22 | 6 / 10 |

This is publisher-track-list coverage, not independently timed per-play precision. Every start remains in the local raw evidence; the large archive is not uploaded. Full-track reference diagnostics from E007 are still incomplete; this rerun evaluates the requested ten-second references.

## Controlled ten-second snippets

Each original midpoint snippet appears once, after five seconds of silence and before ten seconds of silence. Truth comes from those sample positions, independently of the detector. Correct starts must occur during the known clip or at most one second after its end. Out-of-window or wrong-song starts count as false; additional starts inside a valid window count as duplicates. Slowed snippets last 11.11 seconds.

EQ, pitch and tempo cases apply a common 0.15 gain for headroom. EQ cases were checked for absence of PCM clipping. Controlled voiceover covers each entire snippet at equal local RMS, using the fixed Flite text; this is more speech exposure than the periodic DJ-mix treatment. Hard clipping is a separate nonlinear treatment at 25% of the source peak, then scaled for headroom.

| Treatment | Development baseline | Development modal | Held-out baseline | Held-out modal |
| --- | ---: | ---: | ---: | ---: |
| self | 21/22 | 22/22 | 8/8 | 8/8 |
| clean | 21/22 | 22/22 | 8/8 | 8/8 |
| bass_-12 | 21/22 | 22/22 | 8/8 | 8/8 |
| bass_-6 | 21/22 | 22/22 | 8/8 | 8/8 |
| bass_+6 | 20/22 | 22/22 | 8/8 | 8/8 |
| bass_+12 | 20/22 | 22/22 | 8/8 | 8/8 |
| treble_-12 | 21/22 | 22/22 | 8/8 | 8/8 |
| treble_-6 | 21/22 | 22/22 | 8/8 | 8/8 |
| treble_+6 | 21/22 | 22/22 | 8/8 | 8/8 |
| treble_+12 | 22/22 | 22/22 | 8/8 | 8/8 |
| notch_broad | 19/22 | 22/22 | 8/8 | 8/8 |
| notch_narrow | 20/22 | 22/22 | 8/8 | 8/8 |
| highpass | 21/22 | 22/22 | 8/8 | 8/8 |
| lowpass | 22/22 | 22/22 | 8/8 | 8/8 |
| pitch | 21/22 | 22/22 | 8/8 | 8/8 |
| tempo | 20/22 | 22/22 | 8/8 | 8/8 |
| pitch_tempo | 20/22 | 22/22 | 8/8 | 8/8 |
| voice | 17/22 | 18/22 | 6/8 | 6/8 |
| combined | 14/22 | 17/22 | 5/8 | 5/8 |
| clipping | 19/22 | 20/22 | 8/8 | 8/8 |

Across the controlled grid there are 27 recovered recording/treatment cases and 0 lost cases. These are correlated conditions, not 600 independent recordings. PR #3 and PR #7 agree on all 40 controlled cases.

Development EQ: paired mean coverage change 5.68 percentage points; recording-level percentile bootstrap 95% interval [0.00, 13.64] over 22 recordings (10,000 resamples, seed 20260913).
Heldout EQ: paired mean coverage change 0.00 percentage points; recording-level percentile bootstrap 95% interval [0.00, 0.00] over 8 recordings (10,000 resamples, seed 20260913).

The zero-width held-out difference interval reflects identical outcomes on eight observed recordings; it does not bound unobserved failure modes. All eight held-out recordings are by one different artist. No configuration was selected using their detector results. Gains are concentrated in the development recordings, so broad unseen-recording improvement is unproved.

## EQ parameters and fingerprint survival

Twelve independently applied filters: bass shelf at 200 Hz and treble shelf at 3000 Hz, each -12/-6/+6/+12 dB with Q=0.707; 1000 Hz bell cuts of -12 dB at Q=0.7 and Q=5; two-pole 200 Hz high-pass and 3000 Hz low-pass. FFmpeg uses f64 filter precision. These are specific EQ settings, not an arbitrary-distortion guarantee.

| EQ | Reference peak survival mean / worst | Exact hash-key overlap mean |
| --- | ---: | ---: |
| bass_-12 | 95.7% / 87.1% | 79.7% |
| bass_-6 | 97.7% / 91.5% | 87.7% |
| bass_+6 | 97.5% / 91.0% | 87.1% |
| bass_+12 | 95.5% / 86.5% | 79.5% |
| treble_-12 | 85.1% / 70.1% | 60.2% |
| treble_-6 | 91.8% / 86.1% | 75.7% |
| treble_+6 | 93.3% / 85.0% | 76.7% |
| treble_+12 | 87.6% / 75.9% | 63.6% |
| notch_broad | 84.2% / 69.4% | 53.6% |
| notch_narrow | 91.8% / 85.5% | 71.4% |
| highpass | 97.6% / 91.0% | 84.6% |
| lowpass | 82.9% / 70.0% | 55.6% |

Peak survival is the fraction of reference peaks with a transformed peak within four frames and one bin. Hash overlap is the multiset intersection of exact hash keys divided by reference hash count; it does not itself prove a correct temporal alignment. Both were recomputed from native fingerprint dumps on all 30 recordings. The front end is unchanged by the candidate.

## False starts, delays and position limits

Candidate false starts across the controlled and negative grids: 0. Unwatched full-track control: eight complete recordings, 24.93 minutes. Separate negative EQ cases use eight one-minute midpoint excerpts (eight minutes per treatment); these excerpts overlap the full tracks and their variants are correlated. Do not sum them as independent exposure. With zero starts, 24.93 minutes alone gives a one-sided 95% Poisson upper bound of 7.21 starts/hour under that model, far from a production false-alarm guarantee.

| Split / treatment | Start delay p50 / p95, baseline → modal (s) | Worst position error, baseline → modal (s) | End-delay p50, baseline → modal (s) |
| --- | --- | --- | --- |
| development / clean | 1.704/2.277 → 1.674/2.144 | 0.021 → 8.713 | 9.708 → 9.708 |
| development / combined | 6.676/10.844 → 7.011/10.650 | 0.030 → 0.014 | 6.510 → 6.225 |
| heldout / clean | 1.533/2.573 → 1.464/2.573 | 0.013 → 0.005 | 9.791 → 9.791 |
| heldout / combined | 5.052/7.870 → 4.414/7.821 | 0.017 → 0.008 | 7.295 → 7.295 |

Delay summaries contain detected plays only; their denominators and misses remain in the coverage table. They are not a proof of lower latency at matched recall. Position error uses the known reference time at the reported audio frame. The recovered t01 clean start can select a repeated passage and is about 8.7 seconds wrong in position despite identifying the song. This remains a concrete limitation, visible in the worst-case column. No universal alignment fix is claimed.

All timing runs are single observations and share this execution host with preparation work. Outer wall times include process startup and index construction; the monitor’s legacy `cpu_seconds` is elapsed stream-processing time, not process CPU. No speedup or representative performance gate is claimed.

## Decision and proof

The final file audit found nine truncated auxiliary audio caches, two empty fingerprint dumps and eight truncated raw result logs; the cause is unestablished. All reference WAVs and assembled detector inputs retained their recorded hashes. Audio caches and fingerprint dumps were restored only after matching their exact original SHA-256. The eight affected native commands were replayed into separate files, all with identical non-timing event digests. Original truncated logs, original result records, hash mismatches and replay provenance remain preserved locally. The 162 reviewed cases include these eight replacement executions; the audit does not claim the original damaged logs were complete.

Retain the candidate as an **experimental opt-in**, not a new default. The original snippet failure is recovered and the controlled grid has no identity losses or added false starts. The eight recording-disjoint held-out tracks show preserved identity outcomes, not new gains. Voiceover and position ambiguity remain unresolved. The goal is incomplete.

Reviewed 162 completed native runs, 324 stdout/stderr hashes, 44 PR #3/PR #7 event-parity pairs, default-candidate parity and 390 fingerprint comparisons. Executable/input/evaluator hashes are retained before execution, with strict complete-output checks. The tested candidate source is `69c09c4be5fb391a9e3d93ff4465cc4864d0ddf4`; its binary SHA-256 is `d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`.

See [proof, source attribution and reproduction](../evidence/E009/README.md), [per-track data](../evidence/E009/per-track.json) and the neighboring frozen protocol. The source dataset combines Toucan CC BY-NC-SA 4.0 material with separately attributed Kevin MacLeod CC BY 4.0 recordings. Full source audio is not stored in Git.
