# Engineering notes for cqt-rs 0.2

Measurement correction (post-PR #3): the monitor uses `Instant::elapsed`,
so the historical monitor "CPU" percentages below are elapsed wall time
divided by audio duration, not process CPU measurements. The legacy CLI
`cpu_seconds` field has the same misnomer and includes input waits in live
mode. New evaluator summaries fingerprint their binary and inputs; an
external binary must not be attributed to the evaluator checkout's commit.
See [GOAL.md](GOAL.md) for the subsequent research and measurement protocol.

Compact record of what was found, what was decided, what was measured and
what to do next. Written for whoever continues the fingerprinting work.

## 1. What was wrong in 0.1 (and why it mattered for fingerprinting)

| Problem | Effect |
| --- | --- |
| One full-length Hann window for every bin | Constant (STFT-like) resolution, not constant-Q: low bins overlapped, high bins smeared onsets. A pitch shift did not translate the output cleanly. |
| Kernel applied without conjugation or `1/N` | Magnitudes ~1e7, only correct by luck of real input. |
| `num_bins = B * ceil(log2(fmax/fmin))` | Bins above `max_freq`, above Nyquist for the benchmark settings. |
| `num_frames = len / hop`, padding `window - hop` | Tail dropped, frames offset by half a hop. |
| Nested rayon `par_for_each` inside a parallel frame loop, new `FftPlanner` per call | Overhead, allocations per call. |
| Global `lazy_static` tables for Q, ratios, phase factors | Slower than recomputing; removed. |
| `hann-rs 0.1.0` ships `cargo.toml` (lowercase) | Cannot build on Linux from crates.io. |

## 2. Design of 0.2

- **Atom** for bin `k`: `a_k[n] = w_k[n] · exp(j·2π·f_k·(n − c)/sr)`, Hann
  (periodic) of length `N_k = filter_scale · sr / (f_k·α + γ)`,
  `α = 2^(1/B) − 1`. Gain `2 / Σw` so a real sinusoid of amplitude A reads A.
  librosa's output equals ours times `length/2` (`scale=False`) or
  `sqrt(length)/2` (`scale=True`); see `scripts/fingerprint_demo.py`.
- **Spectral kernel** (Brown & Puckette): `X_k = (1/N) Σ_m X[m]·conj(A_k[m])`,
  built in f64, pruned to keep 99 % of L1 mass (`sparsity`), stored as
  contiguous runs (`src/kernel.rs`), positive and negative halves evaluated
  against the real-FFT spectrum and its conjugate.
- **Multi-rate** (`src/plan.rs`, `src/levels.rs`, `src/filter.rs`): octaves
  cut from the top (top octave always full), octave `g` runs at `sr / 2^g`.
  Kaiser half-band filter, 80 dB, transition from the worst group's
  pass-band edge; zero-phase decimation (`y[n] = Σ h[k] x[2n + c − k]`) so
  batch and stream are bit-identical. Polyphase block form vectorizes.
  Capped kernels (`max_kernel_length`) pick a shallower level if their
  pass-band would reach the decimated Nyquist.
- **Latency** `= max_g (N_g/2 · 2^d + (delay − 1)(2^d − 1))`; exact when
  `hop % 2^(levels−1) == 0` (`Cqt::hop_alignment`).
- **Streaming** keeps per-level buffers with absolute origins, emits frame
  `i` when every group has `(i·hop >> d) + N_g/2` samples, compacts dead
  prefixes; `flush` pads zeros only up to the batch frame count.
- **Why single-rate was too slow**: with one 32768-point FFT a 47-sample
  top-octave kernel spans ~2800 FFT bins; the product streamed 500 KB of
  weights per frame and was memory-bound (~1.2 ns per coefficient regardless
  of loop shape). Multi-rate cut non-zeros 50× and the FFTs to 128–512.

## 3. Measurements (4-core container, Rust 1.98)

| Configuration | Per frame (1 thread) | 30 s / 10 s batch | Build |
| --- | ---: | ---: | ---: |
| Legacy grid 14.6–7902 Hz, 12/oct, 22 kHz, hop 1760, multi-rate | 9.3 µs | 4.06 ms (1.74 ms reused workspace) | 0.67 ms |
| Same, single-rate | 158 µs | 15.7 ms | 45 ms |
| 0.1 on the same machine | ~20 µs on 4 threads | 7.44 ms | 1.50 ms |
| Fingerprint 55–7040 Hz, 24/oct, 44.1 kHz, hop 512 | 10.1 µs | 2.94 ms per 10 s | 2.0 ms |
| Fingerprint + `gamma(10)` | 8.6 µs, latency 69 ms | | 1.8 ms |

Remaining batch cost is ~half decimation (memory-bound), ~half FFTs.
Accuracy: multi-rate vs single-rate within 1 % of peak; vs librosa on real
music 0.20 dB mean, r = 0.9996.

## 4. Fingerprint experiment (`scripts/fingerprint_demo.py`)

Reference: 30 s of *Vibe Ace* (CC BY). Queries: 10 s chunks of modified
versions. Features: dB CQT (hop 256); peaks = local maxima over ±24 frames ×
±9 bins that stand ≥ 15 dB above the mean of that neighbourhood, floor −70
dB (≈600 peaks / 30 s); Panako-style triplet hashes
`(Δbin12, Δbin23, round(32·(t2−t1)/(t3−t1)))` with zone 320 frames, fan-out
6, storing `(t1, b1, span)`. Matching: lookups with ±1 bin and ±1 ratio
step, buckets larger than 8 skipped; tempo per match from span ratio
(offset-free); vote in a (Δbin, tempo) histogram; offset from
`t_ref − tempo·t_query`; consistent = ±1 bin, ±3 % tempo, ±1 s offset;
fewer than 30 consistent matches = no identification.

Per-chunk scores (consistent hashes / chunk hashes): original 93/54/57 %,
pitch ±: 11–20 %, tempo ±: 12–17 %, speed +6 %: 54–68 %, clip: 29–65 %,
noise 10 dB: 26–29 %, same song other half: 9–14 % (located at repeated
riffs), unrelated piece: 0.1 %. Shift, tempo and position recovered exactly
in every chunk (position within 0.1 s). Growth with query length (middle
chunk, consistent matches after 1/2/3/5/10 s): original 169/861/1880/3662/
8675, worst modified version (tempo −8 %) 5/38/177/742/1949, unrelated
piece 6/7/7/10/10.

Tuning (offline on cached dumps, `evaluate` in the sweep scripts):
- The old fixed threshold (−50 dB below the global maximum) gave noise
  6–18 %, clip 20–57 %, pitch/tempo 6–16 %. Prominence over the local mean
  is the single largest gain (noise → 26–29 %); 12–18 dB all work, 15 dB
  chosen; ±9 bins beats ±6 (fewer, stabler peaks), ±12 loses matches;
  ±32 frames is a wash, ±16 hurts.
- Per-second or per-octave peak caps did not help on this material.
- Hash parameters matter little: ratio quantization 16–32 equivalent (32
  chosen, 12 raises the control), zone ≥ 320 frames saturates, fan-out 8–10
  gives more absolute matches but a proportionally higher control, ratio
  tolerance 0 or 2 both worse than 1, bin tolerance 1 needed for the
  vocoder cases.
- The control is ~10 matches per chunk whatever the setting, so the
  separation ratio is noisy; judge by the weakest true chunk and matches
  per second.

Lessons:
- Estimating tempo from absolute time ratios only works for offset-zero
  queries; use spans inside the hash or pairs of matches.
- A median estimate lands between clusters; vote (histogram mode).
- Another part of the same song is not a negative control for music with
  repeated riffs.
- Chunk 1 of "original" scores ~93 %, later chunks ~55 %: triplets that
  cross a chunk edge are lost and zero padding changes edge frames. Expect
  this for any windowed query.
- Score ≈ (peak survival)³: the vocoder-based pitch/tempo variants keep
  ~60 % of their peaks (transient smearing), so triplets cap near 20 %;
  resampling keeps 90 %, noise 85–90 %, clipping 75–85 %. Raising survival
  (peak picking) pays three times; hash tweaks pay once.

## 5. Next steps, in order of expected payoff

1. **Decision rule for streaming**: keep a running (Δbin, tempo, offset)
   vote over a sliding window; declare a match when the best cell holds
   ≥ 10× the second-best song and ≥ 30 hashes. From the growth curve this
   fires after 2–3 s for every distortion tried, ~1 s for clean audio.
2. **Hash design**: pair hashes `(Δbin, round(8·log2 Δt))` with the value
   `(t1, b1, Δt)` give tempo per match as `Δt_ref/Δt_query` and survive as
   (peak survival)², so they should double the vocoder-case scores for
   short queries; their keys carry less entropy (~2000 distinct), so
   combine with triplets or score them with a stricter offset vote in a
   catalogue.
3. **Index**: hash → (song id, t1, b1, span) in a sorted/flat map; score by
   the largest (Δbin, tempo, offset) cell per song; report position.
4. **Peak survival under vocoding**: a wider time radius on the query side
   only (±32) or matching peaks by time within ±3 frames instead of exact
   triplet ratios may recover part of the smeared transients; measure with
   `peak_survival` first, then hashes.
5. **Transform settings for streaming**: `gamma` 10–25 Hz cuts latency to
   ~70 ms at 44.1 kHz; hop 256 helps tempo estimation; `bins_per_octave`
   24 keeps semitone shifts on integer bins (36 for finer detune).
6. **Library**: consider exposing `Kernel::apply` on a caller-supplied
   spectrum for users with their own FFT; a `CqtStream::push_interleaved`
   for stereo capture; SIMD (`std::simd`) once stable for the run product.
7. **Decimation cost**: the half-band filter is ~half of batch time; a
   two-stage design (short first stage, longer second) or `f32x8`
   intrinsics would halve it. Not needed for real-time.

## 6. Housekeeping

- 0.2 is a breaking release; migration table in `CHANGELOG.md`.
- CI: fmt, clippy (with and without `parallel`), doc, tests on stable and
  1.98, bench compile. Python validation is manual (`pip install numpy scipy
  soundfile librosa matplotlib`).
- Audio is downloaded at run time from the librosa data mirror with SHA-256
  checks; nothing copyrighted is committed. Attribution: Kevin MacLeod,
  CC BY 3.0 (*Vibe Ace*, *Dance of the Sugar Plum Fairy*).

## 7. Radio monitoring (`monitor/`, `scripts/radio_sim.py`, `scripts/radio_eval.py`)

Goal: a watch list of songs, a continuous stream with DJ treatment, a
confidence score with "below 70 is not a match", real time on a small
budget. This is the inverse of Shazam (few songs, endless query), which
makes the index tiny and lets evidence accumulate in a sliding window.

Design decisions and why:
- **Rust port is bit-identical** to the Python picker/hasher (verified on
  the 30 s excerpt: 617 peaks, 51 109 hashes, same sets). Edge handling
  replicates SciPy `mode="nearest"` by repeating the first/last frame and
  bin, so batch and stream agree; the picker delay is `time_radius`
  frames, the hasher delay is `zone` frames.
- **Votes use the cell's quantized tempo and a rolling origin.** With
  `offset = t_ref − tempo·t_query` in absolute frames, the ~1 % tempo
  error of a single match (integer spans) scatters votes over
  `0.01·t_query` frames, i.e. 8 s after 800 s of stream. Computing the
  offset with the cell tempo and against an origin that moves every four
  windows (cells are re-keyed, `Matcher::rebase`) keeps the scatter under
  one cell; evidence went from ~900 to ~4000 on the same plays and no
  longer depends on how long the monitor has been running (unit test
  `evidence_does_not_decay_late_in_a_long_stream`).
- **Evidence = best cell + 26 neighbours**, evaluated for every occupied
  cell. A first version skipped cells with fewer than ⅓ of the song's
  strongest cell to save CPU; review found the counterexample (an
  isolated 30-vote cell hides a 27-cell cluster of nines whose
  neighbourhood is 243), so the search is exact again and the cost is
  measured instead (see §8). Ties go to the centre with more votes, then
  the smallest key; shift, tempo and offset are vote-weighted means over
  the winning neighbourhood.
- **Votes remember their own tempo and offset** so that expiring a vote
  subtracts exactly its contribution; subtracting the cell mean kept the
  window's old mean alive (offsets `[−40, −40, +40, +40]` expiring the
  first two would still report 0).
