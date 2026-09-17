//! Sparse spectral kernel of the transform.
//!
//! Each bin `k` is defined by a time-domain atom
//! `a_k[n] = w_k[n] * exp(+j * 2π * f_k * (n - c) / sr)` where `w_k` is a
//! Hann window of length `N_k` centred at `c` inside an FFT frame of length
//! `N`, and `f_k` is the centre frequency. The transform coefficient is the
//! inner product `X_k = Σ_n x[n] * conj(a_k[n])`. By Parseval's theorem the
//! same value follows from the frame spectrum `X[m]`:
//!
//! `X_k = (1 / N) * Σ_m X[m] * conj(A_k[m])`
//!
//! where `A_k` is the DFT of `a_k`. Because `A_k` is concentrated around
//! `f_k`, all but a handful of coefficients can be dropped (Brown & Puckette,
//! 1992). For a real input `X[N - m] = conj(X[m])`, so only the `N / 2 + 1`
//! coefficients of a real FFT are needed; the negative-frequency part of each
//! kernel is evaluated against the conjugated positive spectrum.
//!
//! The retained coefficients of a bin form a few contiguous stretches of
//! spectrum indices, stored as [`Run`]s so the product streams through memory
//! without indirection.

use std::ops::Range;

use hann_rs::{HannMode, hann_in_place_f64};
use rustfft::{Fft, FftPlanner, num_complex::Complex};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Independent accumulators of [`dot`]; the batch path reproduces the same
/// split per frame so that both paths sum in the same order.
const LANES: usize = 8;

/// Frames per block of the batch path. Sixteen `f32` fill one cache line,
/// so every retained coefficient of a block is one line per plane;
/// measured against 8 and 32, see docs/NOTES.md.
pub(crate) const FRAMES: usize = 16;

/// The coefficients a [`Kernel`] retains of up to [`FRAMES`] real-FFT
/// spectra, stored as separate real and imaginary planes with the frames
/// of one coefficient contiguous, so that the kernel's multiply-adds
/// vectorize across frames.
#[derive(Debug, Clone)]
pub(crate) struct Spectra {
    /// Spectrum indices covered, `Kernel::band`.
    band: Range<usize>,
    /// `re[(k - band.start) * FRAMES + f]` is the real part of coefficient
    /// `k` of frame `f`.
    re: Vec<f32>,
    im: Vec<f32>,
    frames: usize,
}

impl Spectra {
    /// Storage for the coefficients `kernel` retains.
    pub fn new(kernel: &Kernel) -> Self {
        let len = kernel.band.len() * FRAMES;
        Self {
            band: kernel.band.clone(),
            re: vec![0.0; len],
            im: vec![0.0; len],
            frames: 0,
        }
    }

    /// Forgets every stored frame.
    pub fn clear(&mut self) {
        self.frames = 0;
    }

    /// Stores the retained coefficients of `spectrum` as the next frame.
    pub fn push(&mut self, spectrum: &[Complex<f32>]) {
        let f = self.frames;
        assert!(f < FRAMES);
        let (re, _) = self.re.as_chunks_mut::<FRAMES>();
        let (im, _) = self.im.as_chunks_mut::<FRAMES>();
        for ((re, im), c) in re.iter_mut().zip(im).zip(&spectrum[self.band.clone()]) {
            re[f] = c.re;
            im[f] = c.im;
        }
        self.frames = f + 1;
    }
}

/// Per-frame accumulators of [`Kernel::accumulate_block`]: the total of
/// one bin and the [`LANES`] partial sums of the run in progress.
struct Accumulators {
    re: [f32; FRAMES],
    im: [f32; FRAMES],
    lane_re: [[f32; FRAMES]; LANES],
    lane_im: [[f32; FRAMES]; LANES],
}

impl Accumulators {
    fn new() -> Self {
        Self {
            re: [0.0; FRAMES],
            im: [0.0; FRAMES],
            lane_re: [[0.0; FRAMES]; LANES],
            lane_im: [[0.0; FRAMES]; LANES],
        }
    }
}

