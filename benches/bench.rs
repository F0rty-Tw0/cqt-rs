use criterion::{ criterion_main, criterion_group };

mod bench_normalization;
mod bench_base_freq_ratio;
mod bench_phase_factors;
mod bench_q_factor;
mod bench_complex_hann_window;

criterion_group!(
  benches,
  bench_normalization::bench_calculate_norm,
  bench_base_freq_ratio::bench_get_calculated_base_freq_ratio,
  bench_phase_factors::bench_get_calculated_phase_factors,
  bench_q_factor::bench_get_calculated_q_factor,
  bench_complex_hann_window::bench_create_complex_hann_window
);

criterion_main!(benches);