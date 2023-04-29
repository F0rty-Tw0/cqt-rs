mod q_factor;
mod normalization;

use ndarray::{ Array1, Zip };
use rustfft::num_complex::Complex;

pub use q_factor::get_calculated_q_factor;
pub use normalization::calculate_norm;

use crate::common::CQTParams;

/// Creates a window function for the Constant Q Transform (CQT) filterbank.
///
/// The window function is a complex exponential multiplied by a Hann window and normalized.
/// Formula used: W(n) = exp(-j * 2π * center_freq * Q * n / sample_rate) * norm * hann_window(n)
///
/// # Arguments
///
/// * `center_freq` - The center frequency of the filter in the filterbank.
/// * `cqt_params` - CQTParams
///
/// # Returns
///
/// * `Array1<Complex<f32>>` An 1D Array containing the complex window function values.
pub fn create_complex_hann_window(
  center_freq: f32,
  cqt_params: &CQTParams
) -> Array1<Complex<f32>> {
  let q_factor = cqt_params.q_factor();
  let normalization = cqt_params.norm_factor();

  // Initialize an array of zeros for the complex window
  let mut complex_window = Array1::zeros(cqt_params.window_length);

  Zip::from(cqt_params.hann_window())
    .and(cqt_params.phase_factors())
    .and(complex_window.view_mut())
    .par_for_each(|hann_value, phase, complex_window_element| {
      // Calculate the complex exponential
      let complex_exp = Complex::new(0.0, phase * center_freq).exp();

      // Multiply the Hann window, complex exponentials
      *complex_window_element = complex_exp * q_factor * hann_value * normalization;
    });

  // Return the generated complex window
  complex_window
}

#[cfg(test)]
mod tests {
  use approx::assert_abs_diff_eq;

  use super::*;
  const MIN_FREQ: f32 = 20.0;
  const MAX_FREQ: f32 = 7902.1;
  const CENTER_FREQ: f32 = 440.0; // A4 in Hz
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 44100;
  const WINDOW_LENGTH: usize = 4096;
  const TOLERANCE: f32 = 1e-6;

  #[test]
  fn test_complex_hann_window_length() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let complex_window = create_complex_hann_window(CENTER_FREQ, &cqt_params);
    assert_eq!(complex_window.len(), WINDOW_LENGTH);
  }

  #[test]
  fn test_complex_hann_window_properties() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let complex_window = create_complex_hann_window(CENTER_FREQ, &cqt_params);
    assert_abs_diff_eq!(complex_window[0].norm(), 0.0, epsilon = TOLERANCE);
  }
}