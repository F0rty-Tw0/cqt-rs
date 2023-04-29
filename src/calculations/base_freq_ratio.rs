use lazy_static::lazy_static;
use std::collections::HashMap;

// Defining a lazy_static block for the CALCULATED_BASE_FREQ_RATIOS
lazy_static! {
  // A lookup table for pre-computed base frequency ratios.
  pub static ref CALCULATED_BASE_FREQ_RATIOS: HashMap<usize, f32> = {
    // Defining the max bins per octave as a constant
    const MAX_BINS_PER_OCTAVE:usize = 12;

      // Initialize an empty HashMap for the lookup table
      let mut table = HashMap::new();

      // Iterate over the pre-computed lengths and calculate the Base frequency ratios
      for bin in 0..MAX_BINS_PER_OCTAVE {
          let base_freq_ratio = calculate_base_freq_ratio(bin);

          // Insert the computed Base frequency  ratio into the lookup table with the corresponding bin
          table.insert(bin, base_freq_ratio);
      }

      // Return the populated lookup table
      table
  };
  }

// Retrieve the pre-calculated base frequency ratio for a given number of bins per octave.
///
/// This function takes a `usize` input `bins_per_octave` representing the number of bins per octave
/// and returns the pre-calculated base frequency ratio. The base frequency ratio is computed
/// using a precomputed lookup table for a range of bins per octave. If the input `bins_per_octave`
/// is not in the lookup table, the base frequency ratio is computed using the `calculate_base_freq_ratio`
/// function.
pub fn get_calculated_base_freq_ratio(bins_per_octave: usize) -> f32 {
  // Check if the sum-of-squares for the input Hann window length is in the lookup table
  if let Some(base_freq_ratio) = CALCULATED_BASE_FREQ_RATIOS.get(&bins_per_octave) {
    // If it is, return the precomputed value
    base_freq_ratio.clone()
  } else {
    // Otherwise, for some weird reason compute the base frequency using bins per octave
    calculate_base_freq_ratio(bins_per_octave)
  }
}

/// Calculate the base frequency ratio for a given number of bins per octave.
///
/// # Arguments
///
/// * `bins_per_octave` - The number of bins per octave.
///
/// # Returns
///
/// The base frequency ratio calculated using the input number of bins per octave.
fn calculate_base_freq_ratio(bins_per_octave: usize) -> f32 {
  // r = 2^(1/B):
  (2f32).powf(1.0 / (bins_per_octave as f32))
}

#[cfg(test)]
mod test_base_freq_ratios {
  use super::*;

  #[test]
  fn test_get_calculated_base_freq_ratio() {
    let ratio_5 = get_calculated_base_freq_ratio(5);
    let ratio_10 = get_calculated_base_freq_ratio(10);

    assert_eq!(ratio_5, (2f32).powf(1.0 / 5.0));
    assert_eq!(ratio_10, (2f32).powf(1.0 / 10.0));
  }

  #[test]
  fn test_calculate_base_freq_ratio() {
    let ratio_3 = calculate_base_freq_ratio(3);
    let ratio_7 = calculate_base_freq_ratio(7);

    assert_eq!(ratio_3, (2f32).powf(1.0 / 3.0));
    assert_eq!(ratio_7, (2f32).powf(1.0 / 7.0));
  }
}