# cqt-rs

Constant-Q transform (CQT) for audio fingerprinting and real-time spectral
monitoring, in safe Rust.

[API documentation](https://docs.rs/cqt-rs) · [Changelog](./CHANGELOG.md)

The transform maps a signal onto a logarithmic frequency grid with a fixed
number of bins per octave. Every bin has its own analysis window whose length
is inversely proportional to its centre frequency, so all bins share one
quality factor `Q`. Two consequences make the representation a good front end
for fingerprinting:

- a pitch shift of the audio is a translation along the bin axis;
- a tempo change is a translation along the frame axis.

<p align="center">
  <img src="./plots/cqt_spectrogram.svg" width="49%" />
  <img src="./plots/pitch_shift.svg" width="49%" />
</p>

## Installation

```toml
[dependencies]
cqt-rs = "0.2"
```

cqt-rs 0.2 requires Rust 1.98 or newer. The `parallel` feature (enabled by
default) processes batch frames on a rayon thread pool; disable default
features for a single-threaded build.

## Usage

### Batch

```rust
use cqt_rs::{Cqt, CqtParams};

let params = CqtParams::builder(44_100, 55.0, 7_040.0)
    .bins_per_octave(24)
    .build()?;
let cqt = Cqt::new(params);

let signal: Vec<f32> = load_audio();
let hop = 512;
let magnitudes = cqt.process(&signal, hop)?; // ndarray (frames, bins)
```

Frame `i` is centred on sample `i * hop`, and the signal is zero padded so
`1 + signal.len() / hop` frames are produced. A real sinusoid of amplitude `A`
at a bin's centre frequency reads as magnitude `A` in that bin.
`Cqt::process_complex` returns the complex coefficients instead. When many
signals are processed in a row, `Cqt::process_with` reuses a `CqtWorkspace`
so the decimation buffers are not reallocated per call.

### Real-time streaming

```rust
use cqt_rs::{Cqt, CqtParams, CqtStream};

let cqt = Cqt::new(CqtParams::new(44_100, 55.0, 7_040.0, 24)?);
let mut stream = CqtStream::new(&cqt, 512)?;

// In the capture callback, with blocks of any size:
stream.push(&cqt, block, |magnitudes| {
    // one call per completed frame, in order, no allocation
});

// At the end of the recording, to match `Cqt::process` exactly:
stream.flush(&cqt, |magnitudes| { /* trailing frames */ });
```

The stream emits frame `i` once sample `i * hop + latency` has arrived,
where `latency` is `cqt.latency_samples()`: half the longest analysis window
plus the decimation filter delay. Output is bit-identical to the batch path
regardless of how the audio is chunked.

### Parameters

| Builder method | Default | Effect |
| --- | --- | --- |
| `CqtParams::builder(sample_rate, min_freq, max_freq)` | | Sample rate in Hz and the centre frequencies of the lowest bin and the upper bound of the grid |
| `bins_per_octave(n)` | 12 | Frequency resolution; 24 or 36 lets small pitch shifts land on integer bin offsets |
| `filter_scale(s)` | 1.0 | Multiplies every window length; below one trades frequency resolution for time resolution |
| `gamma(hz)` | 0.0 | Adds a constant bandwidth (variable-Q transform); shortens low-frequency windows, cutting latency |
| `sparsity(f)` | 0.01 | Fraction of each kernel's spectral L1 mass that may be dropped |
| `max_kernel_length(n)` | none | Caps every window at `n` samples |
| `multirate(b)` | true | Analyse each octave at its own halved sample rate (see below) |

`build()` validates everything, including that the highest filter's pass-band
stays below the Nyquist frequency.

### Fingerprinting notes

- Use 24 to 36 bins per octave so a pitch shift of a few percent becomes an
  integer bin offset. The `pitch_shift_translates_bins` test shows that a
  shift of `k` bins moves every harmonic by exactly `k` bins.
- Choose the hop for the time resolution of your hashes; latency does not
  depend on it. Hops that are a multiple of `cqt.hop_alignment()` give every
  frame the same latency.
- `gamma` around 10–25 Hz lowers latency from hundreds of milliseconds to
  tens of milliseconds by shortening the sub-200 Hz windows, which rarely
  carry fingerprint peaks.
- Convert magnitudes with `magnitude_to_db` before peak picking so that
  clipping, compression and codec artefacts change peak levels by a couple of
  dB rather than by orders of magnitude (see `tests/robustness.rs`).

## How it works

Each bin `k` is a Hann-windowed complex exponential `a_k` of length
`Q · sr / f_k`. Following Brown & Puckette (1992), the inner product of a frame
with every `a_k` is evaluated in the frequency domain: one real FFT of the
frame, then a sparse product with the precomputed spectrum of each `a_k`. The
kernels are built in `f64`, pruned to the coefficients that carry 99 % of
their spectral L1 mass, and stored as contiguous runs.

With one FFT for the whole range, short high-frequency kernels spread over
thousands of FFT bins, so by default the transform is multi-rate (Schörkhuber
& Klapuri, 2010): the top octave is analysed at the full rate, the signal is
low-passed with an 80 dB Kaiser half-band filter and decimated by two, and
the next octave is analysed at half the rate with an FFT of the same small
size. The half-band filter runs as a polyphase block filter, so it
vectorizes, and decimation is zero-phase, which is what keeps the batch and
streaming outputs bit-identical. `multirate(false)` selects the single-FFT
engine, which the tests use as the exact reference; the multi-rate output
agrees with it to within 1 % of the frame's peak magnitude.

## Benchmarks

Measured with `cargo bench` on a 4-core container (Rust 1.98). Times are
criterion estimates. Per-frame numbers are single-threaded; batch numbers use
the rayon pool.

| Configuration | Bins | Levels | FFT lengths | Non-zeros | Build | Stream, per frame | Batch |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |
| Legacy grid (14.6–7902 Hz, 12/oct, 22 kHz, hop 1760) | 109 | 10 | 64, 128 × 9 | 1 323 | 0.67 ms | 9.3 µs | 4.06 ms, 1.74 ms with a reused workspace (30 s) |
| Same grid, single-rate | 109 | 1 | 32768 | 70 990 | 45.4 ms | 158 µs | 15.7 ms (30 s) |
| Same grid, kernels capped at 2048 | 109 | 9 | 8–128 | 1 038 | 0.53 ms | | 1.93 ms (30 s) |
| Fingerprint (55–7040 Hz, 24/oct, 44.1 kHz, hop 512) | 169 | 8 | 256, 512 × 7 | 1 891 | 2.0 ms | 10.1 µs | 2.94 ms (10 s) |

For comparison, version 0.1 on the same machine and legacy grid needed
1.50 ms to build its filterbank and 7.44 ms for the 30 s batch, while using
one 2048-sample window for every bin (so it was not constant-Q) and the
rayon pool for both. The 0.2 multi-rate engine processes the same audio
1.8× faster (4.3× with a reused workspace) with true constant-Q
resolution, and the streaming path handles a frame in 9.3 µs, which at hop
1760 is about 0.01 % of one core.

Regenerate the figures with:

```console
cargo run --release --example generate_plots
```

## Validation on real music

`scripts/fingerprint_demo.py` runs the transform (through
`examples/cqt_dump.rs`) on a 30 s excerpt of *Vibe Ace* by Kevin MacLeod
(CC BY 3.0, fetched from the librosa example-data repository), which serves
as the reference. Modified versions of the excerpt, the other half of the
same song, and an unrelated piece (*Dance of the Sugar Plum Fairy*, Kevin
MacLeod, CC BY 3.0) are cut into 10 s chunks, and every chunk is matched
against the reference on its own. Spectrograms use hop 256; fingerprints
are pitch- and tempo-invariant peak triplets (two bin differences plus a
quantized time ratio, as in Panako) looked up with ±1 bin and ±1 ratio step
of tolerance; the tempo of a match is the ratio of the two triplets' time
spans, the pitch shift and tempo are chosen by vote, and the position
follows from `t_ref = tempo · t_query + offset`.

Accuracy: after dividing out librosa's per-filter length scaling, the two
spectrograms agree to a mean 0.20 dB (0.13 dB on components above −40 dB)
with a correlation of 0.9996, and the linear gain ratio is 1.009.

<p align="center">
  <img src="./plots/song_cqt_vs_librosa.png" width="98%" />
</p>

Score = consistent hashes / hashes of the chunk. "Located" is the detected
start of the chunk in the reference, expected value in parentheses.

| Version | Chunk 1 | Chunk 2 | Chunk 3 | Mean | Shift applied / detected | Tempo applied / detected | Located at (expected) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| original | 95.7 % | 48.9 % | 53.8 % | 66.1 % | +0 / +0/+0/+0 | ×1.00 / 1.000/1.000/1.000 | 0.0 (0.0), 10.0 (10.0), 20.0 (20.0) s |
| pitch +2 semitones | 11.1 % | 9.8 % | 12.9 % | 11.3 % | +4 / +4/+4/+4 | ×1.00 / 1.000/1.000/1.000 | 0.0 (0.0), 10.0 (10.0), 20.0 (20.0) s |
| pitch −1 semitone | 16.4 % | 7.7 % | 7.6 % | 10.6 % | −2 / −2/−2/−2 | ×1.00 / 1.000/1.000/1.000 | 0.0 (0.0), 10.0 (10.0), 20.0 (20.0) s |
| tempo +12 % | 13.5 % | 9.6 % | 8.4 % | 10.5 % | +0 / +0/+0/+0 | ×1.12 / 1.115/1.118/1.099 | 0.0 (0.0), 11.2 (11.2), 22.5 (22.4) s |
| tempo −8 % | 13.7 % | 6.2 % | 11.9 % | 10.6 % | +0 / +0/+0/+0 | ×0.92 / 0.931/0.918/0.918 | −0.1 (0.0), 9.2 (9.2), 18.4 (18.4) s |
| speed +6 % (resampled) | 63.9 % | 51.3 % | 50.8 % | 55.3 % | +2 / +2/+2/+2 | ×1.06 / 1.060/1.059/1.059 | 0.0 (0.0), 10.6 (10.6), 21.2 (21.2) s |
| hard clip at 0.2 | 57.4 % | 20.2 % | 24.0 % | 33.9 % | +0 / +0/+0/+0 | ×1.00 / 1.000/1.000/1.000 | 0.0 (0.0), 10.0 (10.0), 20.0 (20.0) s |
| white noise, 10 dB SNR | 5.6 % | 14.5 % | 17.5 % | 12.5 % | +0 / +0/+0/+0 | ×1.00 / 1.000/1.000/1.000 | 0.0 (0.0), 10.0 (10.0), 20.0 (20.0) s |
| same song, other 30 s | 9.8 % | 7.4 % | 9.0 % | 8.7 % | +0 / +0/+0/+0 | ×1.00 / 1.000/1.000/1.000 | 19.4, 14.7, 21.0 s (repeated riffs) |
| control (unrelated piece) | 0.2 % | 0.2 % | 0.1 % | 0.2 % | +0 / — | ×1.00 / — | — |

Every chunk of every modified version is identified at 30× to 300× the
score of the unrelated piece, its pitch shift and tempo are recovered
exactly, and its position in the reference is found to within 0.1 s. The
chunks of the other half of the same song match at positions where the song
repeats its riffs, which is real shared content rather than a false
positive. Chunks 2 and 3 of the unmodified original score about 50 %
because triplets crossing a chunk edge are lost.

<p align="center">
  <img src="./plots/song_pitch_tempo_proof.png" width="98%" />
</p>
<p align="center">
  <img src="./plots/song_match_scores.png" width="80%" />
</p>

Engineering notes with the reasoning behind these choices and the next
steps for a fingerprinting layer are in [`docs/NOTES.md`](./docs/NOTES.md).

Reproduce with:

```console
pip install numpy scipy soundfile librosa matplotlib
python3 scripts/fingerprint_demo.py
```

## References

- J. C. Brown, "Calculation of a constant Q spectral transform", JASA 1991.
- J. C. Brown and M. S. Puckette, "An efficient algorithm for the calculation
  of a constant Q transform", JASA 1992.
- C. Schörkhuber and A. Klapuri, "Constant-Q transform toolbox for music
  processing", SMC 2010.
- C. Schörkhuber, A. Klapuri, N. Holighaus and M. Dörfler, "A Matlab toolbox
  for efficient perfect reconstruction time-frequency transforms with
  log-frequency resolution", AES 2014 (variable-Q).

## License

MIT