/// A contiguous stretch of retained spectral coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    /// First real-FFT spectrum index (`0..=N / 2`).
    start: u32,
    /// Number of coefficients.
    len: u32,
    /// Offset of the first weight in [`Kernel::weights`].
    weight_offset: u32,
    /// Whether the spectrum must be conjugated (negative-frequency part).
    conjugate: bool,
}

/// Sparse spectral kernel shared by every frame of a [`crate::Cqt`].
#[derive(Debug, Clone, PartialEq)]
pub struct Kernel {
    fft_length: usize,
    num_bins: usize,
    /// `runs[run_offsets[k]..run_offsets[k + 1]]` belong to bin `k`.
    run_offsets: Vec<usize>,
    runs: Vec<Run>,
    /// Smallest range of spectrum indices containing every run.
    band: Range<usize>,
    /// `conj(A_k[m]) / N`, pre-scaled so the accumulation needs no division.
    weights: Vec<Complex<f32>>,
    lengths: Vec<usize>,
    frequencies: Vec<f32>,
}

/// Inputs shared by every bin of one kernel.
struct BinSpec<'a> {
    sample_rate: f64,
    frequencies: &'a [f64],
    lengths: &'a [usize],
    sparsity: f32,
}

/// Retained coefficients of one bin before packing.
struct BinKernel {
    runs: Vec<Run>,
    weights: Vec<Complex<f32>>,
}

impl Kernel {
    /// Builds the kernel for bins with the given centre frequencies (Hz) and
    /// window lengths (samples) at `sample_rate`, evaluated with an FFT of
    /// `fft_length` points.
    pub(crate) fn build(
        sample_rate: f64,
        frequencies: &[f64],
        lengths: &[usize],
        fft_length: usize,
        sparsity: f32,
    ) -> Self {
        assert_eq!(frequencies.len(), lengths.len());
        assert!(lengths.iter().all(|&len| len <= fft_length));
        let num_bins = frequencies.len();
        let fft = FftPlanner::<f64>::new().plan_fft_forward(fft_length);
        let spec = BinSpec {
            sample_rate,
            frequencies,
            lengths,
            sparsity,
        };

        let bins = Self::build_bins(&spec, &fft);

        let mut run_offsets = Vec::with_capacity(num_bins + 1);
        let mut runs = Vec::new();
        let mut weights = Vec::new();
        run_offsets.push(0);
        for bin in bins {
            let base = weights.len() as u32;
            runs.extend(bin.runs.iter().map(|run| Run {
                weight_offset: run.weight_offset + base,
                ..*run
            }));
            weights.extend_from_slice(&bin.weights);
            run_offsets.push(runs.len());
        }
        runs.shrink_to_fit();
        weights.shrink_to_fit();
        let band = runs
            .iter()
            .map(|run| run.start as usize..(run.start + run.len) as usize)
            .reduce(|a, b| a.start.min(b.start)..a.end.max(b.end))
            .unwrap_or(0..0);

        Self {
            fft_length,
            num_bins,
            run_offsets,
            runs,
            band,
            weights,
            lengths: lengths.to_vec(),
            frequencies: frequencies.iter().map(|&f| f as f32).collect(),
        }
    }

    #[cfg(feature = "parallel")]
    fn build_bins(spec: &BinSpec<'_>, fft: &std::sync::Arc<dyn Fft<f64>>) -> Vec<BinKernel> {
        (0..spec.frequencies.len())
            .into_par_iter()
            .map_init(
                || Workspace::new(fft.as_ref()),
                |workspace, bin| workspace.build(spec, fft.as_ref(), bin),
            )
            .collect()
    }

    #[cfg(not(feature = "parallel"))]
    fn build_bins(spec: &BinSpec<'_>, fft: &std::sync::Arc<dyn Fft<f64>>) -> Vec<BinKernel> {
        let mut workspace = Workspace::new(fft.as_ref());
        (0..spec.frequencies.len())
            .map(|bin| workspace.build(spec, fft.as_ref(), bin))
            .collect()
    }