- **Per-song tracking** so two songs can be active during a crossfade;
  a new play of the same song is declared when the predicted position
  jumps by > 3 s, the shift by > 2 bins or the tempo by > 0.05 for a full
  second (a repeated riff briefly wins the vote at the tail of a play).
- **Confidence** `100·n/(n+half)`, `half = 2 × max null evidence`. The
  null maximum over 31 min was 25 with absolute offsets and 51 with the
  rolling origin (true evidence rose 4×, null 2×), hence `half = 100`
  at fan-out 6. With the default fan-out of 4 the null maximum is 20
  and `half` 40; threshold 70 ⇔ evidence ≥ 98 ⇔ 4.9× the worst null
  cell, the same margin.
- **Fan-out 4 by default** (§9): the null ceiling, the true evidence,
  the index and the CPU all scale down together (20 vs 50, 1 750 vs
  4 450 median play evidence, 24 vs 51 kB per song-second, 0.4 vs
  0.9 % of a core), so the evidence margin is unchanged while
  detections come 0.3 s earlier (fewer hashes per anchor complete
  sooner). Fan-out 6 stays available (`--fan-out 6 --half 100`).
- **Alignment.** The votes are indirect evidence; once a hypothesis
  wins, `PeakTrack::verify` maps the last 2 s of stream peaks onto the
  song (`frame_ref = tempo·frame + offset`, `bin_ref = bin − shift`,
  tolerance ±4 frames, ±1 bin) and reports the fraction that land on a
  song peak. On the null streams that fraction never exceeds 0.30, on a
  play it is 0.5–0.9 (0.26 in a crossfade's fade-in). The tracker uses
  it twice: a start needs alignment ≥ 0.4 besides confidence 70, which
  rejects a reversed copy of the song (votes 82–254, alignment ≤ 0.29);
  and an active play is held while alignment ≥ 0.3 and confidence ≥ 35,
  which carries a play through a quiet passage where the votes thin out
  (`sweet_waltz key_change_-1`, 4 s at evidence 50–90, previously an
  `end` and a second `start`). Cost: about a hundred binary searches per
  report.
- **Bucket cap** 8 per `(key, song)`, enforced when the index is built:
  a key that occurs more than 8 times in one song loses that song's
  entries and keeps every other song's. The first version compared the
  whole bucket with `8 × songs`, which made one song's evidence depend on
  how many unrelated songs were watched.
- **Hashes are released early.** An anchor's hashes depend only on its
  first `3·fan_out` candidates, so once that many later peaks inside the
  zone are known the anchor is emitted (in order) instead of waiting for
  the 320-frame zone to pass. The hash sequence is provably identical
  (tests against brute force with random watermark steps, sparse and
  dense peaks, fan-out 0–6); the worst case is still `zone`.
- **Detection tracker** (`Tracker`) is a library type with unit tests for
  start, release, brief versus persistent jumps, the alignment gate and
  hold, and end-of-stream; the binary only formats its events (strings
  JSON-escaped). The whole chain behind the transform is one
  `Fingerprinter` (dB → peaks → hashes) shared by the binary and the
  end-to-end test `monitor/tests/end_to_end.rs`, which detects a
  synthetic song played 4 % faster and higher between noise and an
  unwatched song without any audio file.

Measured (4-core container, one thread, defaults: fan-out 4, `half`
40, alignment gate; the fan-out 6 numbers after review round 1 in
brackets):

| | Calibration null stream (31 min) | Programme (15 min) |
| --- | --- | --- |
| Evidence | max 20, 99.9 % 19, median 7 (50 / 46 / 17) | hundreds to thousands during plays |
| Confidence | max 33.3, no start event | 16/16 plays, 0 false starts |
| Alignment | max 0.28, 99.9 % 0.25 | 0.49–0.84 at detection, crossfade 0.61 |
| Detection after play start | | median 1.8 s (2.3 s), max 13.3 s (talk-over), crossfade 7.5 s (6.8 s) |
| Hash delay (anchor → hashes) | median 0.73 s, max 2.0 s (1.13 s) | median 0.73 s (1.02 s) |
| CPU | 0.36 % of one core (0.86 %) | 0.40 % (0.94 %) |
| Index | 73 k hashes (540 dropped by the per-song cap), 2.7 MB for 111 s of songs (169 k, 5.7 MB) | |

DJ set (`scripts/radio_dj.py`, `radio_eval.py --programme stream_dj`):
8.7 min, no silence, every item crossfaded over 6 s, nine plays of the
watched songs with a key-locked BPM change and a key change and 12 s of
talk-over at once, three of them through FM and MP3 as well. 9/9
detected, 0 false starts, position within 0.1 s; from the end of the
talk-over (or of the fade-in when there is none) 1.0, 1.5, 4.2 s
without talk-over and −4.0 (during the talking), 2.2, 3.9, 5.6, 8.4,
17.6 s with it; peak confidence 81–97. The two restarts this stream first produced were the
song's own repeated section flipping the vote to the equivalent
position 4 s away, so the position-jump tolerance of the tracker is now
10 s (`--jump`, was 3 s): a restarted track jumps by far more, a
repeated bar by far less. The programme, the eight-song run and the
negatives are unchanged by it except that looped samples restart less
(7 starts on the hard stream instead of 14).

Second null stream and hard negatives (`scripts/radio_negatives.py`;
the evidence threshold was calibrated on the stream above only, the
alignment thresholds were chosen after looking at all three; every
stream draws on the same eight tracks and three speech recordings, so
none of this is independent material):

| Stream | Minutes | Max evidence | Max confidence | Max alignment | Starts |
| --- | ---: | ---: | ---: | ---: | ---: |
| Second null programme, seed 7777 | 30.7 | 20 (fan-out 6: 58) | 33.3 | 0.30 | 0 |
| Hard negatives, same artist (sugar_plum, 11 treatments) | 6.5 | 14 (34) | 25.9 | 0.30 | 0 |
| Hard negatives, watched songs reversed | 3.9 | 82 (254) | 67.2 (71.8) | 0.29 | 0 (2) |
| Hard negatives, watched songs at 0.6× and 1.5× speed | 3.1 | 58 (197) | 59.2 (66.3) | 0.30 | 0 |
| Hard negatives, 0.5 s loop of a watched song | 2.0 | 122 | 75.3 | 0.33 | 0 |
| Hard negatives, 1 s loop | 2.0 | 397 | 90.8 | 0.62 | 0 |
| Hard negatives, 2 s loop | 1.9 | 1 089 | 96.5 | 0.88 | 3 |
| Hard negatives, starts whose window spans two segments | | | | | 4 (loop → silence, loop → loop) |
| Hard negatives, speech between them | 1.7 | 10 | 20.0 | 0.25 | 0 |

A report is charged to a segment only when its whole evidence window
lies inside the segment; a start whose window spans two segments is
counted under "transition" with the segments named, so every alarm is
accounted for. The second null stream reproduces the calibration
ceiling exactly at fan-out 4 (at fan-out 6 it exceeded it, 58 against
50, still at confidence 37). Loops of one second and more
are the song's own audio repeated; the matcher finds them (7 starts on
the stream against 38 before the gate and the 10 s jump tolerance) and the reported
position jumps back once per loop, which is the cue a policy that
excludes sampled loops would use.

