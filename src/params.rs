//! Transform parameters and their validation.

use std::fmt;

use thiserror::Error;

/// Spectral bandwidth of a Hann window, in units of `sample_rate / length`.
///
/// Used to verify that the pass-band of the highest filter lies below the
/// Nyquist frequency.
pub const HANN_BANDWIDTH: f64 = 1.500_189_54;

/// Errors produced while validating [`CqtParams`].
#[derive(Debug, Clone, Copy, PartialEq, Error)]
#[non_exhaustive]
pub enum CqtParamsError {
    /// The sample rate is zero.
    #[error("sample rate must be greater than zero")]
    InvalidSampleRate,
    /// The minimum frequency is not a finite positive number.
    #[error("minimum frequency must be a finite positive number")]
    InvalidMinFrequency,
    /// The maximum frequency is not finite or is not above the minimum.
    #[error("maximum frequency must be finite and greater than the minimum frequency")]
    InvalidMaxFrequency,
    /// The pass-band of the highest filter would extend beyond Nyquist.
    #[error(
        "the highest filter reaches beyond the Nyquist frequency ({nyquist} Hz): \
         lower `max_freq` or raise the sample rate"
    )]
    AboveNyquist {
        /// Half the sample rate.
        nyquist: f32,
    },
    /// Zero bins per octave were requested.
    #[error("bins per octave must be at least one")]
    InvalidBinsPerOctave,
    /// The filter scale is not a finite positive number.
    #[error("filter scale must be a finite positive number")]
    InvalidFilterScale,
    /// The bandwidth offset (`gamma`) is negative or not finite.
    #[error("gamma must be a finite number greater than or equal to zero")]
    InvalidGamma,
    /// The sparsity threshold is outside `[0, 1)`.
    #[error("sparsity must lie in the range [0, 1)")]
    InvalidSparsity,
    /// The maximum kernel length is smaller than two samples.
    #[error("maximum kernel length must be at least two samples")]
    InvalidMaxKernelLength,
    /// The multi-rate decimation filter would need too many taps because
    /// `max_freq` lies too close to the Nyquist frequency. Lower `max_freq`
    /// or disable multi-rate processing.
    #[error(
        "the decimation filter needs {taps} taps (maximum {max}): \
         lower `max_freq` or use `multirate(false)`"
    )]
    DecimationFilterTooLong {
        /// Taps the design would need.
        taps: usize,
        /// Largest accepted filter.
        max: usize,
    },
    /// The longest kernel (or the requested cap) does not fit in memory.
    #[error("kernel length {length} exceeds the supported maximum of {max} samples")]
    KernelTooLong {
        /// Requested length.
        length: usize,
        /// Largest supported length.
        max: usize,
    },
}

/// Longest kernel that will be accepted, in samples.
pub const MAX_KERNEL_LENGTH: usize = 1 << 26;

/// Validated configuration of a constant-Q (or variable-Q) transform.
///
/// Build one with [`CqtParams::builder`] or [`CqtParams::new`]:
///
/// ```
/// use cqt_rs::CqtParams;
///
/// let params = CqtParams::builder(44_100, 55.0, 7_040.0)
///     .bins_per_octave(24)
///     .build()
///     .unwrap();
/// assert_eq!(params.num_bins(), 169);
/// ```
///
/// Centre frequencies follow `f_k = min_freq * 2^(k / bins_per_octave)` for
/// `k` in `0..num_bins`, and never exceed `max_freq`. The bandwidth of bin
/// `k` is `f_k * (2^(1 / bins_per_octave) - 1) / filter_scale + gamma`, so
/// with `gamma == 0` every filter shares the same quality factor `Q`.
#[derive(Debug, Clone, PartialEq)]
pub struct CqtParams {
    sample_rate: u32,
    min_freq: f32,
    max_freq: f32,
    bins_per_octave: usize,
    filter_scale: f32,
    gamma: f32,
    sparsity: f32,
    max_kernel_length: Option<usize>,
    multirate: bool,
    num_bins: usize,
    q_factor: f32,
}

impl CqtParams {
    /// Starts building a parameter set with the default bins-per-octave (12),
    /// filter scale (1), gamma (0) and sparsity (0.01).
    pub fn builder(sample_rate: u32, min_freq: f32, max_freq: f32) -> CqtParamsBuilder {
        CqtParamsBuilder {
            sample_rate,
            min_freq,
            max_freq,
            bins_per_octave: 12,
            filter_scale: 1.0,
            gamma: 0.0,
            sparsity: 0.01,
            max_kernel_length: None,
            multirate: true,
        }
    }

