//! Long retrieval context, with decisions driven by new disjoint peak intervals.
use super::{Context, Options, json_string, verification_json};
use cqt_monitor::{Candidate, Hash, Index, Matcher, MatcherConfig, Peak, Scored, Verification};
use std::collections::VecDeque;
use std::io::Write;

#[derive(Clone, Copy, Debug)]
struct Hypothesis {
    scored: Scored,
    begin: u64,
    end: u64,
    observations: u8,
}

#[derive(Default)]
struct State {
    pending: Vec<Hypothesis>,
    active: Option<Hypothesis>,
}

#[derive(Debug)]
enum Change {
    Start(Hypothesis),
    End(Hypothesis, &'static str),
}

struct Decisions {
    states: Vec<State>,
    budget: usize,
    threshold: f64,
    start_alignment: f64,
    hold_alignment: f64,
    position_tolerance: f64,
    release: u64,
}

impl Decisions {
    fn agrees(&self, a: Candidate, b: Candidate, at: u64) -> bool {
        a.song == b.song
            && (a.shift - b.shift).abs() <= 2
            && (a.tempo - b.tempo).abs() <= 0.05
            && ((a.tempo - b.tempo) * at as f64 + a.offset - b.offset).abs()
                <= self.position_tolerance
    }

    /// `verify` must use only this interval's peaks. In particular, check the
    /// OLD trajectory before allowing a new fit to replace its prediction.
    fn update(
        &mut self,
        begin: u64,
        end: u64,
        retrieved: &[Scored],
        mut verify: impl FnMut(Candidate) -> f64,
    ) -> Vec<Change> {
        let mut changes = Vec::new();
        for song in 0..self.states.len() {
            let mut state = std::mem::take(&mut self.states[song]);
            let current: Vec<_> = retrieved
                .iter()
                .filter(|s| usize::from(s.candidate.song) == song)
                .copied()
                .collect();
            if let Some(mut play) = state.active {
                let fresh = verify(play.scored.candidate);
                if fresh >= self.hold_alignment {
                    // Refit only a compatible trajectory that also aligns now.
                    if let Some(s) = current.iter().find(|s| {
                        self.agrees(play.scored.candidate, s.candidate, end)
                            && s.alignment >= self.hold_alignment
                    }) {
                        play.scored = *s;
                    } else {
                        play.scored.confidence = 0.0;
                        play.scored.candidate.evidence = 0;
                        play.scored.alignment = fresh;
                    }
                    play.end = end;
                    state.active = Some(play);
                    state.pending.clear();
                    self.states[song] = state;
                    continue;
                }
            }
            let mut pending = Vec::new();
            for mut p in state.pending.drain(..) {
                if p.end != begin || verify(p.scored.candidate) < self.start_alignment {
                    continue;
                }
                // Current rolling confidence is read once, never accumulated.
                if let Some(s) = current.iter().find(|s| {
                    self.agrees(p.scored.candidate, s.candidate, end)
                        && s.alignment >= self.start_alignment
                }) {
                    p.scored = *s;
                } else {
                    p.scored.confidence = 0.0;
                    p.scored.candidate.evidence = 0;
                }
                p.end = end;
                p.observations = p.observations.saturating_add(1).min(5);
                pending.push(p);
            }
            for s in current
                .iter()
                .filter(|s| s.alignment >= self.start_alignment)
            {
                if !pending
                    .iter()
                    .any(|p| self.agrees(p.scored.candidate, s.candidate, end))
                {
                    pending.push(Hypothesis {
                        scored: *s,
                        begin,
                        end,
                        observations: 1,
                    });
                }
            }
            pending.sort_by(|a, b| {
                b.observations
                    .cmp(&a.observations)
                    .then_with(|| b.scored.confidence.total_cmp(&a.scored.confidence))
                    .then_with(|| b.scored.alignment.total_cmp(&a.scored.alignment))
                    .then_with(|| {
                        a.scored
                            .candidate
                            .offset
                            .total_cmp(&b.scored.candidate.offset)
                    })
            });
            if let Some(&p) = pending
                .iter()
                .find(|p| p.observations >= 2 && p.scored.confidence >= self.threshold)
            {
                if let Some(play) = state.active.take() {
                    changes.push(Change::End(play, "trajectory"));
                }
                changes.push(Change::Start(p));
                state.active = Some(p);
                pending.clear();
            }
            pending.truncate(self.budget);
            state.pending = pending;
            if let Some(play) = state.active
                && end.saturating_sub(play.end) >= self.release
            {
                changes.push(Change::End(play, "release"));
                state.active = None;
            }
            self.states[song] = state;
        }
        changes
    }

