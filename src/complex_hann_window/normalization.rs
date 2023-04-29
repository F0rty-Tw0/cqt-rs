use std::{ error::Error, fmt };
use hann_rs::get_hann_window_sum_squares;

#[derive(Debug, PartialEq)]
pub enum NormalizationError {
  InvalidWindowLength,
}

impl Error for NormalizationError {}

impl fmt::Display for NormalizationError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      NormalizationError::InvalidWindowLength => {
        write!(f, "Invalid window length: must be greater than zero")
      }
    }
  }
}

/// Calculates the normalization factor for the Hann window.
///
/// # Arguments
///
/// * `hann_window` - A reference to the Hann window Array1.
///
/// # Returns
///
/// * Result<f32, NormalizationError> - The calculated normalization factor.
pub fn calculate_norm(hann_window: &Vec<f32>) -> Result<f32, NormalizationError> {
  if hann_window.len() == 0 {
    return Err(NormalizationError::InvalidWindowLength);
  }

  // Calculate the sum of squares of the Hann window elements
  let sum_of_squares = get_hann_window_sum_squares(hann_window);

  // Calculate and return the normalization factor as the square root
  // of the sum of squares divided by the window length
  Ok((sum_of_squares / (hann_window.len() as f32)).sqrt())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_calculate_norm() {
    let hann_window = vec![0.5, 0.5];
    assert_eq!(calculate_norm(&hann_window).unwrap(), 0.5);

    let hann_window = vec![0.5, 0.5, 0.5, 0.5];
    assert_eq!(calculate_norm(&hann_window).unwrap(), 0.5);

    let hann_window = vec![0.25, 0.5, 0.25];
    assert_eq!(calculate_norm(&hann_window).unwrap(), ((0.375 / 3.0) as f32).sqrt());

    let hann_window = vec![0.0, 1.0, 0.0];
    assert_eq!(calculate_norm(&hann_window).unwrap(), ((1.0 / 3.0) as f32).sqrt());
  }

  #[test]
  fn test_calculate_norm_with_empty_window() {
    let hann_window = vec![];
    let result = calculate_norm(&hann_window);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), NormalizationError::InvalidWindowLength);
  }
}