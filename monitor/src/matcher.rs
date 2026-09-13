//! Sliding-window evidence accumulation and the confidence score.

use std::collections::{HashMap, VecDeque};

use crate::hashes::Hash;
use crate::index::{FastMap, Index};

/// Tunable parameters of the [`Matcher`].
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// Length of the evidence window in query frames.
    pub window_frames: u64,
    /// Largest pitch shift considered, in bins, either direction.
    pub max_shift: i32,
    /// Smallest tempo factor (reference span / query span) considered.
    pub tempo_min: f64,
    /// Largest tempo factor considered.
    pub tempo_max: f64,
    /// Width of one tempo vote cell.
    pub tempo_step: f64,
    /// Width of one offset vote cell in frames.
    pub offset_step: f64,
    /// Tolerance of the hash lookup on the bin differences.
    pub bin_tolerance: i32,
    /// Tolerance of the hash lookup on the ratio step.
    pub ratio_tolerance: i32,
    /// Evidence count at which the confidence reaches 50; see
    /// [`Matcher::confidence`].
    pub half: f64,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            window_frames: 861,
            max_shift: 24,
            tempo_min: 0.7,
            tempo_max: 1.4,
            tempo_step: 0.02,
            offset_step: 86.0,
            bin_tolerance: 1,
            ratio_tolerance: 1,
            half: 40.0,
        }
    }
}

/// The best-supported hypothesis in the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// Watched song index.
    pub song: u16,
    /// Consistent matches supporting the hypothesis.
    pub evidence: u32,
    /// Query bin minus reference bin (pitch shift of the stream in bins).
    pub shift: i32,
    /// Reference duration / query duration (above one: the stream plays
    /// faster than the watched song).
    pub tempo: f64,
    /// Reference frame corresponding to query frame 0, so that
    /// `frame_ref = tempo · frame_query + offset`.
    pub offset: f64,
}

#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    count: u32,
    tempo_sum: f64,
    /// Sum of the votes' offsets, each computed with the cell's quantized
    /// tempo against the current origin.
    offset_sum: f64,
    /// Sum of the votes' query frames relative to the current origin, so
    /// that the offsets can be refitted with another tempo.
    query_sum: f64,
}

/// One match inside the window, kept so that its exact contribution can
/// be removed from its cell when it expires.
#[derive(Debug, Clone, Copy)]
struct Vote {
    frame: u64,
    key: u64,
    tempo: f64,
    offset: f64,
}

/// Accumulates hash matches over a sliding window of the query and reports
/// the best-supported `(song, shift, tempo, offset)` hypothesis.
///
/// Each matched pair of hashes votes for a cell of that four-dimensional
/// space; the tempo of a match is the ratio of the two triplets' spans
/// (independent of where the query starts) and the offset follows from
/// the anchors. Evidence is the number of votes in the best cell together
/// with its 26 neighbours, which absorbs quantization at cell borders.
///
/// Offsets are measured against a rolling origin a few windows behind the
/// stream and computed with the cell's quantized tempo, so that the
/// unavoidable tempo error of a single match (integer spans) does not
/// scatter the votes of a long stream over many offset cells. When the
/// origin moves, the cells are re-keyed.
#[derive(Debug)]
pub struct Matcher {
    config: MatcherConfig,
    cells: FastMap<u64, Cell>,
    votes: VecDeque<Vote>,
    origin: u64,
    latest_frame: u64,
    lookups: u64,
    matches: u64,
}

impl Matcher {
    /// Creates a matcher.
    pub fn new(config: MatcherConfig) -> Self {
        Self {
            config,
            cells: FastMap::default(),
            votes: VecDeque::new(),
            origin: 0,
            latest_frame: 0,
            lookups: 0,
            matches: 0,
        }
    }

    /// The configuration in use.
    pub fn config(&self) -> &MatcherConfig {
        &self.config
    }

    /// Number of query hashes looked up so far.
    pub fn lookups(&self) -> u64 {
        self.lookups
    }

    /// Number of matched hash pairs so far.
    pub fn matches(&self) -> u64 {
        self.matches
    }

