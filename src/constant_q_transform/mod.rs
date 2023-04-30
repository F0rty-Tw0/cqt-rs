mod input_signal;
mod cqt_signal_error_enum;

use ndarray::{
  parallel::prelude::{ IntoParallelIterator, IndexedParallelIterator, ParallelIterator },
  Array2,
  Axis,
  Zip,
  s,
};
use rustfft::{ num_complex::{ Complex, ComplexFloat }, FftPlanner };

use crate::{ CQTParams, compute_cqt_filterbank };
use input_signal::pad_input_signal;

pub use cqt_signal_error_enum::SignalError;

/// The Cqt struct is an implementation of the Constant Q Transform (CQT)
/// for time-frequency analysis of a signal. The struct provides methods to
/// initialize the CQT parameters and compute the CQT of a given input signal.
pub struct Cqt {
  cqt_params: CQTParams,
  pub filterbank: Array2<Complex<f32>>,
}

impl Cqt {
  /// Constructs a new `Cqt` instance with the given parameters.
  ///
  /// # Arguments
  ///
  /// * `cqt_params` - CQTParams
  ///
  /// # Returns
  ///
  /// A new `Cqt` instance with the specified parameters.
  pub fn new(cqt_params: CQTParams) -> Self {
    // Compute the CQT filterbank using the CQTParams instance
    let filterbank = compute_cqt_filterbank(&cqt_params).expect("Error computing CQT filterbank");

    // Return a new Cqt instance with the given parameters and filterbank
    Cqt {
      cqt_params,
      filterbank,
    }
  }

  /// Process the input signal and compute the Constant-Q Transform (CQT) features.
  ///
  /// # Arguments
  ///
  /// * `input_signal` - An Array1<f32> of the input audio signal
  /// * `hop_size` - The number of samples to hop between frames
  ///
  /// # Returns
  ///
  /// * `Result<Array2<f32>, SignalError>` - The CQT feature matrix
  pub fn process(&self, signal: &[f32], hop_size: usize) -> Result<Array2<f32>, SignalError> {
    let signal_len = signal.len();

    if signal_len == 0 {
      return Err(SignalError::EmptyInputSignal);
    }

    if hop_size == 0 || hop_size > self.cqt_params.window_length {
      return Err(SignalError::InvalidHopSize);
    }

    let num_frames = signal_len / hop_size;

    let window_len = self.cqt_params.window_length;
    let hann_window = &self.cqt_params.hann_window;
    let transposed_filterbank = self.filterbank.t();

    // Assign the input signal to the center of the padded signal
    let signal_padded = pad_input_signal(signal, window_len, hop_size).expect(
      "Error padding input signal"
    );

    // Initialize the matrix to store the FFT output for each frame
    let mut cqt_output = Array2::<Complex<f32>>::zeros((num_frames, window_len));
    let fft = FftPlanner::<f32>::new().plan_fft_forward(window_len);

    // Compute the CQT for each frame
    cqt_output
      .axis_iter_mut(Axis(0))
      .into_par_iter()
      .enumerate()
      .for_each(|(frame_idx, mut fft_output_row)| {
        let start = frame_idx * hop_size;
        let end = start + window_len;

        // Get the frame from the padded signal
        let frame = signal_padded.slice(s![start..end]);

        // Perform element-wise multiplication of the frame with the Hann window,
        // and store the result in the fft_output_row
        Zip::from(&mut fft_output_row)
          .and(frame)
          .and(hann_window)
          .par_for_each(|row_elem, &frame_elem, &window_elem| {
            row_elem.re = frame_elem * window_elem;
          });

        // Perform FFT
        fft.process(fft_output_row.as_slice_mut().expect("Error applying fft to frame"));
      });

    // Apply the CQT filterbank to the FFT output matrix
    let cqt_filtered = cqt_output.dot(&transposed_filterbank);

    // Compute the element-wise absolute value of the filtered CQT matrix NOTE: check if needed to be done later
    let abs_cqt_filtered = cqt_filtered.mapv(|x| x.abs());

    // Just in case tested the parallel version and it's slower
    // let mut abs_cqt_filtered = Array2::<f32>::zeros(cqt_filtered.dim());
    // par_azip!((abs_cqt_filtered_row in &mut abs_cqt_filtered, cqt_filtered_row in &cqt_filtered) {
    //   *abs_cqt_filtered_row = cqt_filtered_row.abs();
    // });

    Ok(abs_cqt_filtered)
  }
}

#[cfg(test)]
mod tests {
  use approx::assert_abs_diff_eq;

  use crate::create_dummy_audio_signal;

  use super::*;

  const MIN_FREQ: f32 = 20.0;
  const MAX_FREQ: f32 = 7902.1;
  const BINS_PER_OCTAVE: usize = 12;
  const SAMPLE_RATE: usize = 44100;
  const WINDOW_LENGTH: usize = 4096;

  #[test]
  fn test_new_cqt() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let cqt = Cqt::new(cqt_params);

    assert_eq!(cqt.filterbank.dim(), (108, 4096));
  }

  #[test]
  fn test_process_valid_signal() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let cqt = Cqt::new(cqt_params);

    let signal = vec![0.0; 1024];
    let hop_size = 512;
    let result = cqt.process(&signal, hop_size);
    assert!(result.is_ok());
    let cqt_features = result.unwrap();
    assert_eq!(cqt_features.dim(), (2, 108));
  }

  #[test]
  fn test_process_sine_wave() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let cqt = Cqt::new(cqt_params);
    let freq = 440.0;
    let hop_size = 512;
    let signal = create_dummy_audio_signal(SAMPLE_RATE, freq, 1.0);
    let result = cqt.process(&signal, hop_size).unwrap();

    let bin_index = ((freq / MIN_FREQ).log2() * (BINS_PER_OCTAVE as f32)).round() as usize;
    let max_value = result.column(bin_index).iter().cloned().fold(f32::MIN, f32::max);

    assert_abs_diff_eq!(max_value, 19386750.0, epsilon = 1e-2);
  }

  #[test]
  fn test_process_empty_signal() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let cqt = Cqt::new(cqt_params);

    let signal = vec![];
    let hop_size = 512;
    let result = cqt.process(&signal, hop_size);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SignalError::EmptyInputSignal);
  }

  #[test]
  fn test_process_invalid_hop_size() {
    let cqt_params = CQTParams::new(
      MIN_FREQ,
      MAX_FREQ,
      BINS_PER_OCTAVE,
      SAMPLE_RATE,
      WINDOW_LENGTH
    ).unwrap();
    let cqt = Cqt::new(cqt_params);

    let signal = vec![0.0; 128];
    let hop_size = 0;
    let result = cqt.process(&signal, hop_size);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SignalError::InvalidHopSize);
  }
}