Eight-song watch list (every music track of the simulation, 510 s of
audio, run over the 31 min stream that is built from six of them, so
the stream is mostly watched material and the two absent songs measure
false alarms):

| | Fan-out 6, `half` 100, votes only | Fan-out 4, `half` 42, votes only | Fan-out 4, `half` 40, alignment gate (default) |
| --- | --- | --- | --- |
| Index | 727 k hashes, 16.1 MB, built in 1.1 s | 316 k hashes, 7.5 MB | 316 k hashes, 7.5 MB |
| Plays detected | 44 / 45, 1 extra start, 0 starts outside a play | 44 / 45, 2 extra starts, 0 outside | 45 / 45, 0 extra starts, 0 outside |
| Detection after play start | median 2.5 s, max 9.3 s | median 2.5 s, max 9.3 s | median 2.4 s, max 9.1 s |
| Absent songs, max confidence | 16.0 / 18.7 | 16.0 / 16.0 | 16.0 / 16.0 |
| Hash delay median | 1.13 s | 0.80 s | 0.80 s |
| CPU | 3.0 % | 0.7 % | 0.7 % |

The miss of the first two columns is a talk-over play of *brahms*
(speech over quiet strings for 12 s of a 42 s play), which the alignment
hold now carries through; the extra starts were a repeat inside *brahms*
`key_change_-1` that the tracker took for a new play, which the gate
now declines because the repeat's hypothesis does not align. Memory
grows linearly with watched audio (about 4 MB per 3 minutes); CPU grows
with the number of matches, which in this run is dominated by the six
songs that really are playing. A 100-song watch list on an ordinary
programme (most of the stream unwatched) has not been measured.

