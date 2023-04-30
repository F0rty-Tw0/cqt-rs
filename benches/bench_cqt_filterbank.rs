use criterion::{ black_box, Criterion };
use cqt_rs::{ compute_cqt_filterbank, CQTParams };

pub fn bench_cqt_filterbank(criterion: &mut Criterion) {
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

  criterion.bench_function("compute_cqt_filterbank", |bencher| {
    bencher.iter(|| { black_box(compute_cqt_filterbank(&cqt_params)) })
  });
}