    fn finish(&mut self) -> Vec<Change> {
        self.states
            .iter_mut()
            .filter_map(|s| {
                s.pending.clear();
                s.active.take().map(|p| Change::End(p, "eof"))
            })
            .collect()
    }
}

pub(super) struct Continuation {
    frames_per_observation: f64,
    observation_number: u64,
    begin: u64,
    matcher: Matcher,
    decisions: Decisions,
    pairs: Option<super::pairs::Retrieval>,
}

impl Continuation {
    pub(super) fn new(opts: &Options, fps: f64, songs: usize) -> Self {
        Self {
            frames_per_observation: (opts.continuation_seconds.unwrap() * fps).max(1.0),
            observation_number: 0,
            pairs: opts
                .pair_fallback
                .then(|| super::pairs::Retrieval::new(fps)),
            begin: 0,
            matcher: Matcher::new(MatcherConfig {
                window_frames: (opts.window * fps).round().max(1.0) as u64,
                offset_step: 0.5 * fps,
                half: opts.half,
                ..MatcherConfig::default()
            }),
            decisions: Decisions {
                states: (0..songs).map(|_| State::default()).collect(),
                budget: opts.continuation_hypotheses,
                threshold: opts.threshold,
                start_alignment: opts.verify_start,
                hold_alignment: opts.verify_hold,
                position_tolerance: 0.5 * fps,
                release: (opts.release * fps).round() as u64,
            },
        }
    }

