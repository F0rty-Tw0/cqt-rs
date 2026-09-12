# Engineering notes for cqt-rs 0.2

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
  rolling origin (true evidence rose 4×, null 2×), hence `half = 100`.
  Threshold 70 ⇔ evidence ≥ 233 ⇔ 4.6× the worst null cell.
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
  start, release, brief versus persistent jumps and end-of-stream; the
  binary only formats its events (strings JSON-escaped).

Measured (4-core container, one thread, after review round 1):

| | Null stream (31 min) | Programme (15 min) |
| --- | --- | --- |
| Evidence | max 50, 99.9 % 46, median 17 | thousands during plays |
| Confidence | max 33.3, no start event | 16/16 plays, 0 false starts |
| Detection after play start | | median 2.3 s (was 3.0 s), max 13.6 s (talk-over), crossfade 6.8 s |
| Hash delay (anchor → hashes) | median 1.13 s, max 2.0 s (was a fixed 2.0 s) | median 1.02 s |
| CPU | 0.9 % of one core (0.33 % without reports; the exact search per report is the rest) | 0.9 % |
| Index | 169 k hashes (3 339 dropped by the per-song cap), 5.7 MB for 111 s of songs | |

Eight-song watch list (every music track of the simulation, 510 s of
audio, run over the 31 min stream that is built from six of them, so
the stream is mostly watched material and the two absent songs measure
false alarms):

| | Fan-out 6, `half` 100 | Fan-out 4, `half` 42 |
| --- | --- | --- |
| Index | 727 k hashes, 16.1 MB, built in 1.1 s | 316 k hashes, 7.5 MB |
| Plays detected | 44 / 45, 1 extra start, 0 starts outside a play | 44 / 45, 2 extra starts, 0 outside |
| Detection after play start | median 2.5 s, max 9.3 s | median 2.5 s, max 9.3 s |
| Absent songs, max confidence | 16.0 / 18.7 | 16.0 / 16.0 |
| Hash delay median | 1.13 s | 0.80 s |
| CPU | 3.0 % | 0.7 % |

The one miss is a talk-over play of *brahms* (speech over quiet strings
for 12 s of a 42 s play; the other talk-over play of the same track is
found in 2.3 s). The extra start is a real ambiguity: *brahms*
`key_change_-1` contains a repeat that the tracker takes for a new play
after one second of disagreement. Memory grows linearly with watched
audio (about 6–9 MB per 3 minutes at fan-out 6, half at fan-out 4);
CPU grows with the number of matches, which in this run is dominated by
the six songs that really are playing. A 100-song watch list on an
ordinary programme (most of the stream unwatched) has not been measured.

Known limits:
- Talk-over with speech louder than the music: evidence 50–70 (confidence
  30–40) while the speech lasts, detection 1–3 s after it stops; one of
  three such plays over quiet strings was missed altogether.
- A crossfade is detected ~0.8 s after the fade-in completes.
- `end` events lag the real end by window + delay (~7 s).
- Only tested at 44.1 kHz mono; the binary refuses mismatched rates.

Next steps for monitoring, in order:
1. Speech-robust evidence: down-weight peaks in 100–1000 Hz during talk
   (or two windows, 5 s and 15 s, and report the better confidence) to
   catch the talk-over intro; measure on `pitch_fader_+4_talkover`.
2. Index serialization (`Index` to/from a file) and a `--listen` mode that
   reads PCM from stdin so the binary can sit behind an FM/stream decoder.
3. Reduce `end` lag by ending on evidence decay slope rather than a fixed
   release.
4. A second real recording (phone mic in a room) to confirm the FM chain
   simulation is not optimistic.
5. Candidate generation at fan-out 4 followed by verification against the
   reference peak track near the predicted position (observation-only
   first), with hard negatives (shared loops, same-artist tracks) and a
   null set separate from the calibration set; see §8.
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
| Correlated triplets, distinct-anchor evidence | next experiment | needs its own calibration; the replay bundle below is the tool for it |
| Candidate generation + verification stage | next experiment | agreed as the way to make fan-out 4 the default; run observation-only first |
| Failure bundles | started | `eval_summary.json` now records commit, monitor configuration, the `stream` event, and for the three slowest plays and every false start the ground-truth segment with the report trace around it |
| Awkward negatives, separate tuning/evaluation sets | agreed, not done | the eight-song run adds six more tracks as targets but no shared-sample or same-artist negatives; the null set was also the calibration set |
| Separate release gates for library and monitor | agreed | the monitor crate stays unpublished; the library findings above are the transform's own acceptance list |

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

Reproducing the before/after figure (`plots/radio_compare.png`):

```console
git worktree add /tmp/before 09978b8
(cd /tmp/before && CARGO_TARGET_DIR=$PWD/../target/before cargo build --release -p cqt-monitor)
python3 scripts/radio_eval.py --monitor target/before/release/monitor --label 09978b8 --out eval_summary_before --no-plots
python3 scripts/radio_eval.py --label f731b5d
python3 scripts/radio_compare.py eval_summary_before eval_summary
```

What the exact search costs: on the null stream every report (4 per
second) scans about 7 000 occupied cells. With the count-based pruning
the search was 0.3 % of a core; the exact search with the slab bound and
a fast integer hasher for the cell maps is about 0.5 %. Maintaining the
slab histogram incrementally (update on push, expiry and rebase instead
of rebuilding it per report) would remove most of that and is the next
optimization if CPU matters.
