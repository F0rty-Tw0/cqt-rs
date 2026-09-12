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
versions. Features: dB CQT (hop 256), local maxima over ±24 frames × ±6
bins above −50 dB (≈950 peaks / 30 s), Panako-style triplet hashes
`(Δbin12, Δbin23, round(24·(t2−t1)/(t3−t1)))` with zone 240 frames, fan-out
6, storing `(t1, b1, span)`. Matching: lookups with ±1 bin and ±1 ratio
step; tempo per match from span ratio (offset-free); vote in a
(Δbin, tempo) histogram; offset from `t_ref − tempo·t_query`; consistent =
±1 bin, ±3 % tempo, ±1 s offset.

Per-chunk scores (consistent hashes / chunk hashes): original 96/49/54 %,
pitch ±: 8–16 %, tempo ±: 6–14 %, speed +6 %: 51–64 %, clip: 20–57 %,
noise 10 dB: 6–18 %, same song other half: 7–10 % (located at repeated
riffs), unrelated piece: 0.1–0.2 %. Shift, tempo and position recovered
exactly in every chunk (position within 0.1 s).

Lessons:
- Estimating tempo from absolute time ratios only works for offset-zero
  queries; use spans inside the hash or pairs of matches.
- A median estimate lands between clusters; vote (histogram mode).
- Another part of the same song is not a negative control for music with
  repeated riffs.
- Chunk 1 of "original" scores ~96 %, later chunks ~50 %: triplets that
  cross a chunk edge are lost and zero padding changes edge frames. Expect
  this for any windowed query.

## 5. Next steps, in order of expected payoff

1. **Peak picker**: local adaptive threshold (dB above the local mean, e.g.
   12 dB over a ±24×±6 window) gave 26 % vs 19 % on the noisy case in trials
   but must be re-tuned with the chunked protocol; add per-band peak caps
   so bass does not dominate.
2. **Hash design**: keep triplets (tempo-invariant) but add pair hashes with
   coarse `Δt` for very short queries; quantize the ratio to 1/32 with ±1
   tolerance; store `(t1, b1, span)` as now so tempo is offset-free.
3. **Index**: hash → (song id, t1, b1, span) in a sorted/flat map; score by
   the largest (Δbin, tempo, offset) cell per song; report position.
4. **Transform settings for streaming**: `gamma` 10–25 Hz cuts latency to
   ~70 ms at 44.1 kHz; hop 256 helps tempo estimation; `bins_per_octave`
   24 keeps semitone shifts on integer bins (36 for finer detune).
5. **Library**: consider exposing `Kernel::apply` on a caller-supplied
   spectrum for users with their own FFT; a `CqtStream::push_interleaved`
   for stereo capture; SIMD (`std::simd`) once stable for the run product.
6. **Decimation cost**: the half-band filter is ~half of batch time; a
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
