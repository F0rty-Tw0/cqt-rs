//! Streaming spectral peak picker with a prominence threshold.

/// A spectral peak: a local maximum of the dB spectrogram.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Peak {
    /// Frame index (hop units from the start of the stream).
    pub frame: u64,
    /// Constant-Q bin index.
    pub bin: u32,
}

/// Picks the local maxima of a dB spectrogram that stand `prominence` dB
/// above the mean of their `±time_radius × ±bin_radius` neighbourhood.
///
/// Frames are pushed one at a time; the peaks of frame `n` are decided once
/// frame `n + time_radius` is known, so the picker has a delay of
/// `time_radius` frames. Edges are handled by replicating the first and
/// last frame and the lowest and highest bin, which matches SciPy's
/// `maximum_filter`/`uniform_filter` with `mode="nearest"` and makes the
/// batch and streaming results identical.
#[derive(Debug, Clone)]
pub struct PeakPicker {
    num_bins: usize,
    time_radius: usize,
    bin_radius: usize,
    prominence: f32,
    floor: f32,
    slots: usize,
    values: Vec<f32>,
    band_max: Vec<f32>,
    band_sum: Vec<f64>,
    time_sum: Vec<f64>,
    prefix: Vec<f64>,
    inserted: u64,
    last: Vec<f32>,
}

impl PeakPicker {
    /// Creates a picker for spectrograms with `num_bins` bins.
    ///
    /// `floor` is an absolute level in dB below which no peak is reported,
    /// a safety net for digital silence; the prominence rule does the real
    /// work.
    pub fn new(
        num_bins: usize,
        time_radius: usize,
        bin_radius: usize,
        prominence: f32,
        floor: f32,
    ) -> Self {
        let slots = 2 * time_radius + 1;
        Self {
            num_bins,
            time_radius,
            bin_radius,
            prominence,
            floor,
            slots,
            values: vec![0.0; slots * num_bins],
            band_max: vec![f32::NEG_INFINITY; slots * num_bins],
            band_sum: vec![0.0; slots * num_bins],
            time_sum: vec![0.0; num_bins],
            prefix: vec![0.0; num_bins + 1],
            inserted: 0,
            last: vec![0.0; num_bins],
        }
    }

    /// Delay between a frame being pushed and its peaks being emitted.
    pub fn delay_frames(&self) -> usize {
        self.time_radius
    }

    /// Number of bins per frame.
    pub fn num_bins(&self) -> usize {
        self.num_bins
    }