    /// Builds a parameter set with explicit bins per octave and defaults for
    /// everything else.
    pub fn new(
        sample_rate: u32,
        min_freq: f32,
        max_freq: f32,
        bins_per_octave: usize,
    ) -> Result<Self, CqtParamsError> {
        Self::builder(sample_rate, min_freq, max_freq)
            .bins_per_octave(bins_per_octave)
            .build()
    }

    /// Sample rate of the analysed signal in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Centre frequency of the lowest bin in Hz.
    pub fn min_freq(&self) -> f32 {
        self.min_freq
    }

    /// Upper bound for bin centre frequencies in Hz.
    pub fn max_freq(&self) -> f32 {
        self.max_freq
    }

    /// Number of bins per octave.
    pub fn bins_per_octave(&self) -> usize {
        self.bins_per_octave
    }

    /// Multiplier applied to every filter length.
    pub fn filter_scale(&self) -> f32 {
        self.filter_scale
    }

    /// Constant bandwidth offset in Hz (variable-Q transform).
    pub fn gamma(&self) -> f32 {
        self.gamma
    }

    /// Fraction of each bin's spectral L1 mass that may be discarded.
    pub fn sparsity(&self) -> f32 {
        self.sparsity
    }

    /// Optional cap on the kernel length in samples.
    pub fn max_kernel_length(&self) -> Option<usize> {
        self.max_kernel_length
    }

    /// Whether each octave is analysed at its own halved sample rate.
    pub fn multirate(&self) -> bool {
        self.multirate
    }

    /// Total number of frequency bins.
    pub fn num_bins(&self) -> usize {
        self.num_bins
    }

    /// Quality factor `filter_scale / (2^(1 / bins_per_octave) - 1)` shared by
    /// every bin when `gamma == 0`.
    pub fn q_factor(&self) -> f32 {
        self.q_factor
    }

    /// Centre frequency of bin `k` in Hz.
    pub fn center_freq(&self, bin: usize) -> f32 {
        self.center_freq_f64(bin) as f32
    }

    pub(crate) fn center_freq_f64(&self, bin: usize) -> f64 {
        f64::from(self.min_freq) * 2f64.powf(bin as f64 / self.bins_per_octave as f64)
    }

    /// Bandwidth of bin `k` in Hz.
    pub fn bandwidth(&self, bin: usize) -> f32 {
        self.bandwidth_f64(bin) as f32
    }

    pub(crate) fn bandwidth_f64(&self, bin: usize) -> f64 {
        let alpha = 2f64.powf(1.0 / self.bins_per_octave as f64) - 1.0;
        self.center_freq_f64(bin) * alpha / f64::from(self.filter_scale) + f64::from(self.gamma)
    }

    /// Length in samples of the analysis window of bin `k`, before any FFT
    /// zero padding.
    pub fn kernel_length(&self, bin: usize) -> usize {
        let ideal = f64::from(self.sample_rate) / self.bandwidth_f64(bin);
        let length = (ideal.round() as usize).max(2);
        match self.max_kernel_length {
            Some(cap) => length.min(cap),
            None => length,
        }
    }

    /// Smallest power of two that holds the longest kernel at the full
    /// sample rate. This is the FFT length of the single-rate engine and the
    /// full-rate span of the lowest octave's frame in the multi-rate engine.
    pub fn fft_length(&self) -> usize {
        self.kernel_length(0).next_power_of_two()
    }
}

/// Builder for [`CqtParams`].
#[derive(Debug, Clone)]
pub struct CqtParamsBuilder {
    sample_rate: u32,
    min_freq: f32,
    max_freq: f32,
    bins_per_octave: usize,
    filter_scale: f32,
    gamma: f32,
    sparsity: f32,
    max_kernel_length: Option<usize>,
    multirate: bool,
}

impl CqtParamsBuilder {
    /// Number of bins per octave. Twelve gives semitone resolution; 24 or 36
    /// make small pitch shifts land on integer bin offsets.
    pub fn bins_per_octave(mut self, bins_per_octave: usize) -> Self {
        self.bins_per_octave = bins_per_octave;
        self
    }

    /// Scales every window length. Values below one shorten the filters for
    /// better time resolution at the cost of frequency resolution.
    pub fn filter_scale(mut self, filter_scale: f32) -> Self {
        self.filter_scale = filter_scale;
        self
    }

    /// Adds a constant bandwidth offset in Hz, turning the transform into a
    /// variable-Q transform. Positive values shorten the low-frequency
    /// windows, which lowers latency and FFT size.
    pub fn gamma(mut self, gamma: f32) -> Self {
        self.gamma = gamma;
        self
    }

    /// Drops the smallest spectral kernel coefficients of every bin until
    /// this fraction of the bin's L1 mass is gone (librosa semantics). The
    /// magnitude error of a bin is bounded by that fraction of the frame's
    /// peak spectral magnitude. Zero keeps every coefficient.
    pub fn sparsity(mut self, sparsity: f32) -> Self {
        self.sparsity = sparsity;
        self
    }

