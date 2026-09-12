//! Decimation cascade shared by the batch and streaming paths.
//!
//! Level 0 holds the input signal; level `d` holds the signal low-passed and
//! decimated by `2^d`. Every level is stored with an absolute index origin
//! so that a frame centred on full-rate sample `t` reads level `d` around
//! index `t >> d`. Decimation is zero-phase: output `n` of level `d + 1` is
//! the filter centred on input `2n` of level `d`, so no delay compensation
//! is needed and the batch and streaming paths produce identical samples.

use crate::filter::HalfBand;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[derive(Debug, Clone)]
struct Level {
    samples: Vec<f32>,
    /// Absolute index of `samples[0]` at this level's rate.
    origin: i64,
    /// `origin` of a freshly created level.
    initial_origin: i64,
}

impl Level {
    fn end(&self) -> i64 {
        self.origin + self.samples.len() as i64
    }
}

/// Outputs per block when decimating.
const BLOCK: usize = 1_024;

#[derive(Debug, Clone)]
pub(crate) struct Levels {
    filter: Option<HalfBand>,
    levels: Vec<Level>,
    scratch: Vec<f32>,
}

impl Levels {
    /// Creates `num_levels` empty levels. The signal is assumed to be zero
    /// before absolute index 0.
    pub fn new(filter: Option<HalfBand>, num_levels: usize) -> Self {
        let taps = filter.as_ref().map_or(1, HalfBand::taps);
        let delay = (taps - 1) as i64 / 2;
        let mut levels = Vec::with_capacity(num_levels);
        // `first` is the first index of a level that can be non-zero; the
        // `taps - 1` samples before it are known zeros kept as filter
        // history so every read stays inside the buffer.
        let mut first: i64 = 0;
        for _ in 0..num_levels {
            let origin = first - (taps as i64 - 1);
            levels.push(Level {
                samples: vec![0.0; taps - 1],
                origin,
                initial_origin: origin,
            });
            first = (first - delay).div_euclid(2) + i64::from((first - delay).rem_euclid(2) != 0);
        }
        Self {
            filter,
            levels,
            scratch: Vec::new(),
        }
    }

    /// Returns every level to its initial state while keeping the allocated
    /// capacity.
    pub fn clear(&mut self) {
        let taps = self.filter.as_ref().map_or(1, HalfBand::taps);
        for level in &mut self.levels {
            level.samples.clear();
            level.samples.resize(taps - 1, 0.0);
            level.origin = level.initial_origin;
        }
    }

    /// Exclusive absolute end index of the samples available at `level`.
    pub fn end(&self, level: usize) -> i64 {
        self.levels[level].end()
    }

    /// Appends full-rate samples.
    pub fn extend(&mut self, samples: &[f32]) {
        self.levels[0].samples.extend_from_slice(samples);
    }

    /// Appends `count` zero samples at the full rate.
    pub fn extend_zeros(&mut self, count: usize) {
        let level = &mut self.levels[0];
        level.samples.resize(level.samples.len() + count, 0.0);
    }

    /// Reserves room for `additional` full-rate samples.
    pub fn reserve(&mut self, additional: usize) {
        self.levels[0].samples.reserve(additional);
    }

    /// Produces every decimated sample the lower levels currently allow.
    pub fn propagate(&mut self) {
        let Some(filter) = &self.filter else {
            return;
        };
        let delay = filter.delay() as i64;
        for d in 1..self.levels.len() {
            let (lower, upper) = self.levels.split_at_mut(d);
            let input = &lower[d - 1];
            let output = &mut upper[0];
            // Output n needs input indices 2n - delay ..= 2n + delay.
            let last_input = input.end() - 1;
            let last_output = (last_input - delay).div_euclid(2);
            let start = output.end();
            if last_output < start {
                continue;
            }
            let count = (last_output - start + 1) as usize;
            let base = output.samples.len();
            output.samples.resize(base + count, 0.0);
            let first_centre = (2 * start - input.origin) as usize;
            decimate(
                filter,
                &input.samples,
                first_centre,
                &mut output.samples[base..],
                &mut self.scratch,
            );
        }
    }

    /// Copies `out.len()` samples of `level` centred on absolute index
    /// `centre`, zero filling outside the available range.
    pub fn frame(&self, level: usize, centre: i64, out: &mut [f32]) {
        let level = &self.levels[level];
        let half = out.len() as i64 / 2;
        let start = centre - half;
        let end = start + out.len() as i64;
        let src_start = start.max(level.origin);
        let src_end = end.min(level.end());
        out.fill(0.0);
        if src_start < src_end {
            let dst = (src_start - start) as usize;
            let src = (src_start - level.origin) as usize;
            let len = (src_end - src_start) as usize;
            out[dst..dst + len].copy_from_slice(&level.samples[src..src + len]);
        }
    }