    /// Forgets all evidence.
    pub fn reset(&mut self) {
        self.cells.clear();
        self.votes.clear();
        self.origin = 0;
        self.latest_frame = 0;
    }

    /// Looks up a query hash in `index` and records its matches. Hashes
    /// must arrive in non-decreasing anchor-frame order.
    pub fn push(&mut self, index: &Index, hash: &Hash) {
        self.lookups += 1;
        self.latest_frame = self.latest_frame.max(hash.frame);
        self.rebase();
        let cfg = &self.config;
        let query_span = f64::from(hash.span.max(1));
        let query_frame = hash.frame as f64 - self.origin as f64;
        let mut new_votes = 0u64;
        index.lookup(hash.key, cfg.bin_tolerance, cfg.ratio_tolerance, |entry| {
            let shift = hash.bin as i32 - i32::from(entry.bin);
            if shift.abs() > cfg.max_shift {
                return;
            }
            let tempo = f64::from(entry.span) / query_span;
            if tempo < cfg.tempo_min || tempo > cfg.tempo_max {
                return;
            }
            let tempo_idx = ((tempo - cfg.tempo_min) / cfg.tempo_step).round() as i32;
            let cell_tempo = cfg.tempo_min + f64::from(tempo_idx) * cfg.tempo_step;
            let offset = f64::from(entry.frame) - cell_tempo * query_frame;
            let offset_idx = (offset / cfg.offset_step).round() as i32;
            let key = encode_cell_raw(entry.song, shift, tempo_idx, offset_idx);
            let cell = self.cells.entry(key).or_default();
            cell.count += 1;
            cell.tempo_sum += tempo;
            cell.offset_sum += offset;
            cell.query_sum += query_frame;
            self.votes.push_back(Vote {
                frame: hash.frame,
                key,
                tempo,
                offset,
            });
            new_votes += 1;
        });
        self.matches += new_votes;
        self.expire();
    }

    /// Moves the origin forward once the stream is more than four windows
    /// past it, re-keying the cells so that their offsets stay relative to
    /// the new origin.
    fn rebase(&mut self) {
        let window = self.config.window_frames;
        if self.latest_frame < self.origin + 5 * window {
            return;
        }
        let new_origin = self.latest_frame - window;
        let shift_frames = (new_origin - self.origin) as f64;
        let cfg = &self.config;
        let mut rekeyed: FastMap<u64, Cell> =
            FastMap::with_capacity_and_hasher(self.cells.len(), Default::default());
        // Old key → (new key, offset delta of that cell).
        let mut mapping: FastMap<u64, (u64, f64)> =
            FastMap::with_capacity_and_hasher(self.cells.len(), Default::default());
        for (&key, cell) in &self.cells {
            let (song, shift, tempo_idx, offset_idx) = decode_cell_raw(key);
            let cell_tempo = cfg.tempo_min + f64::from(tempo_idx) * cfg.tempo_step;
            // offset_rel = t_ref - tempo · (t_query - origin) grows by
            // tempo · Δorigin when the origin moves forward.
            let delta = cell_tempo * shift_frames;
            let offset = f64::from(offset_idx) * cfg.offset_step + delta;
            let new_key = encode_cell_raw(
                song,
                shift,
                tempo_idx,
                (offset / cfg.offset_step).round() as i32,
            );
            mapping.insert(key, (new_key, delta));
            let target = rekeyed.entry(new_key).or_default();
            target.count += cell.count;
            target.tempo_sum += cell.tempo_sum;
            target.offset_sum += cell.offset_sum + delta * f64::from(cell.count);
            target.query_sum += cell.query_sum - shift_frames * f64::from(cell.count);
        }
        for vote in &mut self.votes {
            let (new_key, delta) = mapping[&vote.key];
            vote.key = new_key;
            vote.offset += delta;
        }
        self.cells = rekeyed;
        self.origin = new_origin;
    }

    /// Declares the query has reached `frame` (even without new hashes) so
    /// that old evidence expires.
    pub fn advance(&mut self, frame: u64) {
        self.latest_frame = self.latest_frame.max(frame);
        self.rebase();
        self.expire();
    }