    /// FFT length the kernel was built for.
    pub fn fft_length(&self) -> usize {
        self.fft_length
    }

    /// Number of frequency bins.
    pub fn num_bins(&self) -> usize {
        self.num_bins
    }

    /// Total number of retained spectral coefficients across all bins.
    pub fn num_nonzeros(&self) -> usize {
        self.weights.len()
    }

    /// Window length in samples of each bin.
    pub fn lengths(&self) -> &[usize] {
        &self.lengths
    }

    /// Centre frequency in Hz of each bin.
    pub fn frequencies(&self) -> &[f32] {
        &self.frequencies
    }

    #[inline(always)]
    fn accumulate(&self, spectrum: &[Complex<f32>], bin: usize) -> Complex<f32> {
        let runs = &self.runs[self.run_offsets[bin]..self.run_offsets[bin + 1]];
        let mut acc = Complex::<f32>::default();
        for run in runs {
            let start = run.start as usize;
            let len = run.len as usize;
            let offset = run.weight_offset as usize;
            let spectrum = &spectrum[start..start + len];
            let weights = &self.weights[offset..offset + len];
            acc += if run.conjugate {
                dot::<true>(weights, spectrum)
            } else {
                dot::<false>(weights, spectrum)
            };
        }
        acc
    }

    /// Applies the kernel to a real-FFT spectrum of `fft_length / 2 + 1`
    /// coefficients, writing the complex coefficient of every bin.
    #[inline]
    pub fn apply(&self, spectrum: &[Complex<f32>], out: &mut [Complex<f32>]) {
        assert_eq!(spectrum.len(), self.fft_length / 2 + 1);
        assert_eq!(out.len(), self.num_bins);
        for (bin, value) in out.iter_mut().enumerate() {
            *value = self.accumulate(spectrum, bin);
        }
    }

    /// Applies the kernel and writes magnitudes only.
    #[inline]
    pub fn apply_magnitude(&self, spectrum: &[Complex<f32>], out: &mut [f32]) {
        assert_eq!(spectrum.len(), self.fft_length / 2 + 1);
        assert_eq!(out.len(), self.num_bins);
        for (bin, value) in out.iter_mut().enumerate() {
            *value = self.accumulate(spectrum, bin).norm();
        }
    }

    /// [`Kernel::accumulate`] for every frame of `spectra` at once. Each
    /// frame goes through exactly the additions of [`dot`], in the same
    /// order, so the result is bit-identical to the per-frame path.
    ///
    /// Every loop runs over all [`FRAMES`] slots, also in a partial block,
    /// so that it unrolls and vectorizes; the unused slots hold stale
    /// values that are never written out.
    #[inline(always)]
    fn accumulate_block(&self, spectra: &Spectra, bin: usize, acc: &mut Accumulators) {
        acc.re = [0.0; FRAMES];
        acc.im = [0.0; FRAMES];
        let (re_rows, _) = spectra.re.as_chunks::<FRAMES>();
        let (im_rows, _) = spectra.im.as_chunks::<FRAMES>();
        let runs = &self.runs[self.run_offsets[bin]..self.run_offsets[bin + 1]];
        for run in runs {
            let start = run.start as usize - spectra.band.start;
            let len = run.len as usize;
            let offset = run.weight_offset as usize;
            let weights = &self.weights[offset..offset + len];
            // Whole chunks of LANES coefficients spread over the lanes; the
            // remainder goes to lane 0, exactly as in `dot`.
            let chunked = len / LANES * LANES;
            let lanes = if chunked > 0 { LANES } else { 1 };
            for lane in 0..lanes {
                acc.lane_re[lane] = [0.0; FRAMES];
                acc.lane_im[lane] = [0.0; FRAMES];
            }
            for (i, w) in weights.iter().enumerate() {
                let lane = if i < chunked { i % LANES } else { 0 };
                let k = start + i;
                let (sre, sim) = (&re_rows[k], &im_rows[k]);
                let (lre, lim) = (&mut acc.lane_re[lane], &mut acc.lane_im[lane]);
                if run.conjugate {
                    multiply_add(lre, lim, *w, sre, sim, -1.0);
                } else {
                    multiply_add(lre, lim, *w, sre, sim, 1.0);
                }
            }
            // A lane never holds -0.0, so adding the untouched +0.0 lanes
            // would change nothing: sum only the lanes that were used.
            for f in 0..FRAMES {
                let mut re = acc.lane_re[0][f];
                let mut im = acc.lane_im[0][f];
                for lane in 1..lanes {
                    re += acc.lane_re[lane][f];
                    im += acc.lane_im[lane][f];
                }
                acc.re[f] += re;
                acc.im[f] += im;
            }
        }
    }

