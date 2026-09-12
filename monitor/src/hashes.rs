//! Pitch- and tempo-invariant triplet hashes from peaks.

use std::collections::VecDeque;

use crate::peaks::Peak;

/// Packed hash key: two bin differences and a quantized time ratio.
pub type HashKey = u32;

const KEY_OFFSET: i32 = 512;
const RATIO_OFFSET: i32 = 8;

/// Packs `(Δbin12, Δbin23, ratio_step)` into a key. Adding a small offset to
/// any field of a packed key moves it to the neighbouring key, which is how
/// tolerant lookups probe.
pub fn encode_key(d12: i32, d23: i32, ratio: i32) -> HashKey {
    debug_assert!((-KEY_OFFSET..KEY_OFFSET).contains(&d12));
    debug_assert!((-KEY_OFFSET..KEY_OFFSET).contains(&d23));
    debug_assert!((-RATIO_OFFSET..1024 - RATIO_OFFSET).contains(&ratio));
    (((d12 + KEY_OFFSET) as u32) << 20)
        | (((d23 + KEY_OFFSET) as u32) << 10)
        | (ratio + RATIO_OFFSET) as u32
}

/// Inverse of [`encode_key`].
pub fn decode_key(key: HashKey) -> (i32, i32, i32) {
    (
        (key >> 20) as i32 - KEY_OFFSET,
        ((key >> 10) & 0x3ff) as i32 - KEY_OFFSET,
        (key & 0x3ff) as i32 - RATIO_OFFSET,
    )
}

/// One fingerprint hash: the key plus where its anchor peak sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hash {
    /// Packed key, see [`encode_key`].
    pub key: HashKey,
    /// Frame of the anchor (first) peak.
    pub frame: u64,
    /// Bin of the anchor peak.
    pub bin: u32,
    /// Frames from the first to the third peak; the ratio of two matched
    /// spans is the tempo factor between them.
    pub span: u32,
}

/// Builds triplet hashes from a stream of peaks.
///
/// For an anchor peak `p1` the candidates are the next `3·fan_out` peaks
/// within `zone` frames (ordered by frame, then bin). Each candidate `p2`
/// is combined with the following `fan_out` candidates `p3`, giving the key
/// `(b2 − b1, b3 − b2, round(ratio_steps · (t2 − t1) / (t3 − t1)))`. Bin
/// differences survive pitch shifts and the time ratio survives tempo
/// changes. An anchor's hashes are emitted once every peak within its
/// zone is known, i.e. `zone` frames after the anchor, which is the delay
/// of this stage.
#[derive(Debug, Clone)]
pub struct TripletHasher {
    zone: u64,
    fan_out: usize,
    ratio_steps: f64,
    pending: VecDeque<Peak>,
    known_frame: Option<u64>,
}

impl TripletHasher {
    /// Creates a hasher; `zone` in frames.
    pub fn new(zone: usize, fan_out: usize, ratio_steps: usize) -> Self {
        Self {
            zone: zone as u64,
            fan_out,
            ratio_steps: ratio_steps as f64,
            pending: VecDeque::new(),
            known_frame: None,
        }
    }

    /// Delay in frames between an anchor peak and its hashes.
    pub fn delay_frames(&self) -> usize {
        self.zone as usize
    }

    /// Forgets all peaks.
    pub fn reset(&mut self) {
        self.pending.clear();
        self.known_frame = None;
    }

    /// Adds a peak. Peaks must arrive in increasing `(frame, bin)` order.
    pub fn push(&mut self, peak: Peak) {
        if let Some(last) = self.pending.back() {
            debug_assert!(
                (peak.frame, peak.bin) > (last.frame, last.bin),
                "peaks out of order"
            );
        }
        self.pending.push_back(peak);
    }

    /// Declares that every peak up to and including `frame` has been pushed
    /// and emits the hashes of the anchors whose zone is now complete.
    pub fn advance<F: FnMut(Hash)>(&mut self, frame: u64, mut on_hash: F) {
        self.known_frame = Some(frame);
        while let Some(anchor) = self.pending.front().copied() {
            if anchor.frame + self.zone > frame {
                break;
            }
            self.emit(anchor, &mut on_hash);
            self.pending.pop_front();
        }
    }

    /// Emits the hashes of every remaining anchor, treating the stream as
    /// finished. Call [`TripletHasher::reset`] before reusing the hasher.
    pub fn flush<F: FnMut(Hash)>(&mut self, mut on_hash: F) {
        while let Some(anchor) = self.pending.front().copied() {
            self.emit(anchor, &mut on_hash);
            self.pending.pop_front();
        }
    }

