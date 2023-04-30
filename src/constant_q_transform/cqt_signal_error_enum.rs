use std::{ error::Error, fmt };

#[derive(Debug, PartialEq)]
pub enum SignalError {
  InvalidHopSize,
  EmptyInputSignal,
}

impl Error for SignalError {}

// Implement the Display trait for the custom error type
impl fmt::Display for SignalError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      SignalError::InvalidHopSize => {
        write!(
          f,
          "Invalid hop size: hop size should be greater than 0 and less than or equal to the window length."
        )
      }
      SignalError::EmptyInputSignal => {
        write!(f, "Empty input signal: the input signal should not be empty.")
      }
    }
  }
}