    /// [`Kernel::apply`] for every frame of `spectra`; bin `b` of frame
    /// `f` is written to `out[f * stride + b]`.
    pub(crate) fn apply_block(&self, spectra: &Spectra, out: &mut [Complex<f32>], stride: usize) {
        let mut acc = Accumulators::new();
        for bin in 0..self.num_bins {
            self.accumulate_block(spectra, bin, &mut acc);
            for f in 0..spectra.frames {
                out[f * stride + bin] = Complex::new(acc.re[f], acc.im[f]);
            }
        }
    }

    /// [`Kernel::apply_magnitude`] for every frame of `spectra`, laid out
    /// as in [`Kernel::apply_block`].
    pub(crate) fn apply_magnitude_block(&self, spectra: &Spectra, out: &mut [f32], stride: usize) {
        let mut acc = Accumulators::new();
        for bin in 0..self.num_bins {
            self.accumulate_block(spectra, bin, &mut acc);
            for f in 0..spectra.frames {
                out[f * stride + bin] = Complex::new(acc.re[f], acc.im[f]).norm();
            }
        }
    }
}

/// Sum of `w * s` (or `w * conj(s)` when `CONJ`) over two equally long
/// slices, accumulated in independent lanes so the multiply-adds pipeline
/// and vectorize instead of serialising on one accumulator.
#[inline(always)]
fn dot<const CONJ: bool>(weights: &[Complex<f32>], spectrum: &[Complex<f32>]) -> Complex<f32> {
    let sign = if CONJ { -1.0f32 } else { 1.0f32 };
    let mut re = [0.0f32; LANES];
    let mut im = [0.0f32; LANES];

    let (w_chunks, w_rest) = weights.as_chunks::<LANES>();
    let (s_chunks, s_rest) = spectrum.as_chunks::<LANES>();
    for (w, s) in w_chunks.iter().zip(s_chunks) {
        for lane in 0..LANES {
            let (wr, wi) = (w[lane].re, w[lane].im);
            let (sr, si) = (s[lane].re, sign * s[lane].im);
            re[lane] += wr * sr - wi * si;
            im[lane] += wr * si + wi * sr;
        }
    }
    for (w, s) in w_rest.iter().zip(s_rest) {
        let si = sign * s.im;
        re[0] += w.re * s.re - w.im * si;
        im[0] += w.re * si + w.im * s.re;
    }
    Complex::new(re.iter().sum(), im.iter().sum())
}

/// `acc += w * (re, sign * im)` over the frames of one coefficient; the
/// same expression per element as [`dot`].
#[inline(always)]
fn multiply_add(
    acc_re: &mut [f32; FRAMES],
    acc_im: &mut [f32; FRAMES],
    w: Complex<f32>,
    re: &[f32; FRAMES],
    im: &[f32; FRAMES],
    sign: f32,
) {
    for f in 0..FRAMES {
        let (sr, si) = (re[f], sign * im[f]);
        acc_re[f] += w.re * sr - w.im * si;
        acc_im[f] += w.re * si + w.im * sr;
    }
}

