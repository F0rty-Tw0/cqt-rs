use criterion::{ black_box, Criterion };
use hann_rs::get_hann_window;
use cqt_rs::calculate_norm;

pub fn bench_calculate_norm(criterion: &mut Criterion) {
  const WINDOW_LENGTH: usize = 2000;

  let hann_window = get_hann_window(WINDOW_LENGTH).expect(
    "Failed to get the Hann window from the lookup table"
  );

  criterion.bench_function("calculate_norm", |bencher| {
    bencher.iter(|| black_box(calculate_norm(&hann_window)));
  });
}