    fn emit<F: FnMut(Hash)>(&self, anchor: Peak, on_hash: &mut F) {
        let limit = anchor.frame + self.zone;
        let candidates: Vec<Peak> = self
            .pending
            .iter()
            .skip(1)
            .take_while(|p| p.frame <= limit)
            .take(3 * self.fan_out)
            .copied()
            .collect();
        let t1 = anchor.frame as f64;
        for (a, p2) in candidates.iter().enumerate() {
            if p2.frame == anchor.frame {
                continue;
            }
            for p3 in candidates.iter().skip(a + 1).take(self.fan_out) {
                if p3.frame == p2.frame {
                    continue;
                }
                let ratio = (p2.frame as f64 - t1) / (p3.frame as f64 - t1);
                let step = (ratio * self.ratio_steps).round_ties_even() as i32;
                let key = encode_key(
                    p2.bin as i32 - anchor.bin as i32,
                    p3.bin as i32 - p2.bin as i32,
                    step,
                );
                on_hash(Hash {
                    key,
                    frame: anchor.frame,
                    bin: anchor.bin,
                    span: (p3.frame - anchor.frame) as u32,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_force(peaks: &[Peak], zone: u64, fan_out: usize, steps: f64) -> Vec<Hash> {
        let mut out = Vec::new();
        for (i, p1) in peaks.iter().enumerate() {
            let cand: Vec<Peak> = peaks[i + 1..]
                .iter()
                .take_while(|p| p.frame <= p1.frame + zone)
                .take(3 * fan_out)
                .copied()
                .collect();
            for a in 0..cand.len() {
                let p2 = cand[a];
                if p2.frame == p1.frame {
                    continue;
                }
                for &p3 in cand.iter().skip(a + 1).take(fan_out) {
                    if p3.frame == p2.frame {
                        continue;
                    }
                    let ratio = (p2.frame - p1.frame) as f64 / (p3.frame - p1.frame) as f64;
                    out.push(Hash {
                        key: encode_key(
                            p2.bin as i32 - p1.bin as i32,
                            p3.bin as i32 - p2.bin as i32,
                            (ratio * steps).round_ties_even() as i32,
                        ),
                        frame: p1.frame,
                        bin: p1.bin,
                        span: (p3.frame - p1.frame) as u32,
                    });
                }
            }
        }
        out
    }

    fn peaks() -> Vec<Peak> {
        let mut seed = 7u64;
        let mut peaks = Vec::new();
        let mut frame = 0u64;
        while frame < 2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            frame += 1 + (seed % 9);
            let n = 1 + (seed >> 8) % 3;
            let mut bins: Vec<u32> = (0..n)
                .map(|i| ((seed >> (12 + 5 * i)) % 169) as u32)
                .collect();
            bins.sort_unstable();
            bins.dedup();
            for bin in bins {
                peaks.push(Peak { frame, bin });
            }
        }
        peaks
    }

    #[test]
    fn key_round_trips() {
        for &(a, b, r) in &[
            (0, 0, 0),
            (-168, 168, 32),
            (5, -7, 16),
            (-511, -511, -8),
            (511, 511, 33),
        ] {
            assert_eq!(decode_key(encode_key(a, b, r)), (a, b, r));
        }
        // Neighbouring keys differ by a field step.
        assert_eq!(encode_key(3, 4, 5) + 1, encode_key(3, 4, 6));
        assert_eq!(encode_key(3, 4, 5) + (1 << 10), encode_key(3, 5, 5));
        assert_eq!(encode_key(3, 4, 5) - (1 << 20), encode_key(2, 4, 5));
    }

    #[test]
    fn streaming_matches_brute_force() {
        let peaks = peaks();
        let expected = brute_force(&peaks, 320, 6, 32.0);
        let mut hasher = TripletHasher::new(320, 6, 32);
        let mut got = Vec::new();
        // Feed frame by frame, advancing after each frame's peaks.
        let mut i = 0;
        for frame in 0..=peaks.last().unwrap().frame {
            while i < peaks.len() && peaks[i].frame == frame {
                hasher.push(peaks[i]);
                i += 1;
            }
            hasher.advance(frame, |h| got.push(h));
        }
        hasher.flush(|h| got.push(h));
        assert_eq!(got.len(), expected.len());
        assert_eq!(got, expected);
        assert!(got.len() > 1000);
    }

    #[test]
    fn hashes_are_emitted_zone_frames_after_the_anchor() {
        let mut hasher = TripletHasher::new(10, 2, 32);
        hasher.push(Peak { frame: 0, bin: 5 });
        hasher.push(Peak { frame: 3, bin: 9 });
        hasher.push(Peak { frame: 8, bin: 1 });
        let mut n = 0;
        hasher.advance(9, |_| n += 1);
        assert_eq!(n, 0);
        hasher.advance(10, |_| n += 1);
        assert_eq!(n, 1);
    }
}
