use std::{ error::Error, fmt };

use hann_rs::get_hann_window;
use ndarray::Array1;

use crate::{
  complex_hann_window::{ calculate_norm, get_calculated_q_factor },
  calculations::{ get_calculated_base_freq_ratio, get_calculated_phase_factors },
};

/// Error type for the CQTParams.
#[derive(Debug, PartialEq)]
pub enum CQTParamsError {
  InvalidMinFrequency,
  InvalidMaxFrequency,
  InvalidBinsPerOctave,
  InvalidSampleRate,
  InvalidWindowLength,
}

// Implement the Error trait for the CQTParamsError
impl Error for CQTParamsError {}

// Implement the Display trait for the CQTParamsError enum
impl fmt::Display for CQTParamsError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Write the error message to the Formatter
    match self {
      CQTParamsError::InvalidMinFrequency => {
        write!(f, "Invalid minimum frequency: must be a positive number")
      }
      CQTParamsError::InvalidMaxFrequency => {
        write!(
          f,
          "Invalid maximum frequency: must be a positive number and greater than the minimum frequency"
        )
      }
      CQTParamsError::InvalidBinsPerOctave => {
        write!(f, "Invalid bins per octave: must be a positive integer")
      }
      CQTParamsError::InvalidSampleRate => {
        write!(f, "Invalid sample rate: must be a positive integer")
      }
      CQTParamsError::InvalidWindowLength => {
        write!(f, "Invalid window length: must be a positive integer")
      }
    }
  }
}

/// `CQTParams` is a struct that holds the parameters needed for the
/// Constant-Q Transform (CQT) filter bank.
#[derive(Debug, PartialEq)]
pub struct CQTParams {
  pub min_freq: f32,
  pub max_freq: f32,
  pub bins_per_octave: usize,
  pub sample_rate: usize,
  pub window_length: usize,
  pub hann_window: Vec<f32>,
  num_bins: usize,
  q_factor: f32,
  base_freq_ratio: f32,
  norm_factor: f32,
  phase_factors: Array1<f32>,
}

impl CQTParams {
  /// Create a new CQTParams instance with the provided parameters.
  ///
  /// # Arguments
  ///
  /// * `min_freq` - The minimum frequency in Hz.
  /// * `max_freq` - The maximum frequency in Hz.
  /// * `bins_per_octave` - The number of frequency bins per octave.
  /// * `sample_rate` - The audio sample rate in Hz.
  /// * `window_length` - The length of the analysis window.
  ///
  /// # Errors
  ///
  /// Returns an error if any of the input parameters are not positive integers.
  pub fn new(
    min_freq: f32,
    max_freq: f32,
    bins_per_octave: usize,
    sample_rate: usize,
    window_length: usize
  ) -> Result<Self, CQTParamsError> {
    if min_freq <= 0.0 {
      return Err(CQTParamsError::InvalidMinFrequency);
    }

    if max_freq <= min_freq {
      return Err(CQTParamsError::InvalidMaxFrequency);
    }

    if bins_per_octave == 0 {
      return Err(CQTParamsError::InvalidBinsPerOctave);
    }

    if sample_rate == 0 {
      return Err(CQTParamsError::InvalidSampleRate);
    }

    if window_length == 0 {
      return Err(CQTParamsError::InvalidWindowLength);
    }
    // Computes the smallest power of two greater than or equal to window_length
    // When the input length is not a power of two, the algorithm's performance may degrade.
    let window_length = window_length.next_power_of_two();
    // Compute the number of bins K = B * log2(f_max / f_min):
    let num_bins = ((bins_per_octave as f32) * (max_freq / min_freq).log2().ceil()) as usize;
    // Compute the base frequency ratio
    let base_freq_ratio = get_calculated_base_freq_ratio(bins_per_octave);
    // Compute the Q factor
    let q_factor = get_calculated_q_factor(bins_per_octave).unwrap();
    // Compute the Hann window
    let hann_window = get_hann_window(window_length).unwrap();
    // Compute the normalization factor
    let norm_factor = calculate_norm(&hann_window).unwrap();
    // Compute phase factors
    let phase_factors = get_calculated_phase_factors(window_length, sample_rate);

    Ok(Self {
      min_freq,
      max_freq,
      bins_per_octave,
      sample_rate,
      window_length,
      num_bins,
      q_factor,
      base_freq_ratio,
      hann_window,
      norm_factor,
      phase_factors,
    })
  }

  /// Return the number of bins in the filter bank.
  pub fn num_bins(&self) -> usize {
    self.num_bins
  }

  /// Return the calculated Q facto.
  pub fn q_factor(&self) -> f32 {
    self.q_factor
  }

  /// Return the normalization factor.
  pub fn norm_factor(&self) -> f32 {
    self.norm_factor
  }

