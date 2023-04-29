use lazy_static::lazy_static;
use std::{ collections::HashMap, error::Error, fmt };

/// Error type for the Hann window function.
#[derive(Debug)]
pub enum QFactorError {
  InvalidBinsPerOctave,
}

// Implement the Error trait for the ComplexHannWindowError
impl Error for QFactorError {}

// Implement the Display trait for the ComplexHannWindowError enum
impl fmt::Display for QFactorError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    // Write the error message to the Formatter
    match self {
      QFactorError::InvalidBinsPerOctave => {
        write!(f, "Invalid bins per octave: must be a positive integer")
      }
    }
  }
}

// Defining a lazy_static block for the Q_FACTOR_LOOKUP_TABLE
lazy_static! {
  // A lookup table for pre-computed bins_per_octave;.
  static ref Q_FACTOR_LOOKUP_TABLE: HashMap<usize, f32> = {
    // Defining an array of pre-computed bins_per_octave
    const PRECOMPUTED_BIN_SIZES: [usize; 12] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

    // Initialize an empty HashMap for the lookup table
    let mut table = HashMap::new();

     // Iterate over the pre-computed bins_per_octave and calculate the q factor
    for &bins_per_octave in &PRECOMPUTED_BIN_SIZES {
        let q_factor = calculate_q_factor(bins_per_octave);

        // Insert the calculated q_factor into the lookup table with the corresponding bins_per_octave
        table.insert(bins_per_octave, q_factor);
    }

    // Return the populated lookup table
    table
  };
}

/// Returns the pre-calculated Q factor for the given number of bins per octave.
///
/// If the pre-calculated value is available in the lookup table, it is returned.
/// Otherwise, the Q factor is calculated using the `calculate_q_factor` function.
///
/// # Arguments
///
/// * `bins_per_octave`: A `usize` representing the number of frequency bins per octave.
///
/// # Returns
///
///  Result<f32, QFactorError> -  The Q factor for the given number of bins per octave.
pub fn get_calculated_q_factor(bins_per_octave: usize) -> Result<f32, QFactorError> {
  if let Some(q_factor) = Q_FACTOR_LOOKUP_TABLE.get(&bins_per_octave) {
    Ok(q_factor.clone())
  } else if bins_per_octave > 0 {
    // If the bins_per_octave is not in the lookup table, calculate the q factor
    Ok(calculate_q_factor(bins_per_octave))
  } else {
    Err(QFactorError::InvalidBinsPerOctave)
  }
}

/// Calculates the Q factor for the CQT filterbank.
///
/// The quality factor (Q) in the context of the Constant-Q Transform (CQT)
/// represents the ratio of the center frequency (f) of a filter to its bandwidth (∆f).
/// It is a measure of the selectivity of the filter, with higher
/// Q values indicating a more selective filter with a narrower bandwidth relative to its center frequency.
/// The Q factor can be calculated using the following formula:
///
/// freq_ratio = 2 ^ (1 / bins_per_octave)
/// ∆f = f * (freq_ratio - 1)
/// Q = f / ∆f
///
/// In the CQT, the filters are designed to have a constant Q across all frequency bins.
/// This means that the bandwidth of each filter increases proportionally with the center frequency,
/// providing a logarithmic frequency resolution that mimics human perception of sound.
///
///
/// # Arguments
///
/// * `center_freq` - The center frequency of the filter in the filterbank.
/// * `bins_per_octave` - The number of bins per octave in the filterbank.
///
/// # Returns
///
/// * `f32` - The calculated Q factor.
fn calculate_q_factor(bins_per_octave: usize) -> f32 {
  // Calculate the frequency ratio for the given bins per octave
  let freq_ratio = (2f32).powf(1.0 / (bins_per_octave as f32));

  // Calculate and return the Q factor directly using center_freq and freq_ratio
  1.0 / (freq_ratio - 1.0)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_calculated_q_factor() {
    // Test with bins_per_octave in the lookup table
    assert_eq!(get_calculated_q_factor(12).unwrap(), 16.81714);
    assert_eq!(get_calculated_q_factor(24).unwrap(), 34.127083);

    // Test with bins_per_octave not in the lookup table
    assert_eq!(get_calculated_q_factor(48).unwrap(), 68.750626);
  }

  #[test]
  fn test_get_calculated_q_factor_with_zero_bins_per_octave() {
    assert!(matches!(get_calculated_q_factor(0), Err(QFactorError::InvalidBinsPerOctave)));
  }
}