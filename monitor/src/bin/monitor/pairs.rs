//! E016 bounded, opt-in pair proposals. No labels or verifier access here.
use cqt_monitor::{Candidate, Peak};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const INDEX_LIMIT: usize = 4_000_000;
const PEAK_LIMIT: usize = 512;
const PAIR_LIMIT: usize = 4096;
const LOOKUP_LIMIT: usize = 4096;
const VOTE_LIMIT: usize = 262_144;
const FIT_LIMIT: usize = 64;
const CANDIDATE_LIMIT: usize = 8;
type Key = (u16, i32, i32, i64);

#[derive(Clone, Copy, Debug)]
struct Pair {
    anchor: Peak,
    delta: i32,
    span: u64,
}

fn pairs(peaks: &[Peak], fps: f64, mut emit: impl FnMut(Pair) -> bool) {
    let min = (0.25 * fps).ceil().max(1.0) as u64;
    let max = (2.0 * fps).floor() as u64;
    for (i, &anchor) in peaks.iter().enumerate() {
        for &partner in peaks[i + 1..]
            .iter()
            .skip_while(|p| p.frame - anchor.frame < min)
            .take_while(|p| p.frame - anchor.frame <= max)
            .take(16)
        {
            if !emit(Pair {
                anchor,
                delta: partner.bin as i32 - anchor.bin as i32,
                span: partner.frame - anchor.frame,
            }) {
                return;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    song: u16,
    anchor: Peak,
    span: u64,
}

#[derive(Default)]
pub(super) struct PairIndex {
    entries: BTreeMap<i32, Vec<Entry>>,
    pub(super) len: usize,
    pub(super) rejected_songs: usize,
}

impl PairIndex {
    pub(super) fn add_song(&mut self, song: u16, peaks: &[Peak], fps: f64) {
        let mut pending = Vec::new();
        let mut overflow = false;
        pairs(peaks, fps, |p| {
            if self.len + pending.len() == INDEX_LIMIT {
                overflow = true;
                return false;
            }
            pending.push(p);
            true
        });
        if overflow {
            self.rejected_songs += 1;
            return;
        }
        self.len += pending.len();
        for p in pending {
            self.entries.entry(p.delta).or_default().push(Entry {
                song,
                anchor: p.anchor,
                span: p.span,
            });
        }
    }

    pub(super) fn finish(&mut self) {
        for entries in self.entries.values_mut() {
            entries.sort_unstable_by_key(|e| (e.span, e.song, e.anchor));
            entries.shrink_to_fit();
        }
    }

    pub(super) fn bytes(&self) -> usize {
        self.entries
            .values()
            .map(|v| v.capacity() * std::mem::size_of::<Entry>())
            .sum()
    }

    fn ranges(&self, pair: Pair) -> Vec<&[Entry]> {
        (pair.delta - 1..=pair.delta + 1)
            .filter_map(|delta| {
                let entries = self.entries.get(&delta)?;
                let lo = entries.partition_point(|e| (e.span as f64) < 0.7 * pair.span as f64);
                let hi = entries.partition_point(|e| e.span as f64 <= 1.4 * pair.span as f64);
                Some(&entries[lo..hi])
            })
            .collect()
    }
}

#[derive(Default, Debug)]
pub(super) struct Stats {
    pub(super) peaks: usize,
    pub(super) peak_overflow: bool,
    pub(super) pairs: usize,
    pub(super) pair_limit: bool,
    pub(super) occurrences: usize,
    pub(super) collision_ranges: usize,
    pub(super) collision_entries: usize,
    pub(super) votes: usize,
    pub(super) vote_limit: bool,
    pub(super) cells: usize,
    pub(super) fits: usize,
    pub(super) candidates: usize,
}

#[derive(Clone, Copy, Debug)]
struct Hit {
    query: Peak,
    reference: Peak,
}

pub(super) struct Retrieval {
    recent: VecDeque<Peak>,
    discarded: Option<u64>,
    fps: f64,
}

impl Retrieval {
    pub(super) fn new(fps: f64) -> Self {
        Self {
            recent: VecDeque::new(),
            discarded: None,
            fps,
        }
    }

    pub(super) fn extend(&mut self, query: &[Peak], end: u64) {
        let cutoff = end.saturating_sub((10.0 * self.fps).round() as u64);
        while self.recent.front().is_some_and(|p| p.frame < cutoff) {
            self.recent.pop_front();
        }
        for &p in query.iter().filter(|p| p.frame >= cutoff) {
            if self.recent.len() == PEAK_LIMIT {
                self.discarded = self.recent.pop_front().map(|p| p.frame);
            }
            self.recent.push_back(p);
        }
        if self.discarded.is_some_and(|f| f < cutoff) {
            self.discarded = None;
        }
    }

    pub(super) fn retrieve(
        &self,
        index: &PairIndex,
        end: u64,
        eligible: &[bool],
    ) -> (Vec<Candidate>, Stats) {
        let mut stats = Stats {
            peaks: self.recent.len(),
            peak_overflow: self.discarded.is_some(),
            ..Stats::default()
        };
        if stats.peak_overflow || !eligible.iter().any(|e| *e) {
            return (Vec::new(), stats);
        }
        let origin = end.saturating_sub((10.0 * self.fps).round() as u64);
        let mut cells: BTreeMap<Key, Vec<Hit>> = BTreeMap::new();
        let query: Vec<_> = self.recent.iter().copied().collect();
        pairs(&query, self.fps, |pair| {
            if stats.pairs == PAIR_LIMIT {
                stats.pair_limit = true;
                return false;
            }
            stats.pairs += 1;
            let ranges = index.ranges(pair);
            let count: usize = ranges.iter().map(|r| r.len()).sum();
            stats.occurrences += count;
            if count > LOOKUP_LIMIT {
                stats.collision_ranges += 1;
                stats.collision_entries += count;
                return true;
            }
            for e in ranges.into_iter().flatten() {
                if !eligible[usize::from(e.song)] {
                    continue;
                }
                let shift = pair.anchor.bin as i32 - e.anchor.bin as i32;
                if shift.abs() > 24 {
                    continue;
                }
                if stats.votes == VOTE_LIMIT {
                    stats.vote_limit = true;
                    return false;
                }
                let tempo = e.span as f64 / pair.span as f64;
                let ti = ((tempo - 0.7) / 0.02).round() as i32;
                let cell_tempo = 0.7 + 0.02 * f64::from(ti);
                let offset =
                    e.anchor.frame as f64 - cell_tempo * (pair.anchor.frame - origin) as f64;
                let oi = (offset / (0.5 * self.fps)).round() as i64;
                cells.entry((e.song, shift, ti, oi)).or_default().push(Hit {
                    query: pair.anchor,
                    reference: e.anchor,
                });
                stats.votes += 1;
            }
            true
        });
        stats.cells = cells.len();
        let mut ranked: Vec<_> = cells
            .iter()
            .map(|(&key, hits)| {
                let distinct = hits
                    .iter()
                    .map(|h| h.query.frame)
                    .collect::<BTreeSet<_>>()
                    .len();
                (distinct, key)
            })
            .collect();
        ranked.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut proposals = Vec::new();
        for (_, key) in ranked.into_iter().take(FIT_LIMIT) {
            stats.fits += 1;
            let mut hits = Vec::new();
            for ds in -1..=1 {
                for dt in -1..=1 {
                    for dp in -1..=1 {
                        if let Some(v) = cells.get(&(key.0, key.1 + ds, key.2 + dt, key.3 + dp)) {
                            hits.extend_from_slice(v);
                        }
                    }
                }
            }
            if let Some(candidate) = fit(&mut hits, key, origin, end, self.fps) {
                proposals.push(candidate);
            }
        }
        proposals.sort_by(|a, b| {
            b.evidence
                .cmp(&a.evidence)
                .then_with(|| a.song.cmp(&b.song))
                .then_with(|| a.shift.cmp(&b.shift))
                .then_with(|| a.tempo.total_cmp(&b.tempo))
                .then_with(|| a.offset.total_cmp(&b.offset))
        });
        let mut result: Vec<Candidate> = Vec::new();
        for c in proposals {
            if result.iter().filter(|p| p.song == c.song).count() >= 3
                || result.iter().any(|p| {
                    p.song == c.song
                        && (p.shift - c.shift).abs() <= 2
                        && (p.tempo - c.tempo).abs() <= 0.05
                        && ((p.tempo - c.tempo) * end as f64 + p.offset - c.offset).abs()
                            <= 0.5 * self.fps
                })
            {
                continue;
            }
            result.push(c);
            if result.len() == CANDIDATE_LIMIT {
                break;
            }
        }
        stats.candidates = result.len();
        (result, stats)
    }
}

fn supported(hits: &[Hit], end: u64, fps: f64) -> bool {
    if hits.len() < 6 {
        return false;
    }
    let first = hits.iter().map(|h| h.query.frame).min().unwrap();
    let last = hits.iter().map(|h| h.query.frame).max().unwrap();
    let bins = hits
        .iter()
        .map(|h| (h.query.frame as f64 / (2.0 * fps)).floor() as u64)
        .collect::<BTreeSet<_>>();
    bins.len() >= 3 && (last - first) as f64 >= 4.0 * fps && (end - last) as f64 <= 2.0 * fps
}

fn fit(hits: &mut [Hit], key: Key, origin: u64, end: u64, fps: f64) -> Option<Candidate> {
    let seed_tempo = 0.7 + 0.02 * f64::from(key.2);
    let seed_offset = key.3 as f64 * 0.5 * fps;
    let residual = |h: &Hit| {
        (h.reference.frame as f64 - seed_tempo * (h.query.frame - origin) as f64 - seed_offset)
            .abs()
    };
    hits.sort_unstable_by(|a, b| {
        residual(a)
            .total_cmp(&residual(b))
            .then_with(|| a.query.cmp(&b.query))
            .then_with(|| a.reference.cmp(&b.reference))
    });
    let mut q = BTreeSet::new();
    let mut r = BTreeSet::new();
    let mut chosen = Vec::new();
    for &h in hits.iter() {
        if !q.contains(&h.query.frame) && !r.contains(&h.reference.frame) {
            q.insert(h.query.frame);
            r.insert(h.reference.frame);
            chosen.push(h);
        }
    }
    if !supported(&chosen, end, fps) {
        return None;
    }
    let mut tempo = seed_tempo;
    let mut offset = seed_offset;
    let shift = key.1;
    for _ in 0..2 {
        let n = chosen.len() as f64;
        let mx = chosen
            .iter()
            .map(|h| (h.query.frame - origin) as f64)
            .sum::<f64>()
            / n;
        let my = chosen.iter().map(|h| h.reference.frame as f64).sum::<f64>() / n;
        let variance = chosen
            .iter()
            .map(|h| ((h.query.frame - origin) as f64 - mx).powi(2))
            .sum::<f64>();
        if variance == 0.0 {
            return None;
        }
        tempo = chosen
            .iter()
            .map(|h| ((h.query.frame - origin) as f64 - mx) * (h.reference.frame as f64 - my))
            .sum::<f64>()
            / variance;
        offset = my - tempo * mx;
        chosen.retain(|h| {
            (h.reference.frame as f64 - tempo * (h.query.frame - origin) as f64 - offset).abs()
                <= 4.0
                && (h.query.bin as i32 - h.reference.bin as i32 - shift).abs() <= 1
        });
        if !supported(&chosen, end, fps) {
            return None;
        }
    }
    if !(0.7..=1.4).contains(&tempo) || shift.abs() > 24 {
        return None;
    }
    Some(Candidate {
        song: key.0,
        evidence: chosen.len() as u32,
        shift,
        tempo,
        offset: offset - tempo * origin as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(seed: u64) -> Vec<Peak> {
        let mut seed = seed;
        let mut frame = 0;
        let mut peaks = Vec::new();
        while frame < 10_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            frame += 10 + seed % 16;
            peaks.push(Peak {
                frame,
                bin: 30 + ((seed >> 8) % 110) as u32,
            });
        }
        peaks
    }

    #[test]
    fn sparse_pairs_preserve_shift_tempo_and_late_stream_position() {
        let reference = reference(17);
        let mut index = PairIndex::default();
        index.add_song(0, &reference, 100.0);
        index.add_song(1, &self::reference(19), 100.0);
        index.finish();
        for tempo in [0.72, 0.88, 1.0, 1.12, 1.38] {
            for start in [0, 200_000] {
                let query: Vec<_> = reference
                    .iter()
                    .filter(|p| (3000..3800).contains(&p.frame))
                    .step_by(2)
                    .map(|p| Peak {
                        frame: start + ((p.frame - 3000) as f64 / tempo).round() as u64,
                        bin: p.bin + 4,
                    })
                    .collect();
                let end = query.last().unwrap().frame + 1;
                let mut retrieval = Retrieval::new(100.0);
                retrieval.extend(&query, end);
                let (got, stats) = retrieval.retrieve(&index, end, &[true, true]);
                let c = got
                    .iter()
                    .find(|c| c.song == 0)
                    .unwrap_or_else(|| panic!("tempo {tempo} start {start}: {stats:?} {got:?}"));
                assert!((c.tempo - tempo).abs() < 0.01, "{c:?}");
                assert_eq!(c.shift, 4);
                assert!(
                    (c.tempo * start as f64 + c.offset - 3000.0).abs() < 4.0,
                    "{c:?}"
                );
                assert!(c.evidence >= 6);
                assert!(
                    stats.candidates <= CANDIDATE_LIMIT
                        && stats.fits <= FIT_LIMIT
                        && stats.votes <= VOTE_LIMIT
                );
                assert!(
                    retrieval
                        .retrieve(&index, end, &[false, false])
                        .0
                        .is_empty()
                );
            }
        }
    }

    #[test]
    fn one_anchor_many_partners_and_short_support_cannot_admit() {
        let hit = Hit {
            query: Peak {
                frame: 900,
                bin: 30,
            },
            reference: Peak {
                frame: 1900,
                bin: 30,
            },
        };
        let mut duplicate = vec![hit; 1000];
        assert!(fit(&mut duplicate, (0, 0, 15, 20), 0, 1000, 100.0).is_none());
        let mut short: Vec<_> = (0..20)
            .map(|i| Hit {
                query: Peak {
                    frame: 800 + i * 5,
                    bin: 30,
                },
                reference: Peak {
                    frame: 1800 + i * 5,
                    bin: 30,
                },
            })
            .collect();
        assert!(fit(&mut short, (0, 0, 15, 20), 0, 1000, 100.0).is_none());
        let mut reused: Vec<_> = (0..10)
            .map(|i| Hit {
                query: Peak {
                    frame: i * 100,
                    bin: 30,
                },
                reference: Peak {
                    frame: 1000,
                    bin: 30,
                },
            })
            .collect();
        assert!(fit(&mut reused, (0, 0, 15, 20), 0, 1000, 100.0).is_none());
    }

    #[test]
    fn crowded_lookups_are_dropped_whole_and_context_overflow_expires() {
        let mut index = PairIndex::default();
        index.entries.insert(
            0,
            vec![
                Entry {
                    song: 0,
                    anchor: Peak {
                        frame: 1000,
                        bin: 40
                    },
                    span: 100
                };
                LOOKUP_LIMIT + 1
            ],
        );
        let query = [
            Peak { frame: 0, bin: 40 },
            Peak {
                frame: 100,
                bin: 40,
            },
        ];
        let mut retrieval = Retrieval::new(100.0);
        retrieval.extend(&query, 101);
        let (got, stats) = retrieval.retrieve(&index, 101, &[true]);
        assert!(got.is_empty());
        assert_eq!(stats.collision_ranges, 1);
        assert_eq!(stats.collision_entries, LOOKUP_LIMIT + 1);
        assert_eq!(stats.votes, 0);
        let dense: Vec<_> = (101..101 + PEAK_LIMIT as u64 + 1)
            .map(|frame| Peak { frame, bin: 40 })
            .collect();
        retrieval.extend(&dense, 700);
        assert_eq!(retrieval.recent.len(), PEAK_LIMIT);
        assert!(retrieval.retrieve(&index, 700, &[true]).1.peak_overflow);
        retrieval.extend(&[], 3000);
        let (got, stats) = retrieval.retrieve(&index, 3000, &[true]);
        assert!(!stats.peak_overflow && got.is_empty() && stats.peaks == 0);
    }

    #[test]
    fn query_pair_and_vote_limits_stop_work_without_admission_from_repetition() {
        let mut retrieval = Retrieval::new(100.0);
        let query: Vec<_> = (0..PEAK_LIMIT as u64)
            .map(|frame| Peak { frame, bin: 40 })
            .collect();
        retrieval.extend(&query, PEAK_LIMIT as u64);
        let (_, stats) = retrieval.retrieve(&PairIndex::default(), PEAK_LIMIT as u64, &[true]);
        assert_eq!(stats.pairs, PAIR_LIMIT);
        assert!(stats.pair_limit);
        let mut index = PairIndex::default();
        index.entries.insert(
            0,
            vec![
                Entry {
                    song: 0,
                    anchor: Peak {
                        frame: 1000,
                        bin: 40
                    },
                    span: 100
                };
                LOOKUP_LIMIT
            ],
        );
        let query: Vec<_> = (0..200)
            .map(|i| Peak {
                frame: i * 5,
                bin: 40,
            })
            .collect();
        let mut retrieval = Retrieval::new(100.0);
        retrieval.extend(&query, 1000);
        let (got, stats) = retrieval.retrieve(&index, 1000, &[true]);
        assert_eq!(stats.votes, VOTE_LIMIT);
        assert!(stats.vote_limit);
        assert!(got.is_empty());
    }

    #[test]
    fn pair_generation_is_bounded_and_uses_actual_span() {
        let peaks: Vec<_> = (0..100)
            .map(|i| Peak {
                frame: i * 10,
                bin: (i % 100) as u32,
            })
            .collect();
        let mut got = Vec::new();
        pairs(&peaks, 100.0, |p| {
            got.push(p);
            true
        });
        assert_eq!(got.iter().filter(|p| p.anchor.frame == 0).count(), 16);
        assert!(got.iter().all(|p| (25..=200).contains(&p.span)));
        let mut index = PairIndex {
            len: INDEX_LIMIT,
            ..PairIndex::default()
        };
        index.add_song(0, &peaks, 100.0);
        assert_eq!(index.len, INDEX_LIMIT);
        assert_eq!(index.rejected_songs, 1);
        assert!(index.entries.is_empty());
    }
}
