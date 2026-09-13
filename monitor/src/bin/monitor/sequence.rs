//! Experimental disjoint anchor windows and song-level sequence confirmation.
use super::{Context, Options, verification_json};
use cqt_monitor::{Candidate, Hash, Index, Matcher, MatcherConfig, Peak, Scored};
use std::collections::VecDeque;
use std::io::Write;

#[derive(Clone, Copy, Debug)]
struct Observation {
    begin: u64,
    end: u64,
    scored: Scored,
}

#[derive(Clone, Debug, Default)]
struct State {
    pending: VecDeque<Observation>,
    active: Option<Play>,
}

#[derive(Clone, Copy, Debug)]
struct Play {
    begin: u64,
    last: Observation,
}

#[derive(Clone, Copy, Debug)]
enum Change {
    Start {
        play: Play,
        evidence: u32,
    },
    End {
        song: u16,
        play: Play,
        reason: &'static str,
    },
}

struct Decisions {
    states: Vec<State>,
    threshold: f64,
    half: f64,
    start_alignment: f64,
    hold_alignment: f64,
    position_tolerance: f64,
    release: u64,
}

impl Decisions {
    fn agrees(&self, a: Observation, b: Observation) -> bool {
        let (x, y) = (a.scored.candidate, b.scored.candidate);
        let at = b.end as f64;
        x.song == y.song
            && (x.shift - y.shift).abs() <= 2
            && (x.tempo - y.tempo).abs() <= 0.05
            && ((x.tempo * at + x.offset) - (y.tempo * at + y.offset)).abs()
                <= self.position_tolerance
    }

    fn update(&mut self, begin: u64, end: u64, scored: &[Scored]) -> Vec<Change> {
        let mut changes = Vec::new();
        for song in 0..self.states.len() {
            let observation = scored
                .iter()
                .find(|s| usize::from(s.candidate.song) == song)
                .map(|&scored| Observation { begin, end, scored });
            let mut state = std::mem::take(&mut self.states[song]);
            if let (Some(mut play), Some(o)) = (state.active, observation) {
                if o.scored.candidate.evidence > 0
                    && o.scored.alignment >= self.hold_alignment
                    && self.agrees(play.last, o)
                {
                    play.last = o;
                    state.active = Some(play);
                    state.pending.clear();
                    self.states[song] = state;
                    continue;
                }
            }
            if let Some(o) = observation.filter(|o| {
                o.scored.candidate.evidence > 0 && o.scored.alignment >= self.start_alignment
            }) {
                let consecutive = state.pending.back().is_some_and(|p| p.end == begin);
                let compatible = state.pending.iter().all(|&p| self.agrees(p, o));
                if !consecutive || !compatible {
                    state.pending.clear();
                }
                state.pending.push_back(o);
                if state.pending.len() > 5 {
                    state.pending.pop_front();
                }
                let evidence = state
                    .pending
                    .iter()
                    .fold(0u32, |n, o| n.saturating_add(o.scored.candidate.evidence));
                if state.pending.len() >= 2 && confidence(evidence, self.half) >= self.threshold {
                    if let Some(play) = state.active.take() {
                        changes.push(Change::End {
                            song: song as u16,
                            play,
                            reason: "trajectory",
                        });
                    }
                    let play = Play {
                        begin: state.pending[0].begin,
                        last: o,
                    };
                    changes.push(Change::Start { play, evidence });
                    state.active = Some(play);
                    state.pending.clear();
                }
            } else {
                state.pending.clear();
            }
            if let Some(play) = state.active {
                if end.saturating_sub(play.last.end) >= self.release {
                    changes.push(Change::End {
                        song: song as u16,
                        play,
                        reason: "release",
                    });
                    state.active = None;
                }
            }
            self.states[song] = state;
        }
        changes
    }

    fn finish(&mut self) -> Vec<Change> {
        self.states
            .iter_mut()
            .enumerate()
            .filter_map(|(song, state)| {
                state.pending.clear();
                state.active.take().map(|play| Change::End {
                    song: song as u16,
                    play,
                    reason: "eof",
                })
            })
            .collect()
    }
}

fn confidence(evidence: u32, half: f64) -> f64 {
    100.0 * f64::from(evidence) / (f64::from(evidence) + half)
}