Known limits:
- Talk-over with speech louder than the music: evidence 20–40 (confidence
  30–50) while the speech lasts, detection 1–3 s after it stops.
- A crossfade is detected ~1.5 s after the fade-in completes: the
  alignment gate waits until the song is audible in the last 2 s.
- A looped sample of a second or more of a watched song is reported as
  the song (see the hard negatives above).
- `end` events lag the real end by window + release + delay (8–9 s).
- Two phase-vocoder passes (key-locked BPM change, then a key change)
  leave confidence 80–90 instead of 98; with FM and MP3 on top, the
  talk-over intro of such a play is followed by ten seconds at 40–60
  before the threshold (DJ set below).
- Only tested at 44.1 kHz mono; the binary refuses mismatched rates.

Next steps for monitoring, in order:
0. Independent evaluation: with the configuration frozen, run the
   monitor on CC-licensed recordings that were used nowhere in
   development (other artists, real broadcast or DJ recordings) and
   report the false-alarm bound and the recall from that.
1. Speech-robust evidence: down-weight peaks in 100–1000 Hz during talk
   (or two windows, 5 s and 15 s, and report the better confidence) to
   catch the talk-over intro; measure on `pitch_fader_+4_talkover`.
2. Index serialization (`Index` to/from a file) and a `--listen` mode that
   reads PCM from stdin so the binary can sit behind an FM/stream decoder.
