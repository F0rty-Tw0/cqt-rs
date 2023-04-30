use crate::{ create_complex_hann_window, CQTParams };
use ndarray::{ Array2, Axis, parallel::prelude::* };
use rustfft::{ FftPlanner, num_complex::Complex };
use std::{ error::Error, fmt };

// Defining your custom error type
#[derive(Debug)]
pub enum CQTFilterbankError {
  InvalidParams,
  FFTError,
}

// Implement the Error trait for the custom error type
impl Error for CQTFilterbankError {}

// Implement the Display trait for the custom error type
impl fmt::Display for CQTFilterbankError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      CQTFilterbankError::InvalidParams => { write!(f, "Invalid parameters for CQT filterbank") }
      CQTFilterbankError::FFTError => { write!(f, "FFT error in CQT filterbank computation") }
    }
  }
}

/// Computes a filterbank for the Constant-Q Transform (CQT) with the given parameters.
///
/// # Arguments
///
/// * `cqt_params` - CQTParams
///
/// # Returns
///
/// A 2D array of `Complex<f32>` values representing the filterbank.
/// The first dimension corresponds to the filterbank bins, and the second dimension
/// corresponds to the window samples.
///
/// # Errors
///
/// Returns a `CQTFilterbankError` if there was an error while creating the CQT filterbank.
pub fn compute_cqt_filterbank(
  cqt_params: &CQTParams
) -> Result<Array2<Complex<f32>>, CQTFilterbankError> {
  // Initialize a 2d Array to store the filterbank
  let mut filterbank = Array2::zeros((cqt_params.num_bins(), cqt_params.window_length));

  // Initialize the FFT object
  let fft = FftPlanner::new().plan_fft_forward(cqt_params.window_length);

  filterbank
    .axis_iter_mut(Axis(0))
    .into_par_iter()
    .enumerate()
    .for_each(|(bin, mut window)| {
      // Compute the center frequency for this bin
      let center_freq = cqt_params.center_freq(bin);

      // Create a complex Hann window for this bin
      let mut complex_hann_window = create_complex_hann_window(center_freq, &cqt_params);

      // Apply the FFT to the complex Hann window
      fft.process(
        complex_hann_window
          .as_slice_mut()
          .expect("Error while getting slice of complex_hann_window")
      );

      // Assign the FFT result to the current window of the filterbank
      window.assign(&complex_hann_window);
    });

  Ok(filterbank)
}

#[cfg(test)]
mod tests {
  use crate::{ CQTParams, compute_cqt_filterbank };

  const MIN_FREQ: f32 = 20.0;
  const MAX_FREQ: f32 = 7902.1;
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 44100;
  const WINDOW_LENGTH: usize = 4096;

  #[test]
  fn test_compute_cqt_filterbank_ok() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let filterbank = compute_cqt_filterbank(&cqt_params);
    assert!(filterbank.is_ok());
  }

  #[test]
  fn test_compute_cqt_filterbank_dimensions() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();

    let filterbank = compute_cqt_filterbank(&cqt_params).unwrap();
    let num_bins = ((BINS_PER_OCTAVE as f32) * (MAX_FREQ / MIN_FREQ).log2().ceil()) as usize;
    assert_eq!(filterbank.dim(), (num_bins, WINDOW_LENGTH));
  }
}