pub(super) struct Sequence {
    frames_per_observation: f64,
    observation_number: u64,
    begin: u64,
    matcher_config: MatcherConfig,
    decisions: Decisions,
    pub(super) lookups: u64,
    pub(super) matches: u64,
}

impl Sequence {
    pub(super) fn new(opts: &Options, fps: f64, songs: usize) -> Self {
        let frames_per_observation = (opts.sequence_seconds.unwrap() * fps).max(1.0);
        let width = frames_per_observation.ceil() as u64;
        Self {
            frames_per_observation,
            observation_number: 0,
            begin: 0,
            matcher_config: MatcherConfig {
                window_frames: width,
                offset_step: 0.5 * fps,
                half: opts.half,
                ..MatcherConfig::default()
            },
            decisions: Decisions {
                states: vec![State::default(); songs],
                threshold: opts.threshold,
                half: opts.half,
                start_alignment: opts.verify_start,
                hold_alignment: opts.verify_hold,
                position_tolerance: 0.5 * fps,
                release: (opts.release * fps).round() as u64,
            },
            lookups: 0,
            matches: 0,
        }
    }

    /// `horizon` is exclusive: every hash anchored before it is complete.
    /// Queues may contain later anchors; those belong to subsequent windows.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn drain<W: Write>(
        &mut self,
        ctx: &Context<'_>,
        index: &Index,
        peaks: &mut VecDeque<Peak>,
        hashes: &mut VecDeque<Hash>,
        horizon: u64,
        consumed: u64,
        eof: bool,
        out: &mut W,
    ) {
        loop {
            // Round cumulative boundaries: five two-second observations must
            // fit a ten-second excerpt despite a nonintegral frame rate.
            let next =
                ((self.observation_number + 1) as f64 * self.frames_per_observation).round() as u64;
            if next > horizon && !(eof && self.begin < horizon) {
                break;
            }
            let end = next.min(horizon);
            let complete = end == next;
            let mut matcher = Matcher::new(self.matcher_config.clone());
            while hashes.front().is_some_and(|h| h.frame < end) {
                let h = hashes.pop_front().unwrap();
                assert!(
                    h.frame >= self.begin,
                    "hash arrived after its window closed"
                );
                matcher.push(index, &h);
            }
            matcher.advance(end - 1);
            self.lookups += matcher.lookups();
            self.matches += matcher.matches();
            let mut query = Vec::new();
            while peaks.front().is_some_and(|p| p.frame < end) {
                let p = peaks.pop_front().unwrap();
                assert!(
                    p.frame >= self.begin,
                    "peak arrived after its window closed"
                );
                query.push(p);
            }
            let candidates = if ctx.modal_fit {
                matcher.best_per_song_modal()
            } else {
                matcher.best_per_song()
            };
            let mut scored = Vec::new();
            for c in candidates {
                let v = ctx.tracks[usize::from(c.song)].verify(
                    &query,
                    c.shift,
                    c.tempo,
                    c.offset,
                    ctx.verify_frames,
                    ctx.verify_bins,
                );
                scored.push(Scored {
                    candidate: c,
                    confidence: matcher.confidence(c.evidence),
                    alignment: v.query_fraction(),
                });
                writeln!(out,
                    "{{\"event\":\"observation\",\"begin\":{:.6},\"t\":{:.6},\"consumed\":{:.6},\"complete\":{},\"song\":{},\"evidence\":{},\"confidence\":{:.6},\"shift\":{},\"tempo\":{:.8},\"position\":{:.6},{}}}",
                    self.begin as f64 * ctx.seconds_per_frame, end as f64 * ctx.seconds_per_frame,
                    consumed as f64 / ctx.sample_rate, complete,
                    super::json_string(&ctx.names[usize::from(c.song)]), c.evidence,
                    matcher.confidence(c.evidence), c.shift, c.tempo,
                    (c.tempo * end as f64 + c.offset) * ctx.seconds_per_frame,
                    verification_json(&v),
                ).unwrap();
            }
            writeln!(out,
                "{{\"event\":\"window\",\"begin\":{:.6},\"t\":{:.6},\"consumed\":{:.6},\"complete\":{},\"hashes\":{},\"query_peaks\":{}}}",
                self.begin as f64 * ctx.seconds_per_frame, end as f64 * ctx.seconds_per_frame,
                consumed as f64 / ctx.sample_rate, complete, matcher.lookups(), query.len(),
            ).unwrap();
            if complete {
                for change in self.decisions.update(self.begin, end, &scored) {
                    self.write_change(ctx, change, end, consumed, out);
                }
            }
            self.begin = end;
            self.observation_number += 1;
        }
        if eof {
            for change in self.decisions.finish() {
                self.write_change(ctx, change, horizon, consumed, out);
            }
        }
    }

    fn write_change<W: Write>(
        &self,
        ctx: &Context<'_>,
        change: Change,
        end: u64,
        consumed: u64,
        out: &mut W,
    ) {
        let t = end as f64 * ctx.seconds_per_frame;
        let consumed = consumed as f64 / ctx.sample_rate;
        match change {
            Change::Start { play, evidence } => {
                let c: Candidate = play.last.scored.candidate;
                writeln!(out,
                    "{{\"event\":\"start\",\"sequence\":true,\"t\":{t:.6},\"consumed\":{consumed:.6},\"song\":{},\"evidence\":{evidence},\"confidence\":{:.6},\"verify_q\":{:.6},\"shift\":{},\"tempo\":{:.8},\"position\":{:.6},\"estimated_start\":{:.6},\"source_at_estimated_start\":{:.6}}}",
                    super::json_string(&ctx.names[usize::from(c.song)]),
                    confidence(evidence, self.decisions.half), play.last.scored.alignment,
                    c.shift, c.tempo, (c.tempo * end as f64 + c.offset) * ctx.seconds_per_frame,
                    play.begin as f64 * ctx.seconds_per_frame,
                    (c.tempo * play.begin as f64 + c.offset) * ctx.seconds_per_frame,
                ).unwrap();
            }
            Change::End { song, play, reason } => writeln!(out,
                "{{\"event\":\"end\",\"sequence\":true,\"t\":{t:.6},\"consumed\":{consumed:.6},\"song\":{},\"estimated_start\":{:.6},\"estimated_end\":{:.6},\"reason\":\"{reason}\"}}",
                super::json_string(&ctx.names[usize::from(song)]),
                play.begin as f64 * ctx.seconds_per_frame,
                play.last.end as f64 * ctx.seconds_per_frame,
            ).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> Decisions {
        Decisions {
            states: vec![State::default(); 2],
            threshold: 70.0,
            half: 40.0,
            start_alignment: 0.4,
            hold_alignment: 0.3,
            position_tolerance: 50.0,
            release: 300,
        }
    }
    fn score(song: u16, evidence: u32, offset: f64) -> Scored {
        Scored {
            candidate: Candidate {
                song,
                evidence,
                shift: 0,
                tempo: 1.0,
                offset,
            },
            confidence: confidence(evidence, 40.0),
            alignment: 0.8,
        }
    }

    #[test]
    fn independent_weak_observations_accumulate_but_one_strong_does_not_start() {
        let mut d = tracker();
        assert!(d.update(0, 200, &[score(0, 50, 1000.0)]).is_empty());
        let changes = d.update(200, 400, &[score(0, 50, 1000.0)]);
        assert!(matches!(
            changes.as_slice(),
            [Change::Start { evidence: 100, .. }]
        ));
        let mut d = tracker();
        assert!(d.update(0, 200, &[score(0, 10000, 1000.0)]).is_empty());
        assert!(d.finish().is_empty());
    }

    #[test]
    fn gaps_repeated_windows_and_wrong_positions_cannot_combine() {
        for next_begin in [0, 400] {
            let mut d = tracker();
            d.update(0, 200, &[score(0, 60, 1000.0)]);
            assert!(
                d.update(next_begin, next_begin + 200, &[score(0, 60, 1000.0)])
                    .is_empty()
            );
        }
        let mut d = tracker();
        for i in 0..20 {
            assert!(
                d.update(
                    i * 200,
                    (i + 1) * 200,
                    &[score(0, 60, if i % 2 == 0 { 1000.0 } else { 1100.0 })]
                )
                .is_empty()
            );
        }
    }

    #[test]
    fn overlapping_songs_dropout_release_and_repeat() {
        let mut d = tracker();
        let both = [score(0, 60, 1000.0), score(1, 60, 2000.0)];
        d.update(0, 200, &both);
        assert_eq!(d.update(200, 400, &both).len(), 2);
        assert!(d.update(400, 600, &[]).is_empty());
        assert!(d.update(600, 800, &both).is_empty());
        assert!(d.update(800, 1000, &[]).is_empty());
        assert_eq!(d.update(1000, 1200, &[]).len(), 2);
        assert!(d.update(1200, 1400, &both).is_empty());
        assert_eq!(d.update(1400, 1600, &both).len(), 2);
        assert_eq!(d.finish().len(), 2);
        assert!(d.finish().is_empty());
    }

    #[test]
    fn history_is_bounded_and_alignment_gates_every_observation() {
        let mut d = tracker();
        for i in 0..100 {
            assert!(
                d.update(i * 200, (i + 1) * 200, &[score(0, 1, 0.0)])
                    .is_empty()
            );
            assert!(d.states[0].pending.len() <= 5);
        }
        let mut bad = score(0, 1000, 0.0);
        bad.alignment = 0.39;
        assert!(d.update(20000, 20200, &[bad]).is_empty());
        assert!(d.states[0].pending.is_empty());
    }
    #[test]
    fn anchor_partition_preserves_cross_edge_hashes_and_partial_tail() {
        use cqt_monitor::{IndexBuilder, PeakTrack, encode_key};
        let hashes: Vec<Hash> = [199, 200, 399, 400]
            .into_iter()
            .enumerate()
            .map(|(i, frame)| Hash {
                key: encode_key(i as i32 * 20, 10, 16),
                frame,
                bin: 20,
                span: 320,
            })
            .collect();
        let peaks: Vec<Peak> = hashes
            .iter()
            .map(|h| Peak {
                frame: h.frame,
                bin: h.bin,
            })
            .collect();
        let mut builder = IndexBuilder::new();
        builder.add_song("a", 800, hashes.clone());
        let index = builder.build(8);
        let tracks = [PeakTrack::new(peaks.clone())];
        let ctx = Context {
            names: index.names(),
            tracks: &tracks,
            seconds_per_frame: 0.01,
            sample_rate: 100.0,
            delay: 344,
            verify_span: 200,
            verify_frames: 4,
            verify_bins: 1,
            modal_fit: false,
        };
        let opts = Options {
            sequence_seconds: Some(2.0),
            ..Options::default()
        };
        let run = |horizons: &[u64]| {
            let mut s = Sequence::new(&opts, 100.0, 1);
            let mut hp = VecDeque::from(hashes.clone());
            let mut pp = VecDeque::from(peaks.clone());
            let mut out = Vec::new();
            for &h in horizons {
                s.drain(&ctx, &index, &mut pp, &mut hp, h, 1000, h == 450, &mut out);
            }
            assert_eq!(s.lookups, 4);
            assert!(hp.is_empty() && pp.is_empty());
            assert!(s.decisions.states[0].active.is_none());
            String::from_utf8(out).unwrap()
        };
        let output = run(&[450]);
        assert_eq!(output, run(&[199, 200, 399, 400, 450]));
        assert!(output.contains("\"complete\":false"));
        assert_eq!(output.matches("\"event\":\"window\"").count(), 3);
    }

    #[test]
    fn cumulative_rounding_keeps_five_observations_in_ten_seconds() {
        let opts = Options {
            sequence_seconds: Some(2.0),
            ..Options::default()
        };
        let s = Sequence::new(&opts, 44100.0 / 256.0, 0);
        let boundaries: Vec<u64> = (1..=5)
            .map(|i| (f64::from(i) * s.frames_per_observation).round() as u64)
            .collect();
        assert_eq!(boundaries, [345, 689, 1034, 1378, 1723]);
    }

    #[test]
    fn changed_trajectory_requires_confirmation_before_replacing_play() {
        let mut d = tracker();
        d.update(0, 200, &[score(0, 60, 1000.0)]);
        d.update(200, 400, &[score(0, 60, 1000.0)]);
        assert!(d.update(400, 600, &[score(0, 60, 2000.0)]).is_empty());
        let changes = d.update(600, 800, &[score(0, 60, 2000.0)]);
        assert!(matches!(
            changes.as_slice(),
            [
                Change::End {
                    reason: "trajectory",
                    ..
                },
                Change::Start { .. }
            ]
        ));
    }
}