/// Magnitude below which coefficients are dropped so that at most
/// `sparsity` of the kernel's L1 mass is lost (librosa's `sparsify_rows`).
/// Returns a negative value when nothing may be dropped.
fn sparsity_threshold(atom: &[Complex<f64>], magnitudes: &mut Vec<f64>, sparsity: f32) -> f64 {
    if sparsity <= 0.0 {
        return -1.0;
    }
    magnitudes.clear();
    magnitudes.extend(atom.iter().map(|c| c.norm()));
    magnitudes.sort_unstable_by(|a, b| a.total_cmp(b));
    let total: f64 = magnitudes.iter().sum();
    let budget = total * f64::from(sparsity);
    // Every coefficient at or below the threshold is dropped, so a run of
    // equal magnitudes is dropped as a whole or kept as a whole: spending
    // the budget on part of a tie would discard more mass than allowed.
    let mut dropped = 0.0;
    let mut threshold = -1.0;
    let mut start = 0;
    while start < magnitudes.len() {
        let magnitude = magnitudes[start];
        let mut end = start;
        while end < magnitudes.len() && magnitudes[end] == magnitude {
            end += 1;
        }
        let run = magnitude * (end - start) as f64;
        if dropped + run > budget {
            break;
        }
        dropped += run;
        threshold = magnitude;
        start = end;
    }
    threshold
}

/// Per-thread buffers for kernel construction.
struct Workspace {
    atom: Vec<Complex<f64>>,
    window: Vec<f64>,
    scratch: Vec<Complex<f64>>,
    magnitudes: Vec<f64>,
}

impl Workspace {
    fn new(fft: &dyn Fft<f64>) -> Self {
        Self {
            atom: vec![Complex::default(); fft.len()],
            window: Vec::new(),
            scratch: vec![Complex::default(); fft.get_inplace_scratch_len()],
            magnitudes: Vec::with_capacity(fft.len()),
        }
    }