    fn expire(&mut self) {
        let horizon = self.latest_frame.saturating_sub(self.config.window_frames);
        while let Some(vote) = self.votes.front().copied() {
            if vote.frame >= horizon {
                break;
            }
            self.votes.pop_front();
            if let Some(cell) = self.cells.get_mut(&vote.key) {
                if cell.count <= 1 {
                    self.cells.remove(&vote.key);
                } else {
                    cell.count -= 1;
                    cell.tempo_sum -= vote.tempo;
                    cell.offset_sum -= vote.offset;
                    cell.query_sum -= vote.frame as f64 - self.origin as f64;
                }
            }
        }
    }

    /// Latest anchor frame seen.
    pub fn latest_frame(&self) -> u64 {
        self.latest_frame
    }

    /// Number of votes currently inside the window.
    pub fn votes_in_window(&self) -> usize {
        self.votes.len()
    }

    /// The best-supported hypothesis over all songs, or `None` without any
    /// vote.
    pub fn best(&self) -> Option<Candidate> {
        self.best_per_song().into_iter().max_by_key(|c| c.evidence)
    }

    /// The best-supported hypothesis of every song that has any vote in
    /// the window, in song order.
    ///
    /// Every occupied cell is a candidate centre: a cell's own count says
    /// nothing about its neighbourhood's, so no cell may be skipped on
    /// its count alone. Instead the votes are also histogrammed over
    /// (song, shift, offset), ignoring the tempo; a neighbourhood lies
    /// inside the 3×3 slab of that histogram, so its sum is at most the
    /// slab sum, and a centre whose slab sum cannot beat the incumbent is
    /// skipped without touching its 27 cells. The result is exactly the
    /// full scan's (tested against it). On audio without the watched
    /// songs the votes are thin and spread out, which makes this bound
    /// tight and the search cheap; during a play the incumbent is large
    /// and prunes nearly everything.
    ///
    /// Ties go to the centre with more votes of its own, then to the
    /// smallest cell key, which keeps the result independent of hash-map
    /// iteration order. The reported shift, tempo and offset are
    /// vote-weighted means over the winning neighbourhood.
    pub fn best_per_song(&self) -> Vec<Candidate> {
        self.best_per_song_with_fit(false)
    }

    /// Experimental point estimates from the most occupied cell inside
    /// each winning neighbourhood. Evidence and winning neighbourhoods
    /// are unchanged; only shift, tempo and offset use the modal cell.
    ///
    /// This avoids averaging distinct repeated-pattern alignments into a
    /// position supported by neither. Ties use the fixed neighbourhood
    /// traversal order. The default method retains its existing semantics.
    pub fn best_per_song_modal(&self) -> Vec<Candidate> {
        self.best_per_song_with_fit(true)
    }

    fn best_per_song_with_fit(&self, modal: bool) -> Vec<Candidate> {
        let mut slab_hist: FastMap<u64, u32> =
            FastMap::with_capacity_and_hasher(self.cells.len(), Default::default());
        let mut seed: HashMap<u16, (u32, u64)> = HashMap::new();
        for (&key, cell) in &self.cells {
            *slab_hist.entry(key & MASK_NO_TEMPO).or_default() += cell.count;
            let song = (key >> 48) as u16;
            let entry = seed.entry(song).or_insert((cell.count, key));
            if (cell.count, std::cmp::Reverse(key)) > (entry.0, std::cmp::Reverse(entry.1)) {
                *entry = (cell.count, key);
            }
        }
        let mut best: HashMap<u16, (u32, u32, u64)> = HashMap::new();
        for (&song, &(count, key)) in &seed {
            let total = self.neighbourhood(key).map(|c| c.count).sum::<u32>();
            best.insert(song, (total, count, key));
        }
        for (&key, cell) in &self.cells {
            let song = (key >> 48) as u16;
            let entry = best.get_mut(&song).expect("seeded");
            let centre = key & MASK_NO_TEMPO;
            let mut bound = 0;
            for ds in [-1i64, 0, 1] {
                let row = centre.wrapping_add((ds << 40) as u64);
                for doff in [-1i64, 0, 1] {
                    bound += slab_hist
                        .get(&row.wrapping_add(doff as u64))
                        .copied()
                        .unwrap_or(0);
                }
            }
            if bound < entry.0 {
                continue;
            }
            let total = self.neighbourhood(key).map(|c| c.count).sum::<u32>();
            if (total, cell.count, std::cmp::Reverse(key))
                > (entry.0, entry.1, std::cmp::Reverse(entry.2))
            {
                *entry = (total, cell.count, key);
            }
        }
        self.candidates(best, modal)
    }