3. End a play on the alignment rather than on the evidence release: the
   alignment drops within a second of the song stopping, the votes take
   the window to decay.
4. A second real recording (phone mic in a room) to confirm the FM chain
   simulation is not optimistic.
5. A loop policy: declare a sampled loop when the reported position jumps
   back by the same amount at a fixed period.
6. Maintain the matcher's slab histogram incrementally to take the exact
   search off the per-report cost.

## 8. Review round 1 (PR #3 comments) and what was done

Every inline finding was reproduced against the code and fixed; the
design comments were sorted into "done now", "measured and deferred" and
"next experiment". Commit `4cea498` carries the code, this commit the
measurements.

| Finding | Verdict | Change |
| --- | --- | --- |
| `radio_sim.py` deletes every file in `RADIO_SIM_TMP` | correct, P1 | the run creates its own `TemporaryDirectory` under that location and removes only that |
| Key hasher shifts the product right by 20, zeroing hashbrown's tag bits | correct | multiply and fold the high half into the low half; `monitor/benches/index.rs` measures 4 096 tolerant lookups: hits 5.80 → 2.77 ms, misses 4.80 → 0.53 ms |
| Bucket cap `8 × songs` on the whole bucket | correct | cap enforced per `(key, song)` when the index is built (`Index::dropped` reports the count); a song's evidence no longer changes when unrelated songs are added |
| `CqtWorkspace` / `CqtStream` accept any transform | correct | both record the parameters they were built for; `process_with` rebuilds a foreign workspace, the stream panics on a foreign transform and `reset` re-binds |
| Expiry subtracts the cell mean, not the vote | correct | votes carry tempo and offset; expiry subtracts them exactly, rebase shifts them with their cell |
| Pruning drops every coefficient tied at the boundary | correct | ties are dropped as a run or kept as a run; the two-tap example keeps both coefficients |
| Filter designed before the tap check | correct | `HalfBand::taps_for` decides first; the 384-bins-per-octave example is rejected in microseconds |
| Flush padding not in the frame timeline | correct | padding joins the timeline once audio follows it (`padding_samples`); a repeated flush emits nothing; test compares the continuation with the batch transform of the padded signal |
| JSON built by interpolation | correct | strings escaped; the start/end state machine is a library `Tracker` with unit tests |
| `best_per_song` skips centres by their own count (30 vs 243 example) | correct | exact search over every occupied cell with an upper bound from a (song, shift, offset) histogram; oracle test against the full scan on 300 random histograms |
| Early hash emission | adopted | anchor released when its `3·fan_out` candidates are known; hash delay median 2.0 → 1.0 s, detections up to 1.0 s earlier (median 3.0 → 2.3 s, `plots/radio_compare.png`), hash sequence unchanged |
| Grouped ratio directory (27 → 9 probes) | deferred | after the hasher fix a lookup costs about 0.7 µs and the null stream does 1 400 lookups/s, i.e. 0.1 % of a core; the bound-based search and the pipeline itself dominate. Worth revisiting only with hundreds of songs |
| Correlated triplets, distinct-anchor evidence | tried, rejected | §9: the correlated votes are the signal; counting anchors cut the null ceiling by 30 % and the true evidence by 94 % |
| Candidate generation + verification stage | done | §9: observed first, then adopted as the alignment gate and hold; fan-out 4 is the default |
| Failure bundles | done | `eval_summary.json` records commit, monitor configuration, the `stream` event, and for the three slowest plays and every false start the ground-truth segment with the report trace around it |
| Awkward negatives, separate tuning/evaluation sets | done | `scripts/radio_negatives.py`: a second null programme and 21 min of hard negatives (same artist, reversed, out-of-range speed, loops), evaluated with `radio_eval.py --negatives` after calibrating on the first null stream only |
| Separate release gates for library and monitor | done | CI has `library-*` jobs (format, lint, docs, tests on stable and 1.98 with both feature sets, benches compile, `cargo publish --dry-run`, CHANGELOG heading for the crate version) and `monitor-*` jobs (the same checks plus the synthetic end-to-end detection); the library jobs never build the monitor |

