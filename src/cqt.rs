//! Batch transform.

use std::sync::Arc;

use ndarray::Array2;
use realfft::{RealFftPlanner, RealToComplex};
use rustfft::num_complex::Complex;
use thiserror::Error;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::kernel::{FRAMES, Kernel, Spectra};
use crate::params::CqtParams;
use crate::plan::{Plan, plan};
use halfband_rs::Levels;

/// Errors produced while transforming a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CqtError {
    /// The hop size is zero.
    #[error("hop size must be at least one sample")]
    InvalidHopSize,
    /// The input signal contains no samples.
    #[error("input signal is empty")]
    EmptySignal,
}

/// One octave (or the whole range in single-rate mode): a kernel and an FFT
/// plan at one sample rate.
struct Group {
    first_bin: usize,
    num_bins: usize,
    level: usize,
    fft_length: usize,
    kernel: Kernel,
    fft: Arc<dyn RealToComplex<f32>>,
}

/// Reusable buffers for repeated batch calls.
///
/// [`Cqt::process`] allocates and page-faults a few megabytes of decimation
/// buffers per call; [`Cqt::process_with`] keeps them between calls. A
/// workspace remembers the parameters of the transform that allocated it;
/// a transform with other parameters rebuilds it on first use.
#[derive(Debug, Clone)]
pub struct CqtWorkspace {
    params: CqtParams,
    levels: Levels,
}

impl CqtWorkspace {
    /// Parameters of the transform this workspace currently belongs to.
    pub fn params(&self) -> &CqtParams {
        &self.params
    }
}

/// Working memory for one frame: a real FFT input, output and scratch
/// buffer per group.
#[derive(Debug, Clone)]
pub(crate) struct Scratch {
    frames: Vec<Vec<f32>>,
    spectra: Vec<Vec<Complex<f32>>>,
    fft_scratch: Vec<Vec<Complex<f32>>>,
}

/// [`Scratch`] plus the spectra of one block of frames per group, for the
/// batch path.
struct BlockScratch {
    scratch: Scratch,
    spectra: Vec<Spectra>,
}

/// Constant-Q transform of real-valued signals.
///
/// ```
/// use cqt_rs::{Cqt, CqtParams};
///
/// let params = CqtParams::new(22_050, 110.0, 4_186.0, 12).unwrap();
/// let cqt = Cqt::new(params);
/// let signal = vec![0.0f32; 22_050];
/// let magnitudes = cqt.process(&signal, 512).unwrap();
/// assert_eq!(magnitudes.dim(), (1 + 22_050 / 512, cqt.num_bins()));
/// ```
#[derive(Clone)]
pub struct Cqt {
    params: CqtParams,
    plan: Plan,
    groups: Arc<[Group]>,
    frequencies: Vec<f32>,
    kernel_lengths: Vec<usize>,
}

impl std::fmt::Debug for Cqt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cqt")
            .field("params", &self.params)
            .field("num_levels", &self.num_levels())
            .field("fft_lengths", &self.fft_lengths())
            .field("num_nonzeros", &self.num_nonzeros())
            .field("latency_samples", &self.latency_samples())
            .finish()
    }
}

impl Cqt {
    /// Builds the spectral kernels, decimation filter and FFT plans for
    /// `params`.
    pub fn new(params: CqtParams) -> Self {
        let plan = plan(&params).expect("parameters were validated by the builder");
        let mut planner = RealFftPlanner::<f32>::new();
        let groups: Vec<Group> = plan
            .groups
            .iter()
            .map(|group| {
                let frequencies: Vec<f64> = (group.first_bin..group.first_bin + group.num_bins)
                    .map(|bin| params.center_freq_f64(bin))
                    .collect();
                let kernel = Kernel::build(
                    group.sample_rate(params.sample_rate()),
                    &frequencies,
                    &group.lengths,
                    group.fft_length,
                    params.sparsity(),
                );
                Group {
                    first_bin: group.first_bin,
                    num_bins: group.num_bins,
                    level: group.level,
                    fft_length: group.fft_length,
                    kernel,
                    fft: planner.plan_fft_forward(group.fft_length),
                }
            })
            .collect();
        let frequencies = (0..params.num_bins())
            .map(|bin| params.center_freq(bin))
            .collect();
        let kernel_lengths = (0..params.num_bins())
            .map(|bin| params.kernel_length(bin))
            .collect();
        Self {
            params,
            plan,
            groups: groups.into(),
            frequencies,
            kernel_lengths,
        }
    }