    /// The same search without the bound; the oracle for the tests.
    #[cfg(test)]
    fn best_per_song_full_scan(&self) -> Vec<Candidate> {
        let mut best: HashMap<u16, (u32, u32, u64)> = HashMap::new();
        for (&key, cell) in &self.cells {
            let song = (key >> 48) as u16;
            let total = self.neighbourhood(key).map(|c| c.count).sum::<u32>();
            let entry = best.entry(song).or_insert((total, cell.count, key));
            if (total, cell.count, std::cmp::Reverse(key))
                > (entry.0, entry.1, std::cmp::Reverse(entry.2))
            {
                *entry = (total, cell.count, key);
            }
        }
        self.candidates(best, false)
    }

    fn candidates(&self, best: HashMap<u16, (u32, u32, u64)>, modal: bool) -> Vec<Candidate> {
        let mut out: Vec<Candidate> = best
            .into_iter()
            .filter(|(_, (evidence, _, _))| *evidence > 0)
            .map(|(song, (evidence, _, key))| {
                let cfg = &self.config;
                let (mut n, mut shift_sum) = (0u32, 0i64);
                let (mut tempo_sum, mut offset_sum, mut query_sum, mut quantized) =
                    (0.0, 0.0, 0.0, 0.0);
                let modal_index = if modal {
                    self.neighbourhood_with_shift(key)
                        .enumerate()
                        .max_by_key(|(i, (_, _, cell))| (cell.count, std::cmp::Reverse(*i)))
                        .map(|(i, _)| i)
                } else {
                    None
                };
                for (i, (shift, tempo_idx, cell)) in self.neighbourhood_with_shift(key).enumerate() {
                    if modal_index.is_some_and(|wanted| i != wanted) {
                        continue;
                    }
                    n += cell.count;
                    shift_sum += i64::from(shift) * i64::from(cell.count);
                    tempo_sum += cell.tempo_sum;
                    offset_sum += cell.offset_sum;
                    query_sum += cell.query_sum;
                    let cell_tempo = cfg.tempo_min + f64::from(tempo_idx) * cfg.tempo_step;
                    quantized += cell_tempo * cell.query_sum;
                }
                let n_f = f64::from(n.max(1));
                let tempo = tempo_sum / n_f;
                let shift = (shift_sum as f64 / n_f).round() as i32;
                // Each vote's offset is `ref − cell_tempo · q` for its own
                // cell's quantized tempo. The reported tempo is the votes'
                // mean, so the offset is refitted through the same
                // correspondences with that tempo: the mean of
                // `ref − tempo · q` is the mean offset plus the mean of
                // `(cell_tempo − tempo) · q`. Both describe one line,
                // which is what the verifier and the position rely on.
                let offset_rel = (offset_sum + quantized - tempo * query_sum) / n_f;
                Candidate {
                    song,
                    evidence,
                    shift,
                    tempo,
                    // Relative to the origin so far; express against frame 0.
                    offset: offset_rel - tempo * self.origin as f64,
                }
            })
            .collect();
        out.sort_by_key(|c| c.song);
        out
    }

    fn neighbourhood(&self, key: u64) -> impl Iterator<Item = &Cell> {
        self.neighbourhood_with_shift(key).map(|(_, _, cell)| cell)
    }

    /// The occupied cells among the 27 around `key`, with their shift and
    /// tempo index.
    fn neighbourhood_with_shift(&self, key: u64) -> impl Iterator<Item = (i32, i32, &Cell)> {
        let (song, shift, tempo_idx, offset_idx) = decode_cell_raw(key);
        let mut keys = [(0i32, 0i32, 0u64); 27];
        let mut i = 0;
        for ds in -1..=1 {
            for dt in -1..=1 {
                for doff in -1..=1 {
                    keys[i] = (
                        shift + ds,
                        tempo_idx + dt,
                        encode_cell_raw(song, shift + ds, tempo_idx + dt, offset_idx + doff),
                    );
                    i += 1;
                }
            }
        }
        keys.into_iter().filter_map(|(shift, tempo_idx, k)| {
            self.cells.get(&k).map(|cell| (shift, tempo_idx, cell))
        })
    }