    /// Drops samples of `level` before absolute index `keep_from`.
    pub fn discard_before(&mut self, level: usize, keep_from: i64) {
        let level = &mut self.levels[level];
        let drop = (keep_from - level.origin).clamp(0, level.samples.len() as i64) as usize;
        if drop > 0 {
            level.samples.drain(..drop);
            level.origin += drop as i64;
        }
    }

    /// Number of samples currently retained at `level`.
    pub fn retained(&self, level: usize) -> usize {
        self.levels[level].samples.len()
    }
}

/// Fills `out` with consecutive decimated samples, output `i` centred on
/// `input[first_centre + 2 * i]`, in blocks that are spread over the rayon
/// pool when there are enough of them.
#[cfg(feature = "parallel")]
fn decimate(
    filter: &HalfBand,
    input: &[f32],
    first_centre: usize,
    out: &mut [f32],
    scratch: &mut Vec<f32>,
) {
    if out.len() < 4 * BLOCK {
        for (b, block) in out.chunks_mut(BLOCK).enumerate() {
            filter.decimate_block(input, first_centre + 2 * b * BLOCK, block, scratch);
        }
        return;
    }
    out.par_chunks_mut(BLOCK)
        .enumerate()
        .for_each_init(Vec::new, |odd, (b, block)| {
            filter.decimate_block(input, first_centre + 2 * b * BLOCK, block, odd);
        });
}

#[cfg(not(feature = "parallel"))]
fn decimate(
    filter: &HalfBand,
    input: &[f32],
    first_centre: usize,
    out: &mut [f32],
    scratch: &mut Vec<f32>,
) {
    for (b, block) in out.chunks_mut(BLOCK).enumerate() {
        filter.decimate_block(input, first_centre + 2 * b * BLOCK, block, scratch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_and_bulk_propagation_agree() {
        let filter = HalfBand::design(0.3, 80.0);
        let signal: Vec<f32> = (0..5_000)
            .map(|i| ((i * 7919) % 1_000) as f32 / 500.0 - 1.0)
            .collect();

        let mut bulk = Levels::new(Some(filter.clone()), 4);
        bulk.extend(&signal);
        bulk.propagate();

        let mut incremental = Levels::new(Some(filter), 4);
        for chunk in signal.chunks(37) {
            incremental.extend(chunk);
            incremental.propagate();
        }

        for level in 0..4 {
            assert_eq!(bulk.end(level), incremental.end(level));
            let len = bulk.retained(level);
            let mut a = vec![0.0; len];
            let mut b = vec![0.0; len];
            let centre = bulk.levels[level].origin + len as i64 / 2;
            bulk.frame(level, centre, &mut a);
            incremental.frame(level, centre, &mut b);
            assert_eq!(a, b, "level {level}");
        }
    }

    #[test]
    fn decimated_levels_track_a_slow_sinusoid() {
        let filter = HalfBand::design(0.3, 80.0);
        let sr = 8_000.0f32;
        let signal: Vec<f32> = (0..16_000)
            .map(|i| (std::f32::consts::TAU * 50.0 * i as f32 / sr).sin())
            .collect();
        let mut levels = Levels::new(Some(filter), 3);
        levels.extend(&signal);
        levels.propagate();
        // Level 2 sample n corresponds to full-rate sample 4n.
        let mut frame = vec![0.0; 1];
        for n in [500i64, 1_000, 2_000] {
            levels.frame(2, n, &mut frame);
            let expected = signal[(4 * n) as usize];
            assert!(
                (frame[0] - expected).abs() < 2e-3,
                "{n}: {} vs {expected}",
                frame[0]
            );
        }
    }

    #[test]
    fn clear_restores_initial_state() {
        let filter = HalfBand::design(0.3, 80.0);
        let signal: Vec<f32> = (0..3_000).map(|i| (i as f32 * 0.37).sin()).collect();
        let mut fresh = Levels::new(Some(filter.clone()), 3);
        fresh.extend(&signal);
        fresh.propagate();

        let mut reused = Levels::new(Some(filter), 3);
        reused.extend(&signal[..1_000]);
        reused.propagate();
        reused.discard_before(0, 500);
        reused.clear();
        reused.extend(&signal);
        reused.propagate();

        for level in 0..3 {
            assert_eq!(fresh.end(level), reused.end(level));
            assert_eq!(fresh.levels[level].samples, reused.levels[level].samples);
        }
    }

    #[test]
    fn discard_keeps_later_samples_addressable() {
        let mut levels = Levels::new(None, 1);
        levels.extend(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        levels.discard_before(0, 3);
        let mut frame = vec![0.0; 4];
        levels.frame(0, 4, &mut frame);
        assert_eq!(frame, [0.0, 4.0, 5.0, 0.0]);
    }
}