Two clarifications the review asked for:
- **The 5 s target is measured from the first sample of the play in the
  stream**, crossfade and talk-over included, to the `start` event. The
  crossfade play therefore counts as 6.8 s even though the song is
  inaudible for most of its first 6 s; measured from full level it is
  0.8 s.
- **Zero alarms in 31 minutes bounds the false-alarm rate, it does not
  estimate it.** Under a Poisson model the one-sided 95 % upper bound is
  `−ln 0.05 / T` ≈ 5.8 per hour. Claiming one alarm per 100 hours at
  that confidence needs about 300 alarm-free hours of independent
  material, and the null set must be separate from the calibration set.
  The evidence margin (worst null cell 50 versus threshold 233) is the
  reason to expect the rate to be far lower than the bound, not proof.

Reproducing a before/after figure such as `plots/radio_compare.png`
(here the review-round result `f731b5d` against the current tree):

```console
git worktree add /tmp/before f731b5d
(cd /tmp/before && CARGO_TARGET_DIR=$PWD/../target/before cargo build --release -p cqt-monitor)
python3 scripts/radio_eval.py --monitor target/before/release/monitor --half 100 --label f731b5d --out eval_summary_f731b5d --no-plots
python3 scripts/radio_eval.py --negatives --label "fan-out 4, alignment gate"
python3 scripts/radio_compare.py eval_summary_f731b5d eval_summary
```

