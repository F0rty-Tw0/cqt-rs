//! Constant-Q transform (CQT) for audio fingerprinting and real-time
//! monitoring.
//!
//! The transform maps a signal onto a logarithmic frequency grid with a
//! configurable number of bins per octave. Each bin has its own analysis
//! window whose length is inversely proportional to the centre frequency, so
//! every bin shares the same quality factor `Q`. A pitch shift of the input
//! therefore becomes a shift along the bin axis, and a tempo change becomes a
//! shift along the frame axis, which is what makes the representation useful
//! for fingerprinting.
//!
//! The implementation follows Brown & Puckette (1992): every frame is
//! transformed with a real FFT and multiplied by a sparse, precomputed
//! spectral kernel. By default each octave is analysed at its own sample
//! rate, halved octave by octave with a half-band filter (Schörkhuber &
//! Klapuri, 2010), so every octave uses a small FFT and a compact kernel.
//!
//! # Batch
//!
//! ```
//! use cqt_rs::{Cqt, CqtParams};
//!
//! let params = CqtParams::builder(44_100, 55.0, 7_040.0)
//!     .bins_per_octave(24)
//!     .build()
//!     .unwrap();
//! let cqt = Cqt::new(params);
//!
//! let signal: Vec<f32> = (0..44_100)
//!     .map(|n| (std::f32::consts::TAU * 440.0 * n as f32 / 44_100.0).sin())
//!     .collect();
//! let spectrogram = cqt.process(&signal, 512).unwrap();
//! assert_eq!(spectrogram.ncols(), cqt.num_bins());
//! ```
//!
#![warn(missing_docs)]

mod cqt;
mod kernel;
mod params;
mod plan;

pub use cqt::{Cqt, CqtError, CqtWorkspace};
pub use kernel::Kernel;
pub use params::{CqtParams, CqtParamsBuilder, CqtParamsError};
pub use rustfft::num_complex::Complex;

/// Converts magnitudes to decibels in place.
///
/// Each value becomes `20 * log10(max(value, amin) / reference)`, then the
/// result is clamped to at least `max - top_db` when `top_db` is given, as in
/// librosa's `amplitude_to_db`. A typical call for fingerprinting is
/// `magnitude_to_db(&mut frame, 1.0, 1e-5, Some(80.0))`.
pub fn magnitude_to_db(values: &mut [f32], reference: f32, amin: f32, top_db: Option<f32>) {
    let reference = reference.max(amin);
    let mut max = f32::NEG_INFINITY;
    for value in values.iter_mut() {
        *value = 20.0 * (value.max(amin) / reference).log10();
        max = max.max(*value);
    }
    if let Some(top_db) = top_db {
        let floor = max - top_db;
        for value in values.iter_mut() {
            *value = value.max(floor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_conversion_matches_definition() {
        let mut values = [1.0, 0.1, 0.0];
        magnitude_to_db(&mut values, 1.0, 1e-5, None);
        assert!((values[0] - 0.0).abs() < 1e-6);
        assert!((values[1] + 20.0).abs() < 1e-4);
        assert!((values[2] + 100.0).abs() < 1e-3);

        let mut values = [1.0, 0.1, 0.0];
        magnitude_to_db(&mut values, 1.0, 1e-5, Some(30.0));
        assert!((values[2] + 30.0).abs() < 1e-6);
    }
}
