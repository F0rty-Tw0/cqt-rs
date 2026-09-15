# Unreleased

## Fixed

- `Matcher`: the candidate position and tempo were vote-weighted means over
  a ±0.5 s cell neighbourhood. Beat-periodic self-matches of loop-based
  music bias that mean by 4 to 18 frames, more than the ±4 frame verifier
  tolerance, so a correct song verified at 0.00 alignment and never
  started. The candidate is now refined from the raw votes: the modal
  offset at frame resolution, then a tempo and offset refit on the votes
  within ±8 frames of the mode.
- `Tracker`: a position jump of the same song no longer ends the play by
  default (`--jump 0`). Loop-based tracks have several equally valid
  alignments, and the old behaviour split them into 2 to 10 s fragments:
  126 plays for 22 songs on a 90-minute DJ mix, now 36.
- `monitor`: the `stream` event's `seconds` field was truncated to two
  characters by a `{:.2}` applied to an already formatted string
  ("5436.51" printed as "54"). An empty stream wrote `inf` into the `done`
  event's `realtime_fraction`, which is not JSON; it is now `null`.

## Changed

- Defaults: the evidence window is 10 s instead of 5 s, and the report
  cadence 0.5 s instead of 0.25 s. A masked track in a DJ mix collects
  votes at half the usual rate, and the longer window lets it cross the
  threshold; `end` events follow the audio by up to 5 s more. The slower
  cadence halves the per-report candidate scan, which is the dominant CPU
  cost, and costs up to 0.25 s of detection latency.
- `start` events carry a `since` field: the original start time of the
  play.
- New options `--min-play S` (seconds a play must last before it is
  announced) and `--jump 0` (disable position-jump splitting).
- The per-report scan keeps an incremental slab histogram and a per-slab
  bound instead of rebuilding them, for the same results at about 20 %
  less CPU at dense vote rates.
- `PeakTrack::verify` searches only the aligned slice of the reference.

## Added

- `scripts/mix_eval.py` with `experiments/toucan2020.json` and
  `experiments/recognition-holdout.json`: a reproducible evaluation on a
  real 90-minute, 22-track DJ mix (Toucan Music 2020, CC BY-NC-SA) with 16
  negative tracks. With the new defaults on that corpus: 22 of 22 songs
  found in the full mix (21 of 22 before, t10 missed), 22 of 22 clean 10 s
  clips, 17 of 22 frozen 10 s mix clips (14 of 22 before), and no start on
  the 16 negatives, both as 10 s clips and as about 60 minutes of complete
  tracks. The radio evaluation is unchanged: 16 of 16 plays, 9 of 9 in the
  DJ set, no false alarm on the null streams, 7 on the hard-negative
  loops.

## Notes

- `--time-radius 12 --bin-radius 5` (denser peaks) reaches 18 of 22 on the
  frozen clips and 22 of 22 on the mix with a 5 s window, but costs about
  five times the CPU, doubles the index, and loses one key-locked
  time-stretch play in the radio DJ set. It is left as an option rather
  than a default.

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
  `scripts/radio_negatives.py` builds a second null stream and hard
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