    /// The parameters this transform was built from.
    pub fn params(&self) -> &CqtParams {
        &self.params
    }

    /// Number of frequency bins per frame.
    pub fn num_bins(&self) -> usize {
        self.params.num_bins()
    }

    /// Centre frequency of each bin in Hz.
    pub fn frequencies(&self) -> &[f32] {
        &self.frequencies
    }

    /// Analysis window length of each bin in full-rate samples.
    pub fn kernel_lengths(&self) -> &[usize] {
        &self.kernel_lengths
    }

    /// Number of sample rates in use: one per octave in multi-rate mode.
    pub fn num_levels(&self) -> usize {
        self.plan.num_levels
    }

    /// FFT length of every group, lowest octave first.
    pub fn fft_lengths(&self) -> Vec<usize> {
        self.groups.iter().map(|group| group.fft_length).collect()
    }

    /// Sparse kernel of every group, lowest octave first.
    pub fn kernels(&self) -> impl Iterator<Item = &Kernel> {
        self.groups.iter().map(|group| &group.kernel)
    }

    /// Total number of retained spectral kernel coefficients.
    pub fn num_nonzeros(&self) -> usize {
        self.groups.iter().map(|g| g.kernel.num_nonzeros()).sum()
    }

    /// Number of taps of the decimation filter, or zero in single-rate mode.
    pub fn decimation_taps(&self) -> usize {
        self.plan.filter.as_ref().map_or(0, |f| f.taps())
    }

    /// Span of every group's frame in full-rate samples, lowest octave
    /// first. A hop larger than the smallest span leaves gaps between the
    /// frames of that octave.
    pub fn frame_spans(&self) -> Vec<usize> {
        self.groups
            .iter()
            .map(|group| group.fft_length << group.level)
            .collect()
    }

    /// Full-rate samples between the centre of a frame and the last sample
    /// it needs, which is the delay of a real-time stream.
    ///
    /// This is exact when the hop is a multiple of `2^(levels - 1)`; other
    /// hops emit some frames up to `2^(levels - 1) - 1` samples earlier.
    pub fn latency_samples(&self) -> usize {
        self.plan.latency
    }

    /// Hop sizes that are a multiple of this value give every frame the
    /// same latency and centre every octave on exactly `i * hop_size`.
    pub fn hop_alignment(&self) -> usize {
        1 << (self.plan.num_levels - 1)
    }

    /// Number of frames [`Cqt::process`] produces for a signal of
    /// `signal_length` samples and the given hop (treated as at least one).
    pub fn num_frames(&self, signal_length: usize, hop_size: usize) -> usize {
        1 + signal_length / hop_size.max(1)
    }

    /// Validates a hop size.
    pub fn check_hop_size(&self, hop_size: usize) -> Result<(), CqtError> {
        if hop_size == 0 {
            return Err(CqtError::InvalidHopSize);
        }
        Ok(())
    }

    pub(crate) fn levels(&self) -> Levels {
        Levels::new(self.plan.filter.clone(), self.plan.num_levels)
    }

    /// Allocates a workspace for [`Cqt::process_with`].
    pub fn workspace(&self) -> CqtWorkspace {
        CqtWorkspace {
            params: self.params.clone(),
            levels: self.levels(),
        }
    }

    /// Makes `workspace` usable with this transform, rebuilding its
    /// decimation state when it was allocated by a different one.
    fn adopt(&self, workspace: &mut CqtWorkspace) {
        if workspace.params != self.params {
            *workspace = self.workspace();
        }
    }

