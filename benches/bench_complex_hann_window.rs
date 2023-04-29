use criterion::{ black_box, Criterion };
use cqt_rs::{ create_complex_hann_window, CQTParams };

pub fn bench_create_complex_hann_window(criterion: &mut Criterion) {
  const MIN_FREQ: f32 = 14.568; // A#/Bb-1
  const MAX_FREQ: f32 = 7902.1; // B8
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 44000;
  const WINDOW_LENGTH: usize = 4000;

  let cqt_params = CQTParams::new(
    MIN_FREQ,
    MAX_FREQ,
    BINS_PER_OCTAVE,
    SAMPLE_RATE,
    WINDOW_LENGTH
  ).unwrap();

  let center_freq = cqt_params.center_freq(cqt_params.num_bins());

  criterion.bench_function("create_complex_hann_window", |bencher| {
    bencher.iter(|| { black_box(create_complex_hann_window(center_freq, &cqt_params)) })
  });
}