What the exact search costs: on the null stream every report (4 per
second) scans about 7 000 occupied cells. With the count-based pruning
the search was 0.3 % of a core; the exact search with the slab bound and
a fast integer hasher for the cell maps is about 0.5 %. Maintaining the
slab histogram incrementally (update on push, expiry and rebase instead
of rebuilding it per report) would remove most of that and is the next
optimization if CPU matters.

## 9. Experiments after review round 1

The four items at the top of the previous next-steps list, each run
against the calibration null stream, the programme, the second null
stream and the hard negatives (`scripts/radio_negatives.py`,
`scripts/radio_eval.py --negatives`), with `--arg` passing the variant's
monitor flags. Kept: what improved a measured number without costing
another; dropped: the rest.

### Distinct-anchor evidence (dropped)

Each vote recorded whether it was the first from its query anchor in
its cell, cells counted anchors next to votes, and the evidence
(neighbourhood sum, slab bound, tie-breaks) used the anchor count.
Result at fan-out 6:

| | Votes | Distinct anchors |
| --- | ---: | ---: |
| Null stream, max evidence | 50 | 35 |
| Programme, max evidence per play, min / median | 1 539 / 4 445 | 236 / 268 |
| Margin (play min ÷ null max) | 31× | 6.7× |
| With `half` at 2× null max: plays detected | 16 / 16 | 12 / 16, 13 restarts |

The correlated hashes of a true anchor pair are the signal: a matching
anchor pair contributes tens of consistent votes, a chance pair one or
two, so nearly all of the null evidence already comes from distinct
anchors while the true evidence is almost entirely repeats. Any cap on
votes per anchor moves the margin the wrong way. The code was removed;
the numbers are in the table and the method in this paragraph.

### Candidate-then-verify at fan-out 4 (kept)

Step 1, observation only: `--fan-out 4` with `half` 40 against the
fan-out 6 default. Every number moved the same way or stayed: null
ceiling 50 → 20, median play evidence 4 445 → 1 752 (margin 89× → 88×),
detections 0.1–0.7 s earlier (the anchors' `3·fan_out` candidates
complete sooner), CPU 0.9 → 0.35 %, index 5.7 → 2.7 MB, second null
stream's ceiling 58 → 20, reversed copies 254 → 82 votes (two false starts →
none). One regression: a quiet passage of `sweet_waltz key_change_-1`
dropped the evidence under 98 for 3 s and the play ended and restarted.

Step 2, observation only: `PeakTrack::verify` reported in every
`report` line. Over the whole 5 s window the alignment at the moment of
detection was 0.14–0.39 (the window still holds the previous segment),
over the last 2 s it is 0.49–0.84 with null maxima of 0.28–0.30; at the
reversed and looped segments' evidence peaks it is 0.01–0.23. Hence the
2 s span and the thresholds 0.4 (start) and 0.3 (hold).

Step 3, adopted: `Scored { candidate, confidence, alignment }` into the
tracker; start needs both, an active play is held by alignment ≥ 0.3 at
confidence ≥ 35. Programme: 16/16, 0 false starts, the restart gone,
crossfade detection 6.8 → 7.5 s (the only cost); eight-song run 45/45
with no extra start (was 44/45 with one or two); hard negatives: no
start on reversed or out-of-range copies, loops still start but 14
times instead of 38.

### Hard negatives and a second null set (kept, with a caveat)