  /// Calculate the center frequency for a given bin. f_c = f_min * r^n
  pub fn center_freq(&self, bin: usize) -> f32 {
    self.min_freq * self.base_freq_ratio.powf(bin as f32)
  }

  /// Return a reference to the phase factors array.
  pub fn phase_factors(&self) -> &Array1<f32> {
    &self.phase_factors
  }

  /// Return a reference to the Hann window array.
  pub fn hann_window(&self) -> &Vec<f32> {
    &self.hann_window
  }
}

#[cfg(test)]
mod tests {
  use std::f32::consts::PI;

  use hann_rs::get_hann_window;

  use crate::complex_hann_window::{ get_calculated_q_factor, calculate_norm };

  use super::*;

  const MIN_FREQ: f32 = 20.0;
  const MAX_FREQ: f32 = 7902.1;
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 44100;
  const WINDOW_LENGTH: usize = 4096;

  #[test]
  fn test_cqt_params_ok() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    );

    assert!(cqt_params.is_ok());
  }

  #[test]
  fn test_cqt_params_q_factor() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let expected_q_factor = get_calculated_q_factor(BINS_PER_OCTAVE).unwrap();

    assert_eq!(cqt_params.q_factor(), expected_q_factor);
  }

  #[test]
  fn test_cqt_params_num_bins() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let expected_num_bins = ((BINS_PER_OCTAVE as f32) *
      (MAX_FREQ / MIN_FREQ).log2().ceil()) as usize;

    assert_eq!(cqt_params.num_bins(), expected_num_bins);
  }

  #[test]
  fn test_cqt_params_center_freq() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let expected_center_freq = MIN_FREQ * (2f32).powf(1.0 / (BINS_PER_OCTAVE as f32)).powf(40.0);

    assert_eq!(cqt_params.center_freq(0), MIN_FREQ);
    assert_eq!(cqt_params.center_freq(40), expected_center_freq);
  }

  #[test]
  fn test_cqt_params_phase_factors() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    assert_eq!(cqt_params.phase_factors().len(), WINDOW_LENGTH);
    assert_eq!(cqt_params.phase_factors()[0], 0.0);
    assert_eq!(
      cqt_params.phase_factors()[WINDOW_LENGTH - 1],
      (-2.0 * PI * ((WINDOW_LENGTH - 1) as f32)) / (SAMPLE_RATE as f32)
    );
  }

  #[test]
  fn test_cqt_params_hann_window() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let hann_window = get_hann_window(WINDOW_LENGTH).unwrap();

    assert_eq!(cqt_params.hann_window().len(), WINDOW_LENGTH);
    assert_eq!(cqt_params.hann_window()[0], hann_window[0]);
    assert_eq!(cqt_params.hann_window()[WINDOW_LENGTH - 1], hann_window[WINDOW_LENGTH - 1]);
  }

  #[test]
  fn test_cqt_params_norm_factor() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let hann_window = get_hann_window(WINDOW_LENGTH).unwrap();
    let expected_norm_factor = calculate_norm(&hann_window).unwrap();

    assert_eq!(cqt_params.norm_factor(), expected_norm_factor);
  }
  #[test]
  fn test_cqt_params_invalid_min_frequency() {
    let cqt_params = CQTParams::new(-10.0, MAX_FREQ, BINS_PER_OCTAVE, SAMPLE_RATE, WINDOW_LENGTH);

    assert!(cqt_params.is_err());
    assert_eq!(cqt_params, Err(CQTParamsError::InvalidMinFrequency));
  }

  #[test]
  fn test_cqt_params_invalid_max_frequency() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MIN_FREQ - 1.0,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    );

    assert!(cqt_params.is_err());
    assert_eq!(cqt_params, Err(CQTParamsError::InvalidMaxFrequency));
  }

  #[test]
  fn test_cqt_params_invalid_bins_per_octave() {
    let cqt_params = CQTParams::new(MIN_FREQ, MAX_FREQ, 0, SAMPLE_RATE, WINDOW_LENGTH);

    assert!(cqt_params.is_err());
    assert_eq!(cqt_params, Err(CQTParamsError::InvalidBinsPerOctave));
  }

  #[test]
  fn test_cqt_params_invalid_sample_rate() {
    let cqt_params = CQTParams::new(MIN_FREQ, MAX_FREQ, BINS_PER_OCTAVE, 0, WINDOW_LENGTH);

    assert!(cqt_params.is_err());
    assert_eq!(cqt_params, Err(CQTParamsError::InvalidSampleRate));
  }

  #[test]
  fn test_cqt_params_invalid_window_length() {
    let cqt_params = CQTParams::new(MIN_FREQ, MAX_FREQ, BINS_PER_OCTAVE, SAMPLE_RATE, 0);

    assert!(cqt_params.is_err());
    assert_eq!(cqt_params, Err(CQTParamsError::InvalidWindowLength));
  }
}