    /// Caps every kernel at this many samples. Bins whose ideal window is
    /// longer lose some frequency resolution but the FFT stays small.
    pub fn max_kernel_length(mut self, max_kernel_length: usize) -> Self {
        self.max_kernel_length = Some(max_kernel_length);
        self
    }

    /// Selects the engine. With `true` (the default) every octave is analysed
    /// at its own sample rate, halved octave by octave with an 80 dB
    /// half-band filter, so each octave uses a small FFT and a compact
    /// kernel. With `false` all bins share one FFT at the full rate, which
    /// avoids the decimation filter at a higher per-frame cost.
    pub fn multirate(mut self, multirate: bool) -> Self {
        self.multirate = multirate;
        self
    }

    /// Validates the configuration.
    pub fn build(self) -> Result<CqtParams, CqtParamsError> {
        if self.sample_rate == 0 {
            return Err(CqtParamsError::InvalidSampleRate);
        }
        if !(self.min_freq.is_finite() && self.min_freq > 0.0) {
            return Err(CqtParamsError::InvalidMinFrequency);
        }
        if !(self.max_freq.is_finite() && self.max_freq > self.min_freq) {
            return Err(CqtParamsError::InvalidMaxFrequency);
        }
        if self.bins_per_octave == 0 {
            return Err(CqtParamsError::InvalidBinsPerOctave);
        }
        if !(self.filter_scale.is_finite() && self.filter_scale > 0.0) {
            return Err(CqtParamsError::InvalidFilterScale);
        }
        if !(self.gamma.is_finite() && self.gamma >= 0.0) {
            return Err(CqtParamsError::InvalidGamma);
        }
        if !(self.sparsity.is_finite() && (0.0..1.0).contains(&self.sparsity)) {
            return Err(CqtParamsError::InvalidSparsity);
        }
        if let Some(cap) = self.max_kernel_length {
            if cap < 2 {
                return Err(CqtParamsError::InvalidMaxKernelLength);
            }
            if cap > MAX_KERNEL_LENGTH {
                return Err(CqtParamsError::KernelTooLong {
                    length: cap,
                    max: MAX_KERNEL_LENGTH,
                });
            }
        }

        let alpha = 2f64.powf(1.0 / self.bins_per_octave as f64) - 1.0;
        let q_factor = f64::from(self.filter_scale) / alpha;
        let octaves = (f64::from(self.max_freq) / f64::from(self.min_freq)).log2();
        // Bins whose centre lies at or below max_freq (with a little slack for
        // rounding when max_freq sits exactly on the grid).
        let num_bins = (octaves * self.bins_per_octave as f64 + 1e-9).floor() as usize + 1;

        let params = CqtParams {
            sample_rate: self.sample_rate,
            min_freq: self.min_freq,
            max_freq: self.max_freq,
            bins_per_octave: self.bins_per_octave,
            filter_scale: self.filter_scale,
            gamma: self.gamma,
            sparsity: self.sparsity,
            max_kernel_length: self.max_kernel_length,
            multirate: self.multirate,
            num_bins,
            q_factor: q_factor as f32,
        };

        let nyquist = f64::from(self.sample_rate) / 2.0;
        let top = params.num_bins - 1;
        let top_length = params.kernel_length(top) as f64;
        let top_bandwidth = f64::from(self.sample_rate) / top_length * HANN_BANDWIDTH;
        if params.center_freq_f64(top) + 0.5 * top_bandwidth > nyquist {
            return Err(CqtParamsError::AboveNyquist {
                nyquist: nyquist as f32,
            });
        }

        let longest = params.kernel_length(0);
        if longest > MAX_KERNEL_LENGTH {
            return Err(CqtParamsError::KernelTooLong {
                length: longest,
                max: MAX_KERNEL_LENGTH,
            });
        }

        // Validates per-octave Nyquist limits and the decimation filter.
        crate::plan::plan(&params)?;

        Ok(params)
    }
}