`scripts/radio_negatives.py` renders `stream_null2` (seed 7777, the
same generator and source pool as the calibration stream) and
`stream_hard` (families in the ground truth's `family` key).
`radio_eval.py --negatives` charges a report to a segment only when the
whole evidence window is inside it, counts a start whose window spans
segments under "transition", and prints the per-family table. The
calibration ceiling transferred to the second stream exactly at fan-out
4 (20 = 20) and not at fan-out 6 (58 > 50), which is the reason to
calibrate with a margin (`half` at 2× the ceiling, threshold at 4.9×
it) rather than at the ceiling.

The caveat, raised in review round 2: a new seed reshuffles the same
eight tracks and three speech recordings, and the alignment thresholds
were chosen after observing these streams' scores, so the second
stream is not held out from the complete detector. It is development
validation. The Poisson bound of §8 (5.8 alarms per hour at 95 % from
31 alarm-free minutes) therefore still describes the development
material. An independent estimate needs the configuration frozen, which
it now is (fan-out 4, `half` 40, threshold 70, alignment 0.4 / 0.3,
jump 10 s), and recordings used nowhere in development; the librosa
example corpus that supplied every track here has no unused music, so
that material has to come from elsewhere (CC-licensed radio or DJ
recordings) and is the first next step.

### After the experiments: live input, per-play figures, a DJ set

`monitor --stream -` reads signed 16-bit mono PCM from stdin block by
block and flushes every report, so the binary sits behind
`ffmpeg -f s16le`; on the same 90 s of audio the events are identical
to the file path's. `radio_eval.py` takes `--programme` and draws
`plots/radio_plays*.png`, one panel per play with confidence, alignment
and the start/end events, which is the figure that shows what each
status looks like. `scripts/radio_dj.py` is the combined-treatment DJ
set reported in §7.

### Separate release gates (kept)

`.github/workflows/ci.yml` runs `library-check`, `library-test` (stable
and 1.98, both feature sets) and `library-package` (benches compile,
`cargo publish --dry-run`, a `# <version>` heading in `CHANGELOG.md`)
against `cqt-rs` alone, and `monitor-check` and `monitor-test` against
`cqt-monitor`, whose `monitor/tests/end_to_end.rs` detects a synthetic
song played 4 % faster and higher between noise and an unwatched song
(one start within 5 s at the right tempo, shift and position, one end,
confidence under 50 elsewhere). A library release needs the library jobs
green on the tagged commit; the monitor stays `publish = false` and its
jobs may be red without blocking a library release.

## 10. Review round 2 and what was done

| Finding | Verdict | Change |
| --- | --- | --- |
| [P1] The reported tempo and offset describe different lines: offsets are accumulated with each cell's quantized tempo, the candidate combines their mean with the mean unquantized tempo | correct; the synthetic tests had hidden it because their per-vote tempos split symmetrically over two cells | each cell also sums its votes' origin-relative query frames (shifted on rebase, subtracted on expiry); the candidate refits the offset with the reported tempo through the same correspondences, `mean(ref − tempo·q) = mean(offset) + mean((cell_tempo − tempo)·q)`. Regression test sweeps tempos 0.985–1.1 (a quarter, a half and a full cell off the centres) and plays starting at 0, 1300 (ending just before a rebase), 3500, 4200 (straddling one), 4400 and 8700 (a second one); it fails on the old code (alignment 0.04 at tempo 0.985) and passes now. On the audio: the between-centre DJ play (pitch fader −7 %, tempo 0.93) is detected 5 s earlier, everything else unchanged |
| [P2] A "persistent" jump could be two isolated jumps with weak evidence between them, and successive disagreeing candidates were not checked against each other | correct | `Active::pending` records the disagreeing hypothesis (shift, tempo, position, frames); a report agrees with it under the same jump tolerances or replaces it; a report with neither strong nor held evidence clears it. Test: the reviewer's sequence (strong jump, three weak reports, a different strong jump) produces no event; alternating hypotheses never confirm; ten uninterrupted frames of one do |
| [P2] The "held-out" claim overreaches: same pool, same generator, and the alignment thresholds were chosen after observing the negative scores | correct | wording changed everywhere to "second null stream, development validation"; the caveat and what an independent test needs are in §7 and §9, and the independent evaluation is item 0 of the next steps. The configuration is frozen at the defaults |
| [P2] Family false-start counts omitted starts whose window began in the previous segment | correct | every start is charged to a family, or to "transition" with the two segments named; a sum check enforces it. On the hard stream: 3 starts inside 2 s loops, 4 on transitions between loop segments, 7 in total as before |