    pub(crate) fn scratch(&self) -> Scratch {
        Scratch {
            frames: self
                .groups
                .iter()
                .map(|g| vec![0.0; g.fft_length])
                .collect(),
            spectra: self
                .groups
                .iter()
                .map(|g| g.fft.make_output_vec())
                .collect(),
            fft_scratch: self
                .groups
                .iter()
                .map(|g| g.fft.make_scratch_vec())
                .collect(),
        }
    }

    fn block_scratch(&self) -> BlockScratch {
        BlockScratch {
            scratch: self.scratch(),
            spectra: self
                .groups
                .iter()
                .map(|g| Spectra::new(&g.kernel))
                .collect(),
        }
    }

    /// Whether the frame centred on full-rate sample `centre` can be
    /// computed from the samples available in `levels`.
    pub(crate) fn frame_ready(&self, levels: &Levels, centre: i64) -> bool {
        self.groups.iter().all(|group| {
            let half = (group.fft_length / 2) as i64;
            levels.end(group.level) >= (centre >> group.level) + half
        })
    }

    /// Absolute index of the earliest sample of `level` still needed to
    /// compute frames from `centre` onwards.
    pub(crate) fn first_needed(&self, level: usize, centre: i64) -> i64 {
        self.groups
            .iter()
            .filter(|group| group.level == level)
            .map(|group| (centre >> group.level) - (group.fft_length / 2) as i64)
            .min()
            .unwrap_or(i64::MAX)
    }

    fn forward(&self, levels: &Levels, scratch: &mut Scratch, centre: i64) {
        for (g, group) in self.groups.iter().enumerate() {
            levels.frame(group.level, centre >> group.level, &mut scratch.frames[g]);
            group
                .fft
                .process_with_scratch(
                    &mut scratch.frames[g],
                    &mut scratch.spectra[g],
                    &mut scratch.fft_scratch[g],
                )
                .expect("scratch buffers are sized for this FFT");
        }
    }

    /// Computes the magnitudes of the frame centred on full-rate sample
    /// `centre` from `levels`.
    pub(crate) fn frame_magnitudes(
        &self,
        levels: &Levels,
        scratch: &mut Scratch,
        centre: i64,
        out: &mut [f32],
    ) {
        self.forward(levels, scratch, centre);
        for (g, group) in self.groups.iter().enumerate() {
            let out = &mut out[group.first_bin..group.first_bin + group.num_bins];
            group.kernel.apply_magnitude(&scratch.spectra[g], out);
        }
    }

    /// Fills `levels` with the fully propagated decimation cascade for
    /// `signal`, zero padded so that every frame up to `num_frames` is
    /// complete.
    fn prepare(&self, levels: &mut Levels, signal: &[f32], hop_size: usize, num_frames: usize) {
        let last_centre = (num_frames - 1) * hop_size;
        let needed = last_centre + self.plan.latency + 1;
        levels.clear();
        levels.reserve(needed.max(signal.len()));
        levels.extend(signal);
        if needed > signal.len() {
            levels.extend_zeros(needed - signal.len());
        }
        levels.propagate();
    }

    /// Computes the magnitude spectrogram of `signal`.
    ///
    /// Frame `i` is centred on sample `i * hop_size`; the signal is zero
    /// padded so that `1 + len / hop_size` frames are produced. The result
    /// has shape `(frames, bins)`.
    pub fn process(&self, signal: &[f32], hop_size: usize) -> Result<Array2<f32>, CqtError> {
        self.process_with(&mut self.workspace(), signal, hop_size)
    }

    /// Like [`Cqt::process`] but returns complex coefficients.
    pub fn process_complex(
        &self,
        signal: &[f32],
        hop_size: usize,
    ) -> Result<Array2<Complex<f32>>, CqtError> {
        self.process_complex_with(&mut self.workspace(), signal, hop_size)
    }