impl fmt::Display for CqtParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CQT {} Hz, {}-{} Hz, {} bins/octave, {} bins, Q={:.2}, gamma={}, {}",
            self.sample_rate,
            self.min_freq,
            self.max_freq,
            self.bins_per_octave,
            self.num_bins,
            self.q_factor,
            self.gamma,
            if self.multirate {
                "multi-rate"
            } else {
                "single-rate"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_bins_up_to_max_freq() {
        let params = CqtParams::new(44_100, 55.0, 7_040.0, 12).unwrap();
        // 7040 / 55 = 128 = 2^7 -> 7 octaves + the top bin itself.
        assert_eq!(params.num_bins(), 85);
        assert!((params.center_freq(84) - 7_040.0).abs() < 0.01);
        assert!(params.center_freq(params.num_bins() - 1) <= params.max_freq());
    }

    #[test]
    fn never_exceeds_max_freq() {
        let params = CqtParams::new(44_100, 20.0, 7_902.1, 12).unwrap();
        assert_eq!(params.num_bins(), 104);
        assert!(params.center_freq(103) <= 7_902.1);
        assert!(params.center_freq(104) > 7_902.1);
    }

    #[test]
    fn q_factor_matches_classic_formula() {
        let params = CqtParams::new(44_100, 55.0, 7_040.0, 12).unwrap();
        assert!((params.q_factor() - 16.817).abs() < 1e-3);
        let params = CqtParams::new(44_100, 55.0, 7_040.0, 24).unwrap();
        assert!((params.q_factor() - 34.127).abs() < 1e-3);
    }

    #[test]
    fn kernel_lengths_scale_with_frequency() {
        let params = CqtParams::new(44_100, 55.0, 7_040.0, 12).unwrap();
        let n0 = params.kernel_length(0);
        let n12 = params.kernel_length(12);
        assert!((n0 as f64 / n12 as f64 - 2.0).abs() < 0.01);
        assert_eq!(params.fft_length(), n0.next_power_of_two());
    }

    #[test]
    fn gamma_shortens_low_bins() {
        let cqt = CqtParams::new(44_100, 55.0, 7_040.0, 12).unwrap();
        let vqt = CqtParams::builder(44_100, 55.0, 7_040.0)
            .gamma(20.0)
            .build()
            .unwrap();
        assert!(vqt.kernel_length(0) < cqt.kernel_length(0));
        assert!(vqt.fft_length() < cqt.fft_length());
    }

    #[test]
    fn cap_limits_kernel_and_fft_length() {
        let params = CqtParams::builder(44_100, 20.0, 4_000.0)
            .max_kernel_length(2_048)
            .build()
            .unwrap();
        assert_eq!(params.kernel_length(0), 2_048);
        assert_eq!(params.fft_length(), 2_048);
        assert!(params.kernel_length(params.num_bins() - 1) < 2_048);
    }

    #[test]
    fn rejects_invalid_values() {
        assert_eq!(
            CqtParams::new(0, 20.0, 4_000.0, 12).unwrap_err(),
            CqtParamsError::InvalidSampleRate
        );
        assert_eq!(
            CqtParams::new(44_100, -1.0, 4_000.0, 12).unwrap_err(),
            CqtParamsError::InvalidMinFrequency
        );
        assert_eq!(
            CqtParams::new(44_100, 20.0, 20.0, 12).unwrap_err(),
            CqtParamsError::InvalidMaxFrequency
        );
        assert_eq!(
            CqtParams::new(44_100, 20.0, 4_000.0, 0).unwrap_err(),
            CqtParamsError::InvalidBinsPerOctave
        );
        assert_eq!(
            CqtParams::builder(44_100, 20.0, 4_000.0)
                .filter_scale(0.0)
                .build()
                .unwrap_err(),
            CqtParamsError::InvalidFilterScale
        );
        assert_eq!(
            CqtParams::builder(44_100, 20.0, 4_000.0)
                .gamma(-1.0)
                .build()
                .unwrap_err(),
            CqtParamsError::InvalidGamma
        );
        assert_eq!(
            CqtParams::builder(44_100, 20.0, 4_000.0)
                .sparsity(1.0)
                .build()
                .unwrap_err(),
            CqtParamsError::InvalidSparsity
        );
        assert_eq!(
            CqtParams::builder(44_100, 20.0, 4_000.0)
                .max_kernel_length(1)
                .build()
                .unwrap_err(),
            CqtParamsError::InvalidMaxKernelLength
        );
    }

    #[test]
    fn rejects_infeasible_decimation_filters() {
        // A narrow filter right below Nyquist leaves no room for the
        // half-band transition band.
        let err = CqtParams::new(16_000, 55.0, 7_995.0, 384).unwrap_err();
        assert!(matches!(
            err,
            CqtParamsError::DecimationFilterTooLong { .. }
        ));
        assert!(
            CqtParams::builder(16_000, 55.0, 7_995.0)
                .bins_per_octave(384)
                .multirate(false)
                .build()
                .is_ok()
        );
        // Moderate settings near Nyquist still work with a longer filter.
        assert!(CqtParams::new(16_000, 55.0, 7_040.0, 12).is_ok());
    }

    #[test]
    fn rejects_filters_above_nyquist() {
        let err = CqtParams::new(16_000, 20.0, 7_999.0, 12).unwrap_err();
        assert!(matches!(err, CqtParamsError::AboveNyquist { .. }));
        assert!(CqtParams::new(16_000, 20.0, 7_000.0, 12).is_ok());
    }
}
