//! Verification of a match hypothesis against the reference peak track.
//!
//! The matcher's votes come from hash collisions, which are cheap but
//! indirect evidence. Given its hypothesis `(song, shift, tempo, offset)`,
//! the peaks of the watched song predict where every query peak should
//! sit, and vice versa: `frame_ref = tempo · frame_query + offset`,
//! `bin_ref = bin_query − shift`. The fraction of query peaks with a
//! reference peak at the predicted place is direct evidence for the
//! hypothesis and is independent of how many hashes each peak spawned,
//! so it can gate a decision made on few hashes (small fan-out) and
//! reject hypotheses that collect votes without aligning, such as a
//! reversed or looped copy of the song.

use crate::peaks::Peak;

/// The peaks of one watched song, sorted by `(frame, bin)`.
#[derive(Debug, Clone)]
pub struct PeakTrack {
    peaks: Vec<Peak>,
}

/// How well a hypothesis aligns the query peaks with the reference peaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Verification {
    /// Query peaks inside the verified span.
    pub query_peaks: u32,
    /// Of those, the ones with a reference peak at the predicted place.
    pub query_matched: u32,
    /// Reference peaks inside the span the query maps onto.
    pub reference_peaks: u32,
    /// Of those, the ones with a query peak at the predicted place.
    pub reference_matched: u32,
}

impl Verification {
    /// Fraction of query peaks explained by the reference, `0..=1`.
    pub fn query_fraction(&self) -> f64 {
        fraction(self.query_matched, self.query_peaks)
    }

    /// Fraction of the reference span's peaks present in the query.
    pub fn reference_fraction(&self) -> f64 {
        fraction(self.reference_matched, self.reference_peaks)
    }
}

fn fraction(part: u32, whole: u32) -> f64 {
    if whole == 0 {
        0.0
    } else {
        f64::from(part) / f64::from(whole)
    }
}

impl PeakTrack {
    /// Builds the track; `peaks` may be in any order.
    pub fn new(mut peaks: Vec<Peak>) -> Self {
        peaks.sort_unstable();
        peaks.dedup();
        Self { peaks }
    }

    /// Number of peaks in the track.
    pub fn len(&self) -> usize {
        self.peaks.len()
    }

    /// Whether the track has no peaks.
    pub fn is_empty(&self) -> bool {
        self.peaks.is_empty()
    }

    /// Scores the hypothesis `(shift, tempo, offset)` on the query peaks
    /// `query`, which must be sorted by `(frame, bin)`. A peak matches
    /// when a peak of the other side lies within `frame_tolerance` frames
    /// and `bin_tolerance` bins of its predicted place.
    pub fn verify(
        &self,
        query: &[Peak],
        shift: i32,
        tempo: f64,
        offset: f64,
        frame_tolerance: u32,
        bin_tolerance: u32,
    ) -> Verification {
        let mut v = Verification::default();
        let (Some(first), Some(last)) = (query.first(), query.last()) else {
            return v;
        };
        if tempo <= 0.0 || !tempo.is_finite() || !offset.is_finite() {
            return v;
        }
        v.query_peaks = query.len() as u32;
        for q in query {
            let frame = tempo * q.frame as f64 + offset;
            let bin = q.bin as i64 - i64::from(shift);
            if near(&self.peaks, frame, bin, frame_tolerance, bin_tolerance) {
                v.query_matched += 1;
            }
        }
        let tolerance = f64::from(frame_tolerance);
        let lo = tempo * first.frame as f64 + offset - tolerance;
        let hi = tempo * last.frame as f64 + offset + tolerance;
        let start = self.peaks.partition_point(|p| (p.frame as f64) < lo);
        for r in self.peaks[start..]
            .iter()
            .take_while(|p| p.frame as f64 <= hi)
        {
            v.reference_peaks += 1;
            let frame = (r.frame as f64 - offset) / tempo;
            let bin = r.bin as i64 + i64::from(shift);
            if near(query, frame, bin, frame_tolerance, bin_tolerance) {
                v.reference_matched += 1;
            }
        }
        v
    }
}