    /// [`Cqt::process`] with caller-owned buffers, for repeated calls. A
    /// workspace allocated by a transform with different parameters is
    /// rebuilt first.
    pub fn process_with(
        &self,
        workspace: &mut CqtWorkspace,
        signal: &[f32],
        hop_size: usize,
    ) -> Result<Array2<f32>, CqtError> {
        if signal.is_empty() {
            return Err(CqtError::EmptySignal);
        }
        self.check_hop_size(hop_size)?;
        let num_frames = self.num_frames(signal.len(), hop_size);
        let num_bins = self.num_bins();
        self.adopt(workspace);
        self.prepare(&mut workspace.levels, signal, hop_size, num_frames);
        let levels = &workspace.levels;
        let mut output = vec![0.0f32; num_frames * num_bins];
        self.fill_rows(levels, hop_size, &mut output, Kernel::apply_magnitude_block);
        Ok(Array2::from_shape_vec((num_frames, num_bins), output)
            .expect("output buffer matches the frame grid"))
    }

    /// [`Cqt::process_complex`] with caller-owned buffers, see
    /// [`Cqt::process_with`].
    pub fn process_complex_with(
        &self,
        workspace: &mut CqtWorkspace,
        signal: &[f32],
        hop_size: usize,
    ) -> Result<Array2<Complex<f32>>, CqtError> {
        if signal.is_empty() {
            return Err(CqtError::EmptySignal);
        }
        self.check_hop_size(hop_size)?;
        let num_frames = self.num_frames(signal.len(), hop_size);
        let num_bins = self.num_bins();
        self.adopt(workspace);
        self.prepare(&mut workspace.levels, signal, hop_size, num_frames);
        let levels = &workspace.levels;
        let mut output = vec![Complex::default(); num_frames * num_bins];
        self.fill_rows(levels, hop_size, &mut output, Kernel::apply_block);
        Ok(Array2::from_shape_vec((num_frames, num_bins), output)
            .expect("output buffer matches the frame grid"))
    }

    /// Computes the frames whose rows are `rows`, the first centred on
    /// frame `first * hop_size`: every group runs its FFTs over the block
    /// and then applies its kernel to all of them at once.
    fn fill_block<T, F>(
        &self,
        levels: &Levels,
        scratch: &mut BlockScratch,
        first: usize,
        hop_size: usize,
        rows: &mut [T],
        apply: &F,
    ) where
        F: Fn(&Kernel, &Spectra, &mut [T], usize),
    {
        let num_bins = self.num_bins();
        let frames = rows.len() / num_bins;
        for (g, group) in self.groups.iter().enumerate() {
            let spectra = &mut scratch.spectra[g];
            spectra.clear();
            for frame in first..first + frames {
                let centre = (frame * hop_size) as i64;
                let fft = &mut scratch.scratch;
                levels.frame(group.level, centre >> group.level, &mut fft.frames[g]);
                group
                    .fft
                    .process_with_scratch(
                        &mut fft.frames[g],
                        &mut fft.spectra[g],
                        &mut fft.fft_scratch[g],
                    )
                    .expect("scratch buffers are sized for this FFT");
                spectra.push(&fft.spectra[g]);
            }
            apply(
                &group.kernel,
                spectra,
                &mut rows[group.first_bin..],
                num_bins,
            );
        }
    }

    #[cfg(feature = "parallel")]
    fn fill_rows<T, F>(&self, levels: &Levels, hop_size: usize, output: &mut [T], apply: F)
    where
        T: Send,
        F: Fn(&Kernel, &Spectra, &mut [T], usize) + Sync,
    {
        let block = FRAMES * self.num_bins();
        output.par_chunks_mut(block).enumerate().for_each_init(
            || self.block_scratch(),
            |scratch, (b, rows)| {
                self.fill_block(levels, scratch, b * FRAMES, hop_size, rows, &apply);
            },
        );
    }

    #[cfg(not(feature = "parallel"))]
    fn fill_rows<T, F>(&self, levels: &Levels, hop_size: usize, output: &mut [T], apply: F)
    where
        F: Fn(&Kernel, &Spectra, &mut [T], usize),
    {
        let block = FRAMES * self.num_bins();
        let mut scratch = self.block_scratch();
        for (b, rows) in output.chunks_mut(block).enumerate() {
            self.fill_block(levels, &mut scratch, b * FRAMES, hop_size, rows, &apply);
        }
    }
}
