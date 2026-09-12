# 0.2.0 - 2026-09-12

Breaking rewrite of the transform.

## Fixed

- Every bin now has its own analysis window with length `Q * sample_rate /
  centre_frequency`. Version 0.1 applied the same full-length Hann window to
  every bin, which made the frequency resolution constant rather than
  proportional to frequency: low bins overlapped heavily and high bins had
  poor time resolution. Pitch shifts now translate the output along the bin
  axis, which is the property audio fingerprinting relies on.
- The old spectral kernel was multiplied without conjugation and without the
  `1 / N` Parseval scaling. Magnitudes are now normalized so that a real
  sinusoid of amplitude `A` at a bin centre produces `A`.
- Bins above `max_freq` (and potentially above Nyquist) are no longer
  produced; the pass-band of the top filter is checked against Nyquist.
- Frames are centred on `i * hop_size` (librosa `center=True` semantics),
  giving `1 + len / hop_size` frames instead of silently dropping the tail.
- The published 0.1 build depended on `hann-rs 0.1.0`, whose package ships a
  lowercase `cargo.toml` and cannot be built on case-sensitive file systems.

- Kernel pruning (`sparsity`) no longer discards coefficients tied with
  the last one inside the budget: a run of equal magnitudes is dropped
  whole or kept whole, so the discarded mass never exceeds the limit.
- Configurations whose decimation filter would exceed the tap limit are
  rejected from the tap count alone, before the filter is designed.
- `CqtStream::flush` padding is part of the frame timeline once more audio
  is pushed: continuing after a flush now produces exactly the frames of
  the audio with that silence inserted, and flushing twice emits nothing
  new (`CqtStream::padding_samples`).
- `CqtStream` is bound to its transform: `push`/`flush` panic on a
  transform with different parameters and `reset` re-binds the stream;
  `Cqt::process_with` rebuilds a `CqtWorkspace` allocated by a different
  transform instead of reading it with the wrong layout.

## Added

- `cqt-monitor` workspace crate (`monitor/`): streaming peak picker,
  pitch- and tempo-invariant triplet hasher, watch-list hash index,
  sliding-window matcher with a calibrated confidence score, a per-song
  detection `Tracker`, and a `monitor` binary that reports detections on
  a stream as JSON lines. Validated on a simulated radio programme
  (`scripts/radio_sim.py`, `scripts/radio_eval.py`). The hasher releases
  an anchor as soon as its candidate list is determined, which halves
  the fingerprint delay in normal music; the index caps repetitive keys
  per song; the matcher searches every occupied cell and removes expired
  votes exactly.
- `cqt-monitor`: a `PeakTrack` per watched song aligns the most recent
  stream peaks with the song under the matcher's hypothesis; the
  `Tracker` takes `Scored` candidates and starts a play only when the
  alignment confirms the votes, and holds a play through a quiet
  passage while the peaks still align. Fan-out 4 with `half` 40 is the
  default (same evidence margin as fan-out 6, half the index and CPU).
  `Fingerprinter` (dB → peaks → hashes) is a library type, with an
  end-to-end detection test on synthetic audio.
  `scripts/radio_negatives.py` builds a held-out null stream and hard
  negatives (same artist, reversed, out-of-range speed, loops) that
  `scripts/radio_eval.py --negatives` scores per family;
  `scripts/radio_dj.py` a continuous DJ set with BPM change, key change
  and talk-over at once (`radio_eval.py --programme stream_dj`), and
  the evaluation draws one confidence/alignment panel per play.
  `monitor --stream -` reads live PCM from stdin; `--jump` sets the
  position jump that counts as a new play (10 s).
- CI runs separate gates for the library (`cargo publish --dry-run`,
  CHANGELOG heading check) and the monitor crate.
- `CqtStream::params`, `CqtStream::padding_samples`,
  `CqtWorkspace::params`.
- Multi-rate processing (Schörkhuber & Klapuri, 2010): each octave is
  analysed at its own sample rate, halved octave by octave with an 80 dB
  Kaiser half-band filter, so every octave uses a small FFT and a compact
  kernel. It is the default; `multirate(false)` selects the single-FFT
  engine.
- `CqtParams::builder` with `bins_per_octave`, `filter_scale`, `gamma`
  (variable-Q transform), `sparsity`, `max_kernel_length` and `multirate`.
- `CqtStream`: push-based real-time processing with no per-frame allocation,
  a fixed latency of `fft_length / 2` samples and output identical to the
  batch path.
- `Cqt::process_complex` for phase-aware consumers, and `Cqt::process_with`
  / `process_complex_with` with a reusable `CqtWorkspace` for repeated batch
  calls.
- `Cqt::frequencies`, `kernel_lengths`, `fft_lengths`, `frame_spans`,
  `latency_samples`, `hop_alignment`, `num_levels`, `num_nonzeros` and
  `kernels` describe the built transform.
- `magnitude_to_db` helper.
- Cargo feature `parallel` (default) enabling rayon for the batch path.
- Integration tests for the transform definition, pitch-shift translation,
  tempo scaling, clipping and additive noise, and stream/batch parity.
- Reproducible SVG figures via `cargo run --example generate_plots`.
- `examples/cqt_dump.rs` and `scripts/fingerprint_demo.py`: validation on a
  CC BY song against librosa's CQT (0.2 dB mean difference) and a
  pitch/tempo-invariant fingerprint match of pitch-shifted, time-stretched,
  resampled, clipped and noisy versions.

## Changed

- Frames are transformed with a real FFT (`realfft`) and a sparse spectral
  kernel built in `f64`; the dense complex matrix product is gone.
- FFT plans and scratch buffers are created once instead of per call.
- Minimum supported Rust version is 1.98; the crate uses Edition 2024 and
  forbids `unsafe`.
- Dependencies: `hann-rs 0.2`, `rustfft 6.4`, `realfft 3.5`, `ndarray 0.17`,
  `thiserror 2`, `criterion 0.8`.

## Removed

- `lazy_static` and the global lookup tables for Q factors, base frequency
  ratios, phase factors and windows.
- `compute_cqt_filterbank`, `create_complex_hann_window`, `calculate_norm`,
  `get_calculated_q_factor`, `get_calculated_phase_factors`,
  `get_calculated_base_freq_ratio` and `create_dummy_audio_signal`.
- The `window_length` parameter; FFT lengths now follow from the kernel
  lengths (use `max_kernel_length` to cap them).
- The upper bound on `hop_size`; only zero is rejected.

## Migration

| 0.1 API | 0.2 replacement |
| --- | --- |
| `CQTParams::new(min, max, bins, sr, window_len)` | `CqtParams::builder(sr, min, max).bins_per_octave(bins).max_kernel_length(window_len).build()` |
| `CQTParamsError` | `CqtParamsError` (new variants) |
| `Cqt::new(params)` | unchanged |
| `cqt.process(&signal, hop)` | unchanged signature; frames are now `1 + len / hop` and magnitudes normalized |
| `SignalError` | `CqtError` |
| `cqt.filterbank` | `cqt.kernel()` |
| `params.center_freq(bin)` | unchanged |
| `params.q_factor()` | unchanged |
| `params.hann_window()`, `params.phase_factors()`, `params.norm_factor()` | removed |

# 0.1.0

* Initial release.