    /// Forgets all frames.
    pub fn reset(&mut self) {
        self.inserted = 0;
        self.time_sum.iter_mut().for_each(|v| *v = 0.0);
        self.band_sum.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Pushes the dB values of the next frame and calls `on_peak` for every
    /// peak of the frame that became decidable, in increasing bin order.
    pub fn push<F: FnMut(Peak)>(&mut self, frame: &[f32], mut on_peak: F) {
        assert_eq!(
            frame.len(),
            self.num_bins,
            "frame has the wrong number of bins"
        );
        if self.inserted == 0 {
            // Replicate the first frame so that the first real frame sits
            // at the centre of a full window.
            for _ in 0..self.time_radius {
                self.insert(frame, &mut on_peak);
            }
        }
        self.insert(frame, &mut on_peak);
        self.last.copy_from_slice(frame);
    }

    /// Emits the peaks of the last `time_radius` frames by replicating the
    /// final frame. Call [`PeakPicker::reset`] before pushing more audio.
    pub fn flush<F: FnMut(Peak)>(&mut self, mut on_peak: F) {
        if self.inserted == 0 {
            return;
        }
        let last = self.last.clone();
        for _ in 0..self.time_radius {
            self.insert(&last, &mut on_peak);
        }
    }

    fn insert<F: FnMut(Peak)>(&mut self, frame: &[f32], on_peak: &mut F) {
        let n = self.num_bins;
        let slot = (self.inserted % self.slots as u64) as usize;
        let row = slot * n;
        // Prefix sums for the band means.
        self.prefix[0] = 0.0;
        for (k, &v) in frame.iter().enumerate() {
            self.prefix[k + 1] = self.prefix[k] + f64::from(v);
        }
        let b = self.bin_radius as isize;
        let last = n as isize - 1;
        for k in 0..n {
            let lo = k as isize - b;
            let hi = k as isize + b;
            let lo_c = lo.max(0) as usize;
            let hi_c = hi.min(last) as usize;
            let mut sum = self.prefix[hi_c + 1] - self.prefix[lo_c];
            if lo < 0 {
                sum += f64::from(frame[0]) * (-lo) as f64;
            }
            if hi > last {
                sum += f64::from(frame[n - 1]) * (hi - last) as f64;
            }
            let max = frame[lo_c..=hi_c]
                .iter()
                .fold(f32::NEG_INFINITY, |m, &v| m.max(v));
            let old = self.band_sum[row + k];
            self.time_sum[k] += sum - old;
            self.band_sum[row + k] = sum;
            self.band_max[row + k] = max;
            self.values[row + k] = frame[k];
        }
        self.inserted += 1;
        if self.inserted < self.slots as u64 {
            return;
        }
        // The centre of the window is the frame inserted `time_radius` ago;
        // virtual index = inserted - 1 - time_radius, real index = virtual
        // - time_radius because the first frame was replicated.
        let virtual_centre = self.inserted - 1 - self.time_radius as u64;
        let Some(real) = virtual_centre.checked_sub(self.time_radius as u64) else {
            return;
        };
        let centre_slot = (virtual_centre % self.slots as u64) as usize;
        let centre_row = centre_slot * n;
        let area = (self.slots * (2 * self.bin_radius + 1)) as f64;
        for k in 0..n {
            let v = self.values[centre_row + k];
            if v < self.floor {
                continue;
            }
            let mean = self.time_sum[k] / area;
            if f64::from(v) < mean + f64::from(self.prominence) {
                continue;
            }
            let mut is_max = true;
            for s in 0..self.slots {
                if self.band_max[s * n + k] > v {
                    is_max = false;
                    break;
                }
            }
            if is_max {
                on_peak(Peak {
                    frame: real,
                    bin: k as u32,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brute-force reference with edge replication.
    fn reference(db: &[Vec<f32>], t: usize, b: usize, prominence: f32, floor: f32) -> Vec<Peak> {
        let frames = db.len() as isize;
        let bins = db[0].len() as isize;
        let at =
            |f: isize, k: isize| db[f.clamp(0, frames - 1) as usize][k.clamp(0, bins - 1) as usize];
        let mut out = Vec::new();
        for f in 0..frames {
            for k in 0..bins {
                let v = at(f, k);
                let mut max = f32::NEG_INFINITY;
                let mut sum = 0.0f64;
                for df in -(t as isize)..=(t as isize) {
                    for dk in -(b as isize)..=(b as isize) {
                        let w = at(f + df, k + dk);
                        max = max.max(w);
                        sum += f64::from(w);
                    }
                }
                let mean = sum / ((2 * t + 1) * (2 * b + 1)) as f64;
                if v == max && f64::from(v) >= mean + f64::from(prominence) && v >= floor {
                    out.push(Peak {
                        frame: f as u64,
                        bin: k as u32,
                    });
                }
            }
        }
        out
    }

    fn pseudo_random(seed: &mut u64) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        (*seed >> 40) as f32 / (1u64 << 24) as f32
    }

    #[test]
    fn matches_brute_force_including_edges() {
        let (frames, bins, t, b) = (120, 40, 5, 3);
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut db: Vec<Vec<f32>> = (0..frames)
            .map(|_| {
                (0..bins)
                    .map(|_| -60.0 + 20.0 * pseudo_random(&mut seed))
                    .collect()
            })
            .collect();
        // Plant some clear peaks, including at the edges.
        for &(f, k, v) in &[
            (0usize, 0usize, -10.0f32),
            (3, 39, -5.0),
            (60, 20, -8.0),
            (119, 10, -12.0),
            (100, 5, -20.0),
        ] {
            db[f][k] = v;
        }
        let expected = reference(&db, t, b, 12.0, -55.0);
        let mut picker = PeakPicker::new(bins, t, b, 12.0, -55.0);
        let mut got = Vec::new();
        for frame in &db {
            picker.push(frame, |p| got.push(p));
        }
        picker.flush(|p| got.push(p));
        assert!(!expected.is_empty());
        assert_eq!(got, expected);
    }

    #[test]
    fn streaming_is_independent_of_push_order() {
        let (frames, bins) = (50, 12);
        let mut seed = 42u64;
        let db: Vec<Vec<f32>> = (0..frames)
            .map(|_| {
                (0..bins)
                    .map(|_| -40.0 + 30.0 * pseudo_random(&mut seed))
                    .collect()
            })
            .collect();
        let expected = reference(&db, 4, 2, 3.0, -100.0);
        let mut picker = PeakPicker::new(bins, 4, 2, 3.0, -100.0);
        let mut got = Vec::new();
        for frame in &db {
            picker.push(frame, |p| got.push(p));
        }
        picker.flush(|p| got.push(p));
        assert_eq!(got, expected);
        assert_eq!(picker.delay_frames(), 4);
    }
}