    pub(super) fn lookups(&self) -> u64 {
        self.matcher.lookups()
    }
    pub(super) fn matches(&self) -> u64 {
        self.matcher.matches()
    }

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
            let next =
                ((self.observation_number + 1) as f64 * self.frames_per_observation).round() as u64;
            if next > horizon && !(eof && self.begin < horizon) {
                break;
            }
            let end = next.min(horizon);
            let complete = end == next;
            let before = self.matcher.lookups();
            while hashes.front().is_some_and(|h| h.frame < end) {
                let h = hashes.pop_front().unwrap();
                assert!(h.frame >= self.begin, "late hash after closed interval");
                self.matcher.push(index, &h);
            }
            self.matcher.advance(end - 1);
            let mut query = Vec::new();
            while peaks.front().is_some_and(|p| p.frame < end) {
                let p = peaks.pop_front().unwrap();
                assert!(p.frame >= self.begin, "late peak after closed interval");
                query.push(p);
            }
            let verify = |c: Candidate| {
                ctx.tracks[usize::from(c.song)].verify(
                    &query,
                    c.shift,
                    c.tempo,
                    c.offset,
                    ctx.verify_frames,
                    ctx.verify_bins,
                )
            };
            let mut scored = Vec::new();
            for c in self
                .matcher
                .hypotheses_per_song(self.decisions.budget, ctx.modal_fit)
            {
                let v = verify(c);
                let s = Scored {
                    candidate: c,
                    confidence: self.matcher.confidence(c.evidence),
                    alignment: v.query_fraction(),
                };
                self.write_observation(ctx, s, v, "retrieval", end, consumed, complete, out);
                scored.push(s);
            }
            if let (Some(pairs), Some(pair_index)) = (&mut self.pairs, ctx.pair_index) {
                pairs.extend(&query, end);
                let mut eligible = vec![true; ctx.names.len()];
                for s in &scored {
                    if s.confidence >= self.decisions.threshold {
                        eligible[usize::from(s.candidate.song)] = false;
                    }
                }
                let (proposals, stats) = pairs.retrieve(pair_index, end, &eligible);
                writeln!(out, "{{\"event\":\"pair_budget\",\"t\":{:.6},\"peaks\":{},\"peak_overflow\":{},\"pairs\":{},\"pair_limit\":{},\"occurrences\":{},\"collision_ranges\":{},\"collision_entries\":{},\"votes\":{},\"vote_limit\":{},\"cells\":{},\"fits\":{},\"candidates\":{}}}",
                    end as f64 * ctx.seconds_per_frame, stats.peaks, stats.peak_overflow,
                    stats.pairs, stats.pair_limit, stats.occurrences, stats.collision_ranges,
                    stats.collision_entries, stats.votes, stats.vote_limit, stats.cells,
                    stats.fits, stats.candidates).unwrap();
                for c in proposals {
                    let v = verify(c);
                    // Admission score after the independent anchor gate. Never
                    // add pair counts to triplet evidence/confidence.
                    let s = Scored {
                        candidate: c,
                        confidence: self.decisions.threshold,
                        alignment: v.query_fraction(),
                    };
                    self.write_observation(
                        ctx,
                        s,
                        v,
                        "pair_retrieval",
                        end,
                        consumed,
                        complete,
                        out,
                    );
                    scored.push(s);
                }
                prioritize_retrieval(&mut scored);
            }
            // Emit the actual carried predictions checked by the state machine,
            // including predictions with no remaining hash retrieval support.
            for state in &self.decisions.states {
                for (h, origin) in state
                    .active
                    .iter()
                    .map(|p| (p, "active"))
                    .chain(state.pending.iter().map(|p| (p, "pending")))
                {
                    let c = h.scored.candidate;
                    let v = verify(c);
                    self.write_observation(
                        ctx,
                        Scored {
                            candidate: Candidate { evidence: 0, ..c },
                            confidence: 0.0,
                            alignment: v.query_fraction(),
                        },
                        v,
                        origin,
                        end,
                        consumed,
                        complete,
                        out,
                    );
                }
            }
            writeln!(out,
                "{{\"event\":\"window\",\"continuation\":true,\"begin\":{:.6},\"t\":{:.6},\"consumed\":{:.6},\"complete\":{},\"hashes\":{},\"query_peaks\":{},\"retrieval_votes\":{}}}",
                self.begin as f64 * ctx.seconds_per_frame, end as f64 * ctx.seconds_per_frame,
                consumed as f64 / ctx.sample_rate, complete, self.matcher.lookups() - before,
                query.len(), self.matcher.votes_in_window(),
            ).unwrap();
            if complete {
                for change in self
                    .decisions
                    .update(self.begin, end, &scored, |c| verify(c).query_fraction())
                {
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

    #[allow(clippy::too_many_arguments)]
    fn write_observation<W: Write>(
        &self,
        ctx: &Context<'_>,
        s: Scored,
        v: Verification,
        origin: &str,
        end: u64,
        consumed: u64,
        complete: bool,
        out: &mut W,
    ) {
        let c = s.candidate;
        writeln!(out,
            "{{\"event\":\"observation\",\"continuation\":true,\"origin\":\"{origin}\",\"begin\":{:.6},\"t\":{:.6},\"consumed\":{:.6},\"complete\":{},\"song\":{},\"evidence\":{},\"confidence\":{:.6},\"shift\":{},\"tempo\":{:.8},\"position\":{:.6},{}}}",
            self.begin as f64 * ctx.seconds_per_frame, end as f64 * ctx.seconds_per_frame,
            consumed as f64 / ctx.sample_rate, complete, json_string(&ctx.names[usize::from(c.song)]),
            c.evidence, s.confidence, c.shift, c.tempo,
            (c.tempo * end as f64 + c.offset) * ctx.seconds_per_frame, verification_json(&v),
        ).unwrap();
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
            Change::Start(p) => {
                let c = p.scored.candidate;
                writeln!(out,
                    "{{\"event\":\"start\",\"continuation\":true,\"t\":{t:.6},\"consumed\":{consumed:.6},\"song\":{},\"evidence\":{},\"confidence\":{:.6},\"verify_q\":{:.6},\"shift\":{},\"tempo\":{:.8},\"position\":{:.6},\"estimated_start\":{:.6},\"source_at_estimated_start\":{:.6},\"observations\":{}}}",
                    json_string(&ctx.names[usize::from(c.song)]), c.evidence, p.scored.confidence,
                    p.scored.alignment, c.shift, c.tempo, (c.tempo * end as f64 + c.offset) * ctx.seconds_per_frame,
                    p.begin as f64 * ctx.seconds_per_frame,
                    (c.tempo * p.begin as f64 + c.offset) * ctx.seconds_per_frame, p.observations,
                ).unwrap();
            }
            Change::End(p, reason) => writeln!(out,
                "{{\"event\":\"end\",\"continuation\":true,\"t\":{t:.6},\"consumed\":{consumed:.6},\"song\":{},\"estimated_start\":{:.6},\"estimated_end\":{:.6},\"reason\":\"{reason}\"}}",
                json_string(&ctx.names[usize::from(p.scored.candidate.song)]),
                p.begin as f64 * ctx.seconds_per_frame, p.end as f64 * ctx.seconds_per_frame,
            ).unwrap(),
        }
    }
}

fn prioritize_retrieval(scored: &mut [Scored]) {
    // The state machine selects the first compatible current trajectory.
    // Preserve stable triplet ordering, but let an admitted pair supersede
    // a below-threshold triplet for its eligible song.
    scored.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tracker(budget: usize) -> Decisions {
        Decisions {
            states: vec![State::default(), State::default()],
            budget,
            threshold: 70.0,
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
            confidence: 100.0 * f64::from(evidence) / (f64::from(evidence) + 40.0),
            alignment: 0.8,
        }
    }
    #[test]
    fn admitted_pair_outranks_a_weak_triplet_but_still_needs_fresh_checks() {
        let weak = score(0, 20, 1000.0);
        let mut pair = score(0, 6, 1000.0);
        pair.confidence = 70.0;
        let mut current = [weak, pair];
        prioritize_retrieval(&mut current);
        let mut t = tracker(3);
        assert!(t.update(0, 200, &current, |_| 0.8).is_empty());
        assert!(matches!(
            t.update(200, 400, &current, |_| 0.8).as_slice(),
            [Change::Start(_)]
        ));
        let mut t = tracker(3);
        assert!(t.update(0, 200, &current, |_| 0.8).is_empty());
        assert!(t.update(200, 400, &current, |_| 0.0).is_empty());
        let mut strong = score(1, 200, 2000.0);
        strong.alignment = 0.8;
        let mut priority = [weak, pair, strong];
        prioritize_retrieval(&mut priority);
        assert_eq!(priority[0].candidate.song, 1);
    }

    #[test]
    fn continuous_anchors_keep_long_context_across_partition_and_partial_tail() {
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
        let peaks: Vec<_> = hashes
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
            pair_index: None,
            seconds_per_frame: 0.01,
            sample_rate: 100.0,
            delay: 344,
            verify_span: 200,
            verify_frames: 4,
            verify_bins: 1,
            modal_fit: true,
        };
        let opts = Options {
            continuation_seconds: Some(2.0),
            window: 10.0,
            ..Options::default()
        };
        let run = |horizons: &[u64]| {
            let mut s = Continuation::new(&opts, 100.0, 1);
            let mut hp = VecDeque::from(hashes.clone());
            let mut pp = VecDeque::from(peaks.clone());
            let mut out = Vec::new();
            for &h in horizons {
                s.drain(&ctx, &index, &mut pp, &mut hp, h, 1000, h == 450, &mut out);
            }
            assert_eq!(s.lookups(), 4);
            assert_eq!(s.matcher.votes_in_window(), 4);
            assert!(hp.is_empty() && pp.is_empty());
            assert!(s.decisions.states[0].active.is_none());
            String::from_utf8(out).unwrap()
        };
        let output = run(&[450]);
        assert_eq!(output, run(&[199, 200, 399, 400, 450]));
        assert_eq!(output.matches("\"event\":\"window\"").count(), 3);
        assert!(output.contains("\"complete\":false"));
        assert!(!output.contains("\"event\":\"start\""));
        let s = Continuation::new(&opts, 44100.0 / 256.0, 0);
        assert_eq!((5.0 * s.frames_per_observation).round() as u64, 1723);
    }

