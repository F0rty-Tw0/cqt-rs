use criterion::{ black_box, Criterion };
use cqt_rs::get_calculated_phase_factors;

pub fn bench_get_calculated_phase_factors(criterion: &mut Criterion) {
  const WINDOW_LENGTH: usize = 2000;
  const SAMPLE_RATE: usize = 22000;

  criterion.bench_function("get_calculated_phase_factors", |bencher| {
    bencher.iter(|| black_box(get_calculated_phase_factors(WINDOW_LENGTH, SAMPLE_RATE)));
  });
}