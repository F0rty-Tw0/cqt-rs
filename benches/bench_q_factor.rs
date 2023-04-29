use criterion::{ black_box, Criterion };
use cqt_rs::get_calculated_q_factor;

pub fn bench_get_calculated_q_factor(criterion: &mut Criterion) {
  const BINS_PER_OCTAVE: usize = 13;

  criterion.bench_function("get_calculated_q_factor", |bencher| {
    bencher.iter(|| black_box(get_calculated_q_factor(BINS_PER_OCTAVE)));
  });
}