    #[test]
    fn overlapping_retrieval_never_accumulates_confidence() {
        let mut d = tracker(3);
        for i in 0..30 {
            assert!(
                d.update(i * 200, (i + 1) * 200, &[score(0, 60, 1000.0)], |_| 0.8)
                    .is_empty()
            );
        }
        assert!(matches!(
            d.update(6000, 6200, &[score(0, 100, 1000.0)], |_| 0.8)
                .as_slice(),
            [Change::Start(_)]
        ));
    }
    #[test]
    fn fresh_prediction_required_before_refitting_or_confirming() {
        let mut d = tracker(3);
        let s = [score(0, 100, 1000.0)];
        assert!(d.update(0, 200, &s, |_| 0.8).is_empty());
        // A newly fitted trajectory looks good, but the old prediction fails.
        assert!(d.update(200, 400, &s, |_| 0.0).is_empty());
        assert!(matches!(
            d.update(400, 600, &s, |_| 0.8).as_slice(),
            [Change::Start(_)]
        ));
    }
    #[test]
    fn stale_high_scores_do_not_renew_or_restart_and_empty_retrieval_can_hold() {
        let mut d = tracker(3);
        let mut s = score(0, 1000, 1000.0);
        d.update(0, 200, &[s], |_| 0.8);
        d.update(200, 400, &[s], |_| 0.8);
        assert!(d.update(400, 600, &[], |_| 0.8).is_empty());
        s.alignment = 0.0;
        assert!(d.update(600, 800, &[s], |_| 0.0).is_empty());
        assert!(
            matches!(d.update(800, 1000, &[s], |_| 0.0).as_slice(), [Change::End(p, "release")] if p.end == 600)
        );
        assert!(d.update(1000, 1200, &[s], |_| 0.0).is_empty());
    }
    #[test]
    fn competing_positions_survive_until_continuation_disambiguates() {
        let mut d = tracker(3);
        let a = score(0, 130, 1000.0);
        let b = score(0, 100, 2000.0);
        d.update(0, 200, &[a, b], |_| 0.8);
        assert_eq!(d.states[0].pending.len(), 2);
        let mut bad = a;
        bad.alignment = 0.0;
        assert!(
            matches!(d.update(200, 400, &[bad, b], |c| if c.offset == 2000.0 { 0.8 } else { 0.0 }).as_slice(),
            [Change::Start(p)] if p.scored.candidate.offset == 2000.0)
        );
    }
    #[test]
    fn gaps_repeated_intervals_overlap_dropout_and_eof() {
        let both = [score(0, 100, 1000.0), score(1, 100, 2000.0)];
        for begin in [0, 400] {
            let mut d = tracker(3);
            d.update(0, 200, &both, |_| 0.8);
            assert!(d.update(begin, begin + 200, &both, |_| 0.8).is_empty());
        }
        let mut d = tracker(3);
        d.update(0, 200, &both, |_| 0.8);
        assert_eq!(d.update(200, 400, &both, |_| 0.8).len(), 2);
        assert!(d.update(400, 600, &[], |_| 0.0).is_empty());
        assert!(d.update(600, 800, &[], |_| 0.8).is_empty());
        assert_eq!(d.finish().len(), 2);
        assert!(d.finish().is_empty());
    }
    #[test]
    fn pending_storage_is_bounded_and_trajectory_changes_need_confirmation() {
        let mut d = tracker(3);
        let many: Vec<_> = (0..20)
            .map(|i| score(0, 1, f64::from(i) * 1000.0))
            .collect();
        for i in 0..20 {
            d.update(i * 200, (i + 1) * 200, &many, |_| 0.8);
            assert!(d.states[0].pending.len() <= 3);
        }
        let mut d = tracker(3);
        d.update(0, 200, &[score(0, 100, 1000.0)], |_| 0.8);
        d.update(200, 400, &[score(0, 100, 1000.0)], |_| 0.8);
        assert!(
            d.update(400, 600, &[score(0, 100, 2000.0)], |_| 0.0)
                .is_empty()
        );
        let changes = d.update(600, 800, &[score(0, 100, 2000.0)], |c| {
            if c.offset == 2000.0 { 0.8 } else { 0.0 }
        });
        assert!(matches!(
            changes.as_slice(),
            [Change::End(_, "trajectory"), Change::Start(_)]
        ));
    }
}
