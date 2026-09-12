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
/// changes.
///
/// An anchor's hashes depend only on its candidate list, so they are
/// emitted as soon as that list is determined: when `3·fan_out` later
/// peaks inside the zone are known (any later peak would not be a
/// candidate) or, in sparse passages, when the zone has passed. Anchors
/// are released in order, and the hash sequence is identical to the one
/// obtained by waiting for the whole zone. The worst-case delay of this
/// stage is `zone` frames; at twenty peaks per second and the default
/// fan-out the median is about half that.
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

    /// Worst-case delay in frames between an anchor peak and its hashes.
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
    /// and emits the hashes of the anchors whose candidate list is now
    /// determined, oldest first.
    pub fn advance<F: FnMut(Hash)>(&mut self, frame: u64, mut on_hash: F) {
        self.known_frame = Some(frame);
        while let Some(anchor) = self.pending.front().copied() {
            if anchor.frame + self.zone > frame && !self.candidates_complete(anchor) {
                break;
            }
            self.emit(anchor, &mut on_hash);
            self.pending.pop_front();
        }
    }

    /// Whether the front anchor already has its full `3·fan_out`
    /// candidates. Peaks arrive in order, so once the last candidate slot
    /// is filled by a peak inside the zone no later peak can be a
    /// candidate.
    fn candidates_complete(&self, anchor: Peak) -> bool {
        self.pending
            .get(3 * self.fan_out)
            .is_some_and(|p| p.frame <= anchor.frame + self.zone)
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

    /// Feeds `peaks` with watermark steps drawn from `seed` and returns
    /// the emitted hashes in order.
    fn stream(peaks: &[Peak], zone: usize, fan_out: usize, seed: u64) -> Vec<Hash> {
        let mut hasher = TripletHasher::new(zone, fan_out, 32);
        let mut got = Vec::new();
        let mut seed = seed;
        let mut i = 0;
        let mut frame = 0;
        let last = peaks.last().map_or(0, |p| p.frame);
        while frame <= last {
            while i < peaks.len() && peaks[i].frame <= frame {
                hasher.push(peaks[i]);
                i += 1;
            }
            hasher.advance(frame, |h| got.push(h));
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            frame += 1 + seed % 40;
        }
        for p in &peaks[i..] {
            hasher.push(*p);
        }
        hasher.flush(|h| got.push(h));
        got
    }

    #[test]
    fn streaming_matches_brute_force() {
        let peaks = peaks();
        let expected = brute_force(&peaks, 320, 6, 32.0);
        let got = stream(&peaks, 320, 6, 1);
        assert_eq!(got.len(), expected.len());
        assert_eq!(got, expected);
        assert!(got.len() > 1000);
    }

    #[test]
    fn streaming_matches_brute_force_for_any_watermark_and_density() {
        let dense = peaks();
        // Sparse: every fifth peak, so most zones never fill their
        // candidate list and anchors wait for the zone to pass.
        let sparse: Vec<Peak> = dense.iter().copied().step_by(5).collect();
        for (zone, fan_out) in [(320, 6), (320, 4), (50, 2), (10, 1), (320, 0), (1, 3)] {
            for set in [&dense, &sparse, &Vec::new(), &dense[..1].to_vec()] {
                let expected = brute_force(set, zone as u64, fan_out, 32.0);
                for seed in [3, 17, 99] {
                    assert_eq!(
                        stream(set, zone, fan_out, seed),
                        expected,
                        "zone {zone} fan-out {fan_out} seed {seed} n {}",
                        set.len()
                    );
                }
            }
        }
    }

    #[test]
    fn hashes_are_emitted_once_the_candidates_are_known() {
        // fan-out 1: three candidates complete the list. Anchor 0's list
        // is complete when the peak at frame 4 is known, long before the
        // zone (10 frames) has passed.
        let mut hasher = TripletHasher::new(10, 1, 32);
        hasher.push(Peak { frame: 0, bin: 5 });
        hasher.push(Peak { frame: 3, bin: 9 });
        hasher.push(Peak { frame: 3, bin: 12 });
        let mut n = 0;
        hasher.advance(3, |_| n += 1);
        assert_eq!(n, 0);
        hasher.push(Peak { frame: 4, bin: 1 });
        hasher.advance(4, |h| {
            assert_eq!(h.frame, 0);
            n += 1;
        });
        // (3,9)→(3,12) shares a frame and is skipped; (3,12)→(4,1) is
        // the one triplet. The anchor is gone from the queue.
        assert_eq!(n, 1, "anchor 0 released with its triplet");
        assert_eq!(hasher.pending.len(), 3);
        // The anchors at frame 3 have only two later peaks: they wait
        // for their zone to pass, then leave the queue together.
        hasher.advance(12, |_| n += 1);
        assert_eq!(hasher.pending.len(), 3);
        hasher.advance(13, |_| n += 1);
        assert_eq!(hasher.pending.len(), 1);
        assert_eq!(hasher.pending[0].frame, 4);
    }

    #[test]
    fn hashes_are_emitted_zone_frames_after_the_anchor_when_sparse() {
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