    fn build(&mut self, spec: &BinSpec<'_>, fft: &dyn Fft<f64>, bin: usize) -> BinKernel {
        let fft_length = fft.len();
        let half = fft_length / 2;
        let sample_rate = spec.sample_rate;
        let freq = spec.frequencies[bin];
        let length = spec.lengths[bin];

        self.window.resize(length, 0.0);
        hann_in_place_f64(&mut self.window, HannMode::Periodic);
        let window_sum: f64 = self.window.iter().sum();
        // Scale so that a real sinusoid of amplitude A at the centre
        // frequency yields |X_k| ≈ A.
        let gain = 2.0 / window_sum;

        self.atom.fill(Complex::default());
        let start = (fft_length - length) / 2;
        let centre = fft_length as f64 / 2.0;
        let omega = std::f64::consts::TAU * freq / sample_rate;
        for (i, &w) in self.window.iter().enumerate() {
            let n = (start + i) as f64 - centre;
            self.atom[start + i] = Complex::from_polar(w * gain, omega * n);
        }
        fft.process_with_scratch(&mut self.atom, &mut self.scratch);

        let threshold = sparsity_threshold(&self.atom, &mut self.magnitudes, spec.sparsity);
        let scale = 1.0 / fft_length as f64;
        let keep = |c: &Complex<f64>| c.norm() > threshold;
        let weight = |c: &Complex<f64>| {
            let w = c.conj() * scale;
            Complex::new(w.re as f32, w.im as f32)
        };

        let mut runs = Vec::new();
        let mut weights = Vec::new();
        let mut push = |index: usize, conjugate: bool, coefficient: &Complex<f64>| {
            let extend = runs.last().is_some_and(|run: &Run| {
                run.conjugate == conjugate && run.start as usize + run.len as usize == index
            });
            if extend {
                runs.last_mut().unwrap().len += 1;
            } else {
                runs.push(Run {
                    start: index as u32,
                    len: 1,
                    weight_offset: weights.len() as u32,
                    conjugate,
                });
            }
            weights.push(weight(coefficient));
        };

        // Positive half: m in 0..=N/2, evaluated against X[m].
        for (m, coefficient) in self.atom.iter().enumerate().take(half + 1) {
            if keep(coefficient) {
                push(m, false, coefficient);
            }
        }
        // Negative half: m in N/2+1..N, evaluated against conj(X[N - m]).
        // Walk m downwards so the spectrum index N - m ascends.
        for m in (half + 1..fft_length).rev() {
            let coefficient = &self.atom[m];
            if keep(coefficient) {
                push(fft_length - m, true, coefficient);
            }
        }

        BinKernel { runs, weights }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometric(
        sample_rate: f64,
        min_freq: f64,
        bins: usize,
        bins_per_octave: usize,
        q: f64,
    ) -> (Vec<f64>, Vec<usize>) {
        let frequencies: Vec<f64> = (0..bins)
            .map(|k| min_freq * 2f64.powf(k as f64 / bins_per_octave as f64))
            .collect();
        let lengths: Vec<usize> = frequencies
            .iter()
            .map(|f| (q * sample_rate / f).round() as usize)
            .collect();
        (frequencies, lengths)
    }

    #[test]
    fn kernel_is_sparse_and_run_encoded() {
        let (frequencies, lengths) = geometric(22_050.0, 55.0, 60, 12, 16.8);
        let fft_length = lengths[0].next_power_of_two();
        let kernel = Kernel::build(22_050.0, &frequencies, &lengths, fft_length, 0.01);
        let dense = kernel.num_bins() * (fft_length / 2 + 1);
        assert!(kernel.num_nonzeros() * 10 < dense);
        assert!(kernel.runs.len() < kernel.num_nonzeros() / 4);
        assert_eq!(kernel.lengths(), lengths.as_slice());
        assert_eq!(kernel.frequencies().len(), 60);
        for run in &kernel.runs {
            assert!(run.start as usize + run.len as usize <= fft_length / 2 + 1);
        }
    }

    #[test]
    fn pruning_keeps_coefficients_tied_at_the_boundary() {
        // A periodic two-sample Hann window is [0, 1]: the atom has two
        // equal FFT magnitudes. A 50 % budget may drop one of them, but
        // dropping "everything at or below" the first would drop both.
        let kernel = Kernel::build(8_000.0, &[100.0], &[2], 2, 0.5);
        assert_eq!(kernel.num_nonzeros(), 2);
        // Three equal coefficients and a budget for one: none is dropped.
        let mut magnitudes = Vec::new();
        let atom = [Complex::new(2.0, 0.0); 3];
        assert!(sparsity_threshold(&atom, &mut magnitudes, 0.34) < 0.0);
        // Budget for two of them: still none, the run is indivisible.
        assert!(sparsity_threshold(&atom, &mut magnitudes, 0.67) < 0.0);
        // Budget for all three: all dropped.
        assert_eq!(sparsity_threshold(&atom, &mut magnitudes, 1.0), 2.0);
        // Mixed magnitudes: the run of ones fits, the run of twos does not.
        let atom = [
            Complex::new(1.0, 0.0),
            Complex::new(2.0, 0.0),
            Complex::new(1.0, 0.0),
            Complex::new(2.0, 0.0),
        ];
        assert_eq!(sparsity_threshold(&atom, &mut magnitudes, 0.5), 1.0);
    }

    #[test]
    fn zero_sparsity_keeps_all_coefficients() {
        let (frequencies, lengths) = geometric(8_000.0, 100.0, 12, 12, 16.8);
        let fft_length = lengths[0].next_power_of_two();
        let kernel = Kernel::build(8_000.0, &frequencies, &lengths, fft_length, 0.0);
        assert_eq!(kernel.num_nonzeros(), kernel.num_bins() * fft_length);
    }
}
