use criterion::{ black_box, Criterion };
use cqt_rs::get_calculated_base_freq_ratio;

pub fn bench_get_calculated_base_freq_ratio(criterion: &mut Criterion) {
  const BINS_PER_OCTAVE: usize = 12;

  criterion.bench_function("get_calculated_base_freq_ratio", |bencher| {
    bencher.iter(|| black_box(get_calculated_base_freq_ratio(BINS_PER_OCTAVE)));
  });
}