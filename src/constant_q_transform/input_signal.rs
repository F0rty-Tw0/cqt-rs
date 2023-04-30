use ndarray::{ Array1, s };

use super::SignalError;

/// Pads an input signal symmetrically to prepare it for the CQT computation.
///
/// # Arguments
///
/// * `signal` - The input signal as a slice of `f32` values.
/// * `window_len` - The length of the window used in the CQT computation.
/// * `hop_size` - The number of samples between successive CQT frames.
///
/// # Returns
///
/// `Result<Array1<f32>, SignalError> ` containing the padded input signal.
pub fn pad_input_signal(
  signal: &[f32],
  window_len: usize,
  hop_size: usize
) -> Result<Array1<f32>, SignalError> {
  if hop_size == 0 || hop_size > window_len {
    return Err(SignalError::InvalidHopSize);
  }
  let signal_len = signal.len();

  if signal_len == 0 {
    return Err(SignalError::EmptyInputSignal);
  }

  // Calculate the total amount of padding needed
  let signal_padding = window_len - hop_size;
  // Calculate the amount of padding for each side of the signal
  let half_signal_padding = signal_padding / 2;

  let signal_array = Array1::from(signal.to_vec());
  let mut signal_padded = Array1::<f32>::zeros(signal_padding + signal_len);

  // Assign the input signal to the center of the padded signal
  signal_padded
    .slice_mut(s![half_signal_padding..half_signal_padding + signal_len])
    .assign(&signal_array);

  Ok(signal_padded)
}

#[cfg(test)]
mod tests {
  use super::*;

  const SIGNAL: [f32; 4] = [1.0, 2.0, 3.0, 4.0];
  const WINDOW_LENGTH: usize = 4;

  #[test]
  fn test_pad_input_signal_valid() {
    let hop_size = 2;
    let expected = Array1::from(vec![0.0, 1.0, 2.0, 3.0, 4.0, 0.0]);
    let result = pad_input_signal(&SIGNAL, WINDOW_LENGTH, hop_size).unwrap();
    assert_eq!(result, expected);
  }

  #[test]
  fn test_pad_input_signal_empty_signal() {
    let signal: Vec<f32> = vec![];
    let hop_size = 2;
    let result = pad_input_signal(&signal, WINDOW_LENGTH, hop_size);

    assert!(result.is_err());
    assert_eq!(result, Err(SignalError::EmptyInputSignal));
  }

  #[test]
  fn test_pad_input_signal_invalid_hop_size_zero() {
    let hop_size = 0;
    let result = pad_input_signal(&SIGNAL, WINDOW_LENGTH, hop_size);
    assert!(result.is_err());
    assert_eq!(result, Err(SignalError::InvalidHopSize));
  }

  #[test]
  fn test_pad_input_signal_invalid_hop_size_greater_than_window_len() {
    let hop_size = 5;
    let result = pad_input_signal(&SIGNAL, WINDOW_LENGTH, hop_size);
    assert!(result.is_err());
    assert_eq!(result, Err(SignalError::InvalidHopSize));
  }
}