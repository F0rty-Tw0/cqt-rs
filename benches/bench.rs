use criterion::{ criterion_main, criterion_group };

mod bench_base_freq_ratio;
mod bench_complex_hann_window;
mod bench_cqt_filterbank;
mod bench_cqt;
mod bench_normalization;
mod bench_phase_factors;
mod bench_q_factor;

criterion_group!(
  benches,
  bench_base_freq_ratio::bench_get_calculated_base_freq_ratio,
  bench_complex_hann_window::bench_create_complex_hann_window,
  bench_cqt_filterbank::bench_cqt_filterbank,
  bench_cqt::bench_cqt_process,
  bench_normalization::bench_calculate_norm,
  bench_phase_factors::bench_get_calculated_phase_factors,
  bench_q_factor::bench_get_calculated_q_factor
);

criterion_main!(benches);