    /// Maps an evidence count to a confidence in `0..=100`:
    /// `100 · n / (n + half)`. `half` is calibrated on audio that contains
    /// no watched song: with `half` at twice the largest evidence ever seen
    /// on such audio, false matches stay below 34 and true matches with a
    /// few hundred votes read above 75. Thirty minutes of simulated radio
    /// without the watched songs peaked at 20 with the default fan-out of
    /// 4 (50 with fan-out 6), hence the default of 40.
    pub fn confidence(&self, evidence: u32) -> f64 {
        let n = f64::from(evidence);
        100.0 * n / (n + self.config.half)
    }
}

/// Cell key layout: song in bits 48–63, `shift + 128` in bits 40–47,
/// `tempo_idx + 128` in bits 32–39 and `offset_idx + 2³¹` in bits 0–31.
/// This mask drops the tempo field.
const MASK_NO_TEMPO: u64 = 0xffff_ff00_ffff_ffff;

fn encode_cell_raw(song: u16, shift: i32, tempo_idx: i32, offset_idx: i32) -> u64 {
    (u64::from(song) << 48)
        | (((shift + 128) as u64 & 0xff) << 40)
        | (((tempo_idx + 128) as u64 & 0xff) << 32)
        | ((offset_idx as i64 + (1 << 31)) as u64 & 0xffff_ffff)
}

