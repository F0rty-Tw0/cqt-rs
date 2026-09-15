//! The front end shared by the watched songs and the stream: constant-Q
//! magnitudes to decibels, peaks, hashes.

use cqt_rs::{Cqt, CqtError, magnitude_to_db};

use crate::hashes::{Hash, TripletHasher};
use crate::peaks::{Peak, PeakPicker};

/// Parameters of the peak picker and the hasher.
#[derive(Debug, Clone)]
pub struct FingerprintConfig {
    /// Peak window half-width in frames.
    pub time_radius: usize,
    /// Peak window half-width in bins.
    pub bin_radius: usize,
    /// Decibels a peak must stand above the mean of its window.
    pub prominence: f32,
    /// Absolute floor in dB below which nothing is a peak.
    pub floor: f32,
    /// Frames after an anchor in which its triplet partners are taken.
    pub zone: usize,
    /// Partners per anchor stage; an anchor gives up to
    /// `3 · fan_out · fan_out` hashes.
    pub fan_out: usize,
    /// Quantization steps of the triplet time ratio.
    pub ratio_steps: usize,
}

impl Default for FingerprintConfig {
    fn default() -> Self {
        Self {
            time_radius: 24,
            bin_radius: 9,
            prominence: 15.0,
            floor: -80.0,
            zone: 320,
            fan_out: 4,
            ratio_steps: 32,
        }
    }
}

/// Peak picker and hasher chained behind a dB conversion. Frames are
/// pushed one at a time; peaks and hashes come out with the documented
/// delays of the two stages.
#[derive(Debug, Clone)]
pub struct Fingerprinter {
    picker: PeakPicker,
    hasher: TripletHasher,
    db: Vec<f32>,
    frames: u64,
    peaks: u64,
}

impl Fingerprinter {
    /// Creates the front end for frames of `num_bins` magnitudes.
    pub fn new(num_bins: usize, config: &FingerprintConfig) -> Self {
        Self {
            picker: PeakPicker::new(
                num_bins,
                config.time_radius,
                config.bin_radius,
                config.prominence,
                config.floor,
            ),
            hasher: TripletHasher::new(config.zone, config.fan_out, config.ratio_steps),
            db: vec![0.0; num_bins],
            frames: 0,
            peaks: 0,
        }
    }

    /// Worst-case frames between a pushed frame and the hashes anchored on
    /// it.
    pub fn delay_frames(&self) -> u64 {
        (self.picker.delay_frames() + self.hasher.delay_frames()) as u64
    }

    /// Frames pushed so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Peaks found so far.
    pub fn peaks(&self) -> u64 {
        self.peaks
    }

    /// Pushes one frame of linear magnitudes; `on_peak` receives the peaks
    /// that became decidable, `on_hash` the hashes whose anchors are
    /// complete.
    pub fn push<P: FnMut(Peak), H: FnMut(Hash)>(
        &mut self,
        magnitudes: &[f32],
        mut on_peak: P,
        on_hash: H,
    ) {
        self.db.copy_from_slice(magnitudes);
        magnitude_to_db(&mut self.db, 1.0, 1e-5, None);
        let hasher = &mut self.hasher;
        let peaks = &mut self.peaks;
        self.picker.push(&self.db, |p| {
            *peaks += 1;
            on_peak(p);
            hasher.push(p);
        });
        self.frames += 1;
        if let Some(decided) = self
            .frames
            .checked_sub(self.picker.delay_frames() as u64 + 1)
        {
            self.hasher.advance(decided, on_hash);
        }
    }

    /// Emits the peaks and hashes still pending, treating the stream as
    /// finished. Call [`Fingerprinter::reset`] before pushing more.
    pub fn flush<P: FnMut(Peak), H: FnMut(Hash)>(&mut self, mut on_peak: P, mut on_hash: H) {
        let hasher = &mut self.hasher;
        let peaks = &mut self.peaks;
        self.picker.flush(|p| {
            *peaks += 1;
            on_peak(p);
            hasher.push(p);
        });
        self.hasher.flush(&mut on_hash);
    }

    /// Forgets all frames.
    pub fn reset(&mut self) {
        self.picker.reset();
        self.hasher.reset();
        self.frames = 0;
        self.peaks = 0;
    }
}

/// Fingerprints a whole signal with the batch transform, which gives the
/// same peaks and hashes as streaming it: the frame count, the peaks and
/// the hashes.
pub fn fingerprint(
    cqt: &Cqt,
    hop_size: usize,
    samples: &[f32],
    config: &FingerprintConfig,
) -> Result<(u64, Vec<Peak>, Vec<Hash>), CqtError> {
    let magnitudes = cqt.process(samples, hop_size)?;
    let mut fp = Fingerprinter::new(cqt.num_bins(), config);
    let mut peaks = Vec::new();
    let mut hashes = Vec::new();
    for row in magnitudes.rows() {
        let row = row.as_slice().expect("contiguous row");
        fp.push(row, |p| peaks.push(p), |h| hashes.push(h));
    }
    fp.flush(|p| peaks.push(p), |h| hashes.push(h));
    Ok((fp.frames(), peaks, hashes))
}
