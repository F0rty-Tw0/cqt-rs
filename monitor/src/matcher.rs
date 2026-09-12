//! Sliding-window evidence accumulation and the confidence score.

use std::collections::{HashMap, VecDeque};

use crate::hashes::Hash;
use crate::index::Index;

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
            half: 100.0,
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
    offset_sum: f64,
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
    cells: HashMap<u64, Cell>,
    votes: VecDeque<(u64, u64)>,
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
            cells: HashMap::new(),
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
            self.votes.push_back((hash.frame, key));
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
        let mut rekeyed: HashMap<u64, Cell> = HashMap::with_capacity(self.cells.len());
        let mut mapping: HashMap<u64, u64> = HashMap::with_capacity(self.cells.len());
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
            mapping.insert(key, new_key);
            let target = rekeyed.entry(new_key).or_default();
            target.count += cell.count;
            target.tempo_sum += cell.tempo_sum;
            target.offset_sum += cell.offset_sum + delta * f64::from(cell.count);
        }
        for vote in &mut self.votes {
            vote.1 = mapping[&vote.1];
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
        while let Some(&(frame, key)) = self.votes.front() {
            if frame >= horizon {
                break;
            }
            self.votes.pop_front();
            if let Some(cell) = self.cells.get_mut(&key) {
                if cell.count <= 1 {
                    self.cells.remove(&key);
                } else {
                    // Remove the cell's average contribution; the sums are
                    // only used for the estimates of the winning cell.
                    let n = f64::from(cell.count);
                    cell.tempo_sum -= cell.tempo_sum / n;
                    cell.offset_sum -= cell.offset_sum / n;
                    cell.count -= 1;
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
    pub fn best_per_song(&self) -> Vec<Candidate> {
        // Only cells with at least two votes, and at least a third of the
        // strongest cell of their song, are worth a neighbourhood sum.
        let mut max_single: HashMap<u16, u32> = HashMap::new();
        for (&key, cell) in &self.cells {
            let song = (key >> 48) as u16;
            let m = max_single.entry(song).or_default();
            *m = (*m).max(cell.count);
        }
        let mut best: HashMap<u16, (u32, u64)> = HashMap::new();
        for (&key, cell) in &self.cells {
            let song = (key >> 48) as u16;
            let threshold = (max_single[&song] / 3).max(2);
            if cell.count < threshold {
                continue;
            }
            let total = self.neighbourhood(key).map(|c| c.count).sum::<u32>();
            let entry = best.entry(song).or_insert((0, key));
            if total > entry.0 {
                *entry = (total, key);
            }
        }
        let mut out: Vec<Candidate> = best
            .into_iter()
            .filter(|(_, (evidence, _))| *evidence > 0)
            .map(|(song, (evidence, key))| {
                let (_, shift, _, _) = decode_cell(&self.config, key);
                let (mut n, mut tempo_sum, mut offset_sum) = (0u32, 0.0, 0.0);
                for cell in self.neighbourhood(key) {
                    n += cell.count;
                    tempo_sum += cell.tempo_sum;
                    offset_sum += cell.offset_sum;
                }
                let tempo = tempo_sum / f64::from(n.max(1));
                Candidate {
                    song,
                    evidence,
                    shift,
                    tempo,
                    // Offsets are relative to the origin and were computed
                    // with the cell tempo; express them against frame 0.
                    offset: offset_sum / f64::from(n.max(1)) - tempo * self.origin as f64,
                }
            })
            .collect();
        out.sort_by_key(|c| c.song);
        out
    }

    fn neighbourhood(&self, key: u64) -> impl Iterator<Item = &Cell> {
        let (song, shift, tempo_idx, offset_idx) = decode_cell_raw(key);
        let mut keys = [0u64; 27];
        let mut i = 0;
        for ds in -1..=1 {
            for dt in -1..=1 {
                for doff in -1..=1 {
                    keys[i] = encode_cell_raw(song, shift + ds, tempo_idx + dt, offset_idx + doff);
                    i += 1;
                }
            }
        }
        keys.into_iter().filter_map(|k| self.cells.get(&k))
    }

    /// Maps an evidence count to a confidence in `0..=100`:
    /// `100 · n / (n + half)`. `half` is calibrated on audio that contains
    /// no watched song: with `half` at twice the largest evidence ever seen
    /// on such audio, false matches stay below 34 and true matches with a
    /// few hundred votes read above 75. Thirty minutes of simulated radio
    /// without the watched songs peaked at 51, hence the default of 100.
    pub fn confidence(&self, evidence: u32) -> f64 {
        let n = f64::from(evidence);
        100.0 * n / (n + self.config.half)
    }
}

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

fn decode_cell(cfg: &MatcherConfig, key: u64) -> (u16, i32, f64, f64) {
    let (song, shift, tempo_idx, offset_idx) = decode_cell_raw(key);
    (
        song,
        shift,
        cfg.tempo_min + f64::from(tempo_idx) * cfg.tempo_step,
        f64::from(offset_idx) * cfg.offset_step,
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