/// Whether `peaks` (sorted by frame) holds a peak within the tolerances of
/// the predicted `(frame, bin)`.
fn near(peaks: &[Peak], frame: f64, bin: i64, frame_tolerance: u32, bin_tolerance: u32) -> bool {
    let lo = frame - f64::from(frame_tolerance);
    let hi = frame + f64::from(frame_tolerance);
    if hi < 0.0 {
        return false;
    }
    let start = peaks.partition_point(|p| (p.frame as f64) < lo);
    peaks[start..]
        .iter()
        .take_while(|p| p.frame as f64 <= hi)
        .any(|p| (i64::from(p.bin) - bin).unsigned_abs() <= u64::from(bin_tolerance))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peaks(seed: u64, frames: u64) -> Vec<Peak> {
        let mut seed = seed;
        let mut out = Vec::new();
        let mut frame = 0u64;
        while frame < frames {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            frame += 3 + (seed % 10);
            out.push(Peak {
                frame,
                bin: 10 + ((seed >> 8) % 140) as u32,
            });
        }
        out
    }

    #[test]
    fn the_true_hypothesis_explains_every_peak_and_a_wrong_one_few() {
        let song = peaks(3, 20_000);
        let track = PeakTrack::new(song.clone());
        // Query: frames 5000..6000 of the song, 4 bins up, 8 % slower
        // (query frames 1.08× longer), starting at query frame 700.
        let query: Vec<Peak> = song
            .iter()
            .filter(|p| p.frame >= 5000 && p.frame < 6000)
            .map(|p| Peak {
                frame: 700 + ((p.frame - 5000) as f64 * 1.08).round() as u64,
                bin: p.bin + 4,
            })
            .collect();
        let tempo = 1.0 / 1.08;
        let offset = 5000.0 - tempo * 700.0;
        let v = track.verify(&query, 4, tempo, offset, 2, 0);
        assert_eq!(v.query_peaks as usize, query.len());
        assert_eq!(v.query_matched, v.query_peaks, "{v:?}");
        assert!(v.reference_peaks >= v.query_peaks);
        assert_eq!(v.reference_matched, v.query_peaks, "{v:?}");
        assert!(v.query_fraction() > 0.99 && v.reference_fraction() > 0.95);

        // Wrong shift, wrong place.
        let wrong = track.verify(&query, 1, 1.0, 12_000.0, 2, 0);
        assert!(wrong.query_fraction() < 0.2, "{wrong:?}");
        assert!(wrong.reference_fraction() < 0.2, "{wrong:?}");

        // Reversed query at the right place: little alignment.
        let n = query.len();
        let last = query[n - 1].frame;
        let mut reversed: Vec<Peak> = query
            .iter()
            .map(|p| Peak {
                frame: last - p.frame + 700,
                bin: p.bin,
            })
            .collect();
        reversed.sort_unstable();
        let back = track.verify(&reversed, 4, tempo, offset, 2, 0);
        assert!(back.query_fraction() < 0.3, "{back:?}");
    }

    #[test]
    fn degenerate_inputs_score_zero() {
        let track = PeakTrack::new(peaks(1, 500));
        assert_eq!(
            track.verify(&[], 0, 1.0, 0.0, 2, 1),
            Verification::default()
        );
        let q = [Peak { frame: 5, bin: 5 }];
        assert_eq!(track.verify(&q, 0, 0.0, 0.0, 2, 1), Verification::default());
        assert_eq!(
            track.verify(&q, 0, 1.0, f64::NAN, 2, 1),
            Verification::default()
        );
        assert_eq!(Verification::default().query_fraction(), 0.0);
        // Predicted before the track starts: nothing matches, no panic.
        let v = track.verify(&q, 0, 1.0, -1_000.0, 2, 1);
        assert_eq!(
            (v.query_peaks, v.query_matched, v.reference_peaks),
            (1, 0, 0)
        );
        assert!(PeakTrack::new(Vec::new()).is_empty());
    }
}
