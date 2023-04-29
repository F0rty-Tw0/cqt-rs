use lazy_static::lazy_static;
use ndarray::Array1;
use std::{ collections::HashMap, f32::consts::PI };

// Defining a lazy_static block for the CALCULATED_PHASE_FACTORS
lazy_static! {
  // A lookup table for pre-computed phase factors.
  pub static ref CALCULATED_PHASE_FACTORS: HashMap<(usize, usize), Array1<f32>> = {
      // Defining an array of pre-computed window lengths
      const PRECOMPUTED_WINDOW_LENGTHS: [usize; 5] = [256, 512, 1024, 2048, 4096];
      const SAMPLE_RATES: [usize; 4] = [ 16000, 22050, 44100, 48000];
      // Initialize an empty HashMap for the lookup table
      let mut table = HashMap::new();
      // Iterate over the pre-computed lengths and calculate the phase factors
      for &window_length in &PRECOMPUTED_WINDOW_LENGTHS {
        for &sample_rate in &SAMPLE_RATES {
          let phase_factors = calculate_phase_factors(window_length, sample_rate);
          // Insert the computed phase factors into the lookup table with the corresponding length
          table.insert((window_length, sample_rate), phase_factors);
        }
      }
   
      // Return the populated lookup table
      table
  };
}

/// Retrieve the pre-calculated phase factors for a given window length and sample rate.
///
/// This function takes a `usize` input `window_length` representing the window length
/// and a `usize` input `sample_rate` representing the sample rate.
/// It returns the pre-calculated phase factors as an `Array1<f32>`.
/// The phase factors are computed using a precomputed lookup table for a range of window lengths.
/// If the input `window_length` is not in the lookup table, the phase factors are computed using
/// the `calculate_phase_factors` function.
pub fn get_calculated_phase_factors(window_length: usize, sample_rate: usize) -> Array1<f32> {
  if let Some(phase_factors) = CALCULATED_PHASE_FACTORS.get(&(window_length, sample_rate)) {
    // If it is, return the precomputed value
    phase_factors.clone()
  } else {
    // Otherwise, compute the phase factors using window_length and sample_rate
    calculate_phase_factors(window_length, sample_rate)
  }
}

/// Calculate the phase factors for a given window length and sample rate.
///
/// # Arguments
///
/// * `window_length` - The window length.
/// * `sample_rate` - The sample rate.
///
/// # Returns
///
/// The phase factors as an `Array1<f32>` calculated using the input window length and sample rate.
fn calculate_phase_factors(window_length: usize, sample_rate: usize) -> Array1<f32> {
  Array1::from_shape_fn(window_length, |n| { (-2.0 * PI * (n as f32)) / (sample_rate as f32) })
}

#[cfg(test)]
mod test_phase_factors {
  use super::*;

  #[test]
  fn test_get_calculated_phase_factors() {
    const WINDOW_LENGTH: usize = 256;
    const SAMPLE_RATE: usize = 44100;

    let phase_factors = get_calculated_phase_factors(WINDOW_LENGTH, SAMPLE_RATE);

    for (i, &value) in phase_factors.iter().enumerate() {
      assert_eq!(value, (-2.0 * PI * (i as f32)) / (SAMPLE_RATE as f32));
    }
  }

  #[test]
  fn test_calculate_phase_factors() {
    const WINDOW_LENGTH: usize = 128;
    const SAMPLE_RATE: usize = 22050;

    let phase_factors = calculate_phase_factors(WINDOW_LENGTH, SAMPLE_RATE);

    for (i, &value) in phase_factors.iter().enumerate() {
      assert_eq!(value, (-2.0 * PI * (i as f32)) / (SAMPLE_RATE as f32));
    }
  }
}