fn decode_cell_raw(key: u64) -> (u16, i32, i32, i32) {
    (
        (key >> 48) as u16,
        ((key >> 40) & 0xff) as i32 - 128,
        ((key >> 32) & 0xff) as i32 - 128,
        ((key & 0xffff_ffff) as i64 - (1 << 31)) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashes::TripletHasher;
    use crate::index::IndexBuilder;
    use crate::peaks::Peak;

    fn pseudo_peaks(seed: u64, frames: u64) -> Vec<Peak> {
        let mut seed = seed;
        let mut peaks = Vec::new();
        let mut frame = 0u64;
        while frame < frames {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            frame += 2 + (seed % 12);
            peaks.push(Peak {
                frame,
                bin: 20 + ((seed >> 8) % 120) as u32,
            });
        }
        peaks
    }

    fn hashes(peaks: &[Peak]) -> Vec<Hash> {
        let mut hasher = TripletHasher::new(320, 6, 32);
        let mut out = Vec::new();
        for p in peaks {
            hasher.push(*p);
        }
        hasher.flush(|h| out.push(h));
        out
    }

    #[test]
    fn finds_song_shift_tempo_and_offset() {
        let song_a = pseudo_peaks(1, 20_000);
        let song_b = pseudo_peaks(2, 20_000);
        let mut builder = IndexBuilder::new();
        builder.add_song("a", 20_000, hashes(&song_a));
        builder.add_song("b", 20_000, hashes(&song_b));
        let index = builder.build(8);

        // Query: song b from frame 6000 on, 3 bins higher, 10 % slower
        // (query spans are 1.1× longer, so tempo = 1/1.1).
        let query: Vec<Peak> = song_b
            .iter()
            .filter(|p| p.frame >= 6000 && p.frame < 9000)
            .map(|p| Peak {
                frame: ((p.frame - 6000) as f64 * 1.1).round() as u64,
                bin: p.bin + 3,
            })
            .collect();
        let mut matcher = Matcher::new(MatcherConfig {
            window_frames: 5000,
            ..MatcherConfig::default()
        });
        for h in hashes(&query) {
            matcher.push(&index, &h);
        }
        let best = matcher.best().expect("evidence");
        assert_eq!(best.song, 1);
        assert_eq!(best.shift, 3);
        assert!(
            (best.tempo - 1.0 / 1.1).abs() < 0.01,
            "tempo {}",
            best.tempo
        );
        assert!(
            (best.offset - 6000.0).abs() < 30.0,
            "offset {}",
            best.offset
        );
        assert!(best.evidence > 200, "evidence {}", best.evidence);
        assert!(matcher.confidence(best.evidence) > 85.0);

        // Unrelated peaks give little evidence.
        let mut matcher = Matcher::new(MatcherConfig::default());
        for h in hashes(&pseudo_peaks(99, 3000)) {
            matcher.push(&index, &h);
        }
        let noise = matcher.best().map_or(0, |c| c.evidence);
        assert!(noise < best.evidence / 10, "noise {noise}");
    }

    #[test]
    fn modal_fit_does_not_average_two_supported_offsets_into_a_false_position() {
        let mut matcher = Matcher::new(MatcherConfig::default());
        for (offset_idx, count) in [(-1, 120), (0, 1), (1, 80)] {
            for _ in 0..count {
                inject(
                    &mut matcher,
                    1000,
                    encode_cell_raw(0, 0, 15, offset_idx),
                    1.0,
                    f64::from(offset_idx) * 86.0,
                );
            }
        }
        let legacy = matcher.best_per_song()[0];
        let modal = matcher.best_per_song_modal()[0];
        assert_eq!(legacy.evidence, 201);
        assert_eq!(modal.evidence, legacy.evidence);
        assert!((modal.offset + 86.0).abs() < 1e-9);
        let query = [Peak {
            frame: 500,
            bin: 20,
        }];
        let track = crate::PeakTrack::new(vec![Peak {
            frame: 414,
            bin: 20,
        }]);
        let verified = |c: Candidate| {
            track
                .verify(&query, c.shift, c.tempo, c.offset, 2, 0)
                .query_matched
        };
        assert_eq!(verified(legacy), 0);
        assert_eq!(verified(modal), 1);
    }

    #[test]
    fn modal_fit_preserves_a_single_alignment_through_expiry_and_rebase() {
        let mut matcher = Matcher::new(MatcherConfig::default());
        for frame in 4200..4300 {
            inject(&mut matcher, frame, encode_cell_raw(0, 0, 15, -1), 1.0, -86.0);
        }
        let original = matcher.best_per_song_modal();
        assert_eq!(original, matcher.best_per_song());
        matcher.advance(5 * matcher.config.window_frames);
        assert_eq!(matcher.best_per_song_modal(), original);
        matcher.advance(100_000);
        assert!(matcher.best_per_song_modal().is_empty());
        assert!(matcher.best_per_song().is_empty());
    }

    #[test]
    fn evidence_expires_with_the_window() {
        let song = pseudo_peaks(5, 10_000);
        let mut builder = IndexBuilder::new();
        builder.add_song("s", 10_000, hashes(&song));
        let index = builder.build(8);
        let mut matcher = Matcher::new(MatcherConfig {
            window_frames: 500,
            ..MatcherConfig::default()
        });
        for h in hashes(&song[..200]) {
            matcher.push(&index, &h);
        }
        assert!(matcher.best().unwrap().evidence > 0);
        matcher.advance(1_000_000);
        assert_eq!(matcher.votes_in_window(), 0);
        assert!(matcher.best().is_none());
    }

    #[test]
    fn evidence_does_not_decay_late_in_a_long_stream() {
        // The same 40 s of a stretched song played 20 minutes into the
        // stream must gather about as much evidence as at the start.
        let song = pseudo_peaks(11, 20_000);
        let mut builder = IndexBuilder::new();
        builder.add_song("s", 20_000, hashes(&song));
        let index = builder.build(8);
        let play = |start: u64| -> Vec<Peak> {
            song.iter()
                .filter(|p| p.frame >= 3000 && p.frame < 10_000)
                .map(|p| Peak {
                    frame: start + ((p.frame - 3000) as f64 * 1.087).round() as u64,
                    bin: p.bin,
                })
                .collect()
        };
        let evidence = |start: u64| {
            let mut matcher = Matcher::new(MatcherConfig::default());
            for h in hashes(&play(start)) {
                matcher.push(&index, &h);
            }
            let best = matcher.best().unwrap();
            assert!(
                (best.tempo - 1.0 / 1.087).abs() < 0.02,
                "tempo {}",
                best.tempo
            );
            // Position at the last query frame must land where the song is.
            let last = start as f64 + 7000.0 * 1.087;
            let position = best.tempo * last + best.offset;
            assert!(
                (position - 10_000.0).abs() < 100.0,
                "position {position} for start {start}"
            );
            best.evidence
        };
        let early = evidence(0);
        let late = evidence(200_000);
        assert!(
            late as f64 > 0.8 * early as f64,
            "early {early} late {late}"
        );
    }

    fn inject(matcher: &mut Matcher, frame: u64, key: u64, tempo: f64, offset: f64) {
        let cell = matcher.cells.entry(key).or_default();
        cell.count += 1;
        cell.tempo_sum += tempo;
        cell.offset_sum += offset;
        cell.query_sum += frame as f64 - matcher.origin as f64;
        matcher.votes.push_back(Vote {
            frame,
            key,
            tempo,
            offset,
        });
    }

    #[test]
    fn a_strong_neighbourhood_beats_a_strong_single_cell() {
        // One isolated cell with 30 votes against a 3×3×3 cluster of nine
        // votes per cell: the cluster's centre has evidence 243 and must
        // win, whatever its own count.
        let mut matcher = Matcher::new(MatcherConfig::default());
        for _ in 0..30 {
            inject(&mut matcher, 0, encode_cell_raw(0, 10, 15, 50), 1.0, 0.0);
        }
        for ds in -1..=1 {
            for dt in -1..=1 {
                for doff in -1..=1 {
                    for _ in 0..9 {
                        inject(
                            &mut matcher,
                            0,
                            encode_cell_raw(0, ds, 15 + dt, doff),
                            1.0,
                            0.0,
                        );
                    }
                }
            }
        }
        let best = matcher.best().unwrap();
        assert_eq!(best.evidence, 243);
        assert_eq!(best.shift, 0);
        assert!(matcher.confidence(best.evidence) > 70.0);
    }

    #[test]
    fn expiring_votes_removes_their_own_contribution() {
        // Two early votes at (tempo 0.9, offset −40) and two late ones at
        // (1.1, +40) share a cell whose quantized tempo is 1.0. Once the
        // early ones expire the estimate must come from the late votes
        // only, not from the cell's old mean: tempo 1.1, and the offset
        // of the line with that tempo through their correspondence
        // (frame 150 maps to 190), 190 − 1.1 · 150 = 25.
        let mut matcher = Matcher::new(MatcherConfig {
            window_frames: 100,
            ..MatcherConfig::default()
        });
        let key = encode_cell_raw(0, 0, 15, 0);
        inject(&mut matcher, 0, key, 0.9, -40.0);
        inject(&mut matcher, 0, key, 0.9, -40.0);
        inject(&mut matcher, 150, key, 1.1, 40.0);
        inject(&mut matcher, 150, key, 1.1, 40.0);
        matcher.latest_frame = 150;
        let before = matcher.best().unwrap();
        assert_eq!(before.evidence, 4);
        assert!((before.tempo - 1.0).abs() < 1e-9);
        assert!(before.offset.abs() < 1e-9);
        matcher.advance(200);
        let after = matcher.best().unwrap();
        assert_eq!(after.evidence, 2);
        assert!((after.tempo - 1.1).abs() < 1e-9, "tempo {}", after.tempo);
        assert!(
            (after.offset - 25.0).abs() < 1e-9,
            "offset {}",
            after.offset
        );
        matcher.advance(300);
        assert!(matcher.best().is_none());
    }

    #[test]
    fn bounded_search_matches_the_full_scan() {
        // Random histograms: a sparse background, a few clusters, and
        // repeated cells so that ties occur.
        let mut seed = 0x9E37_79B9u64;
        let mut next = |m: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % m
        };
        for trial in 0..300 {
            let mut matcher = Matcher::new(MatcherConfig::default());
            let songs = 1 + next(3) as u16;
            let background = next(400) as usize;
            for _ in 0..background {
                let key = encode_cell_raw(
                    next(u64::from(songs)) as u16,
                    next(49) as i32 - 24,
                    next(36) as i32,
                    next(60) as i32 - 30,
                );
                for _ in 0..=next(3) {
                    inject(&mut matcher, 0, key, 1.0, 0.0);
                }
            }
            for _ in 0..next(4) {
                let (song, shift, tempo, offset) = (
                    next(u64::from(songs)) as u16,
                    next(45) as i32 - 22,
                    2 + next(32) as i32,
                    next(56) as i32 - 28,
                );
                let strength = 1 + next(12);
                for ds in -1..=1 {
                    for dt in -1..=1 {
                        for doff in -1..=1 {
                            if next(4) == 0 {
                                continue;
                            }
                            let key = encode_cell_raw(song, shift + ds, tempo + dt, offset + doff);
                            for _ in 0..strength {
                                inject(&mut matcher, 0, key, 1.0, 0.0);
                            }
                        }
                    }
                }
            }
            assert_eq!(
                matcher.best_per_song(),
                matcher.best_per_song_full_scan(),
                "trial {trial}"
            );
        }
    }

    /// The reported tempo and offset must describe one alignment line
    /// through the matched peaks: `frame_ref = tempo · frame_query +
    /// offset` within the verifier's tolerance. Tempos between the
    /// quantization centres (cells are 0.02 wide) and plays that start
    /// just before and after a rebase of the origin are the cases in
    /// which a mean tempo combined with offsets computed from the cells'
    /// quantized tempos drifts apart from the votes.
    #[test]
    fn reported_tempo_and_offset_predict_the_matched_peaks() {
        use crate::verify::PeakTrack;
        let song = pseudo_peaks(21, 40_000);
        let mut builder = IndexBuilder::new();
        builder.add_song("s", 40_000, hashes(&song));
        let index = builder.build(8);
        let track = PeakTrack::new(song.clone());
        let window = MatcherConfig::default().window_frames;
        for &tempo in &[0.985, 0.995, 1.005, 1.01, 1.015, 1.03, 1.05, 1.1] {
            // Rebases happen when the stream is five windows past the
            // origin; a play of 3000 / tempo frames from 1300 ends just
            // before the first one (the largest origin-relative frames),
            // from 4200 it straddles it, from 8700 the second.
            for &start in &[0u64, 1300, 3500, 4200, 4400, 8700] {
                let query: Vec<Peak> = song
                    .iter()
                    .filter(|p| p.frame >= 6000 && p.frame < 9000)
                    .map(|p| Peak {
                        frame: start + ((p.frame - 6000) as f64 / tempo).round() as u64,
                        bin: p.bin + 2,
                    })
                    .collect();
                let mut matcher = Matcher::new(MatcherConfig::default());
                for h in hashes(&query) {
                    matcher.push(&index, &h);
                }
                let best = matcher.best().expect("evidence");
                let latest = query.last().unwrap().frame;
                let recent: Vec<Peak> = query
                    .iter()
                    .filter(|p| p.frame + window >= latest)
                    .copied()
                    .collect();
                let v = track.verify(&recent, best.shift, best.tempo, best.offset, 4, 0);
                let worst = recent
                    .iter()
                    .zip(song.iter().filter(|p| {
                        p.frame >= 6000
                            && p.frame < 9000
                            && start + ((p.frame - 6000) as f64 / tempo).round() as u64 + window
                                >= latest
                    }))
                    .map(|(q, r)| {
                        (best.tempo * q.frame as f64 + best.offset - r.frame as f64).abs()
                    })
                    .fold(0.0f64, f64::max);
                assert!(
                    v.query_fraction() > 0.9 && worst < 4.0,
                    "tempo {tempo} start {start}: reported tempo {:.4} offset {:.1}, \
                     alignment {:.2}, worst prediction error {worst:.1} frames",
                    best.tempo,
                    best.offset,
                    v.query_fraction()
                );
            }
        }
    }

    #[test]
    fn cell_keys_round_trip() {
        for &(s, sh, t, o) in &[
            (0u16, 0i32, 0i32, 0i32),
            (7, -24, 35, -4000),
            (65535, 24, 0, 4000),
        ] {
            assert_eq!(decode_cell_raw(encode_cell_raw(s, sh, t, o)), (s, sh, t, o));
        }
    }
}
