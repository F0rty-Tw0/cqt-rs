//! Per-song detection state machine: turns the matcher's running
//! candidates into start and end events.

use crate::matcher::Candidate;

/// Tunable parameters of the [`Tracker`].
#[derive(Debug, Clone)]
pub struct TrackerConfig {
    /// Confidence at and above which a song counts as playing.
    pub threshold: f64,
    /// Frames below the threshold after which a play ends.
    pub release_frames: u64,
    /// Frames a disagreeing hypothesis must persist before it is declared
    /// a new play of the same song; at the tail of a play a repeated riff
    /// can briefly win the vote.
    pub confirm_frames: u64,
    /// Shift difference (bins) above which the hypothesis has jumped.
    pub jump_shift: i32,
    /// Tempo difference above which the hypothesis has jumped.
    pub jump_tempo: f64,
    /// Difference (frames) between the predicted and the reported song
    /// position above which the hypothesis has jumped.
    pub jump_frames: f64,
}

/// A detection in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Active {
    /// Query frame of the start event.
    pub start_frame: u64,
    /// Pitch shift in bins when the play started.
    pub shift: i32,
    /// Tempo factor when the play started.
    pub tempo: f64,
    /// Song position in reference frames at `frame`.
    pub position: f64,
    /// Query frame of the last agreeing update.
    pub frame: u64,
    /// Query frame of the last update at or above the threshold.
    pub last_above: u64,
    /// Query frame since which the reported hypothesis has disagreed with
    /// this play, if it does.
    pub disagreeing_since: Option<u64>,
}

/// What the tracker reports.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// A watched song started playing (or a new play of it was detected).
    Start {
        /// Watched song index.
        song: u16,
        /// Query frame of the update.
        frame: u64,
        /// Winning candidate at that update.
        candidate: Candidate,
        /// Its confidence.
        confidence: f64,
        /// Song position in reference frames at `frame`.
        position: f64,
    },
    /// A play ended.
    End {
        /// Watched song index.
        song: u16,
        /// Query frame of the update.
        frame: u64,
        /// Query frame of the matching start event.
        start_frame: u64,
    },
}

/// Tracks, for every watched song, whether it is playing.
#[derive(Debug, Clone)]
pub struct Tracker {
    config: TrackerConfig,
    active: Vec<Option<Active>>,
}

impl Tracker {
    /// Creates a tracker for `num_songs` watched songs.
    pub fn new(num_songs: usize, config: TrackerConfig) -> Self {
        Self {
            config,
            active: vec![None; num_songs],
        }
    }

    /// The configuration in use.
    pub fn config(&self) -> &TrackerConfig {
        &self.config
    }

    /// The play in progress of `song`, if any.
    pub fn active(&self, song: u16) -> Option<&Active> {
        self.active[usize::from(song)].as_ref()
    }

    /// Feeds the candidates of one report at query frame `frame`, each
    /// with its confidence, and emits the resulting events. Songs without
    /// a candidate count as confidence zero.
    pub fn update<F: FnMut(Event)>(
        &mut self,
        frame: u64,
        scored: &[(Candidate, f64)],
        mut on_event: F,
    ) {
        let cfg = &self.config;
        for song in 0..self.active.len() {
            let found = scored
                .iter()
                .find(|(c, _)| usize::from(c.song) == song)
                .copied();
            let above = found.is_some_and(|(_, confidence)| confidence >= cfg.threshold);
            match (self.active[song], found) {
                (Some(mut a), Some((c, confidence))) if above => {
                    let position = c.tempo * frame as f64 + c.offset;
                    let predicted = a.position + a.tempo * (frame - a.frame) as f64;
                    let jumped = (c.shift - a.shift).abs() > cfg.jump_shift
                        || (c.tempo - a.tempo).abs() > cfg.jump_tempo
                        || (position - predicted).abs() > cfg.jump_frames;
                    a.last_above = frame;
                    if !jumped {
                        a.disagreeing_since = None;
                        a.position = position;
                        a.frame = frame;
                        self.active[song] = Some(a);
                        continue;
                    }
                    let since = *a.disagreeing_since.get_or_insert(frame);
                    if frame - since < cfg.confirm_frames {
                        self.active[song] = Some(a);
                        continue;
                    }
                    on_event(Event::End {
                        song: song as u16,
                        frame,
                        start_frame: a.start_frame,
                    });
                    self.active[song] = Some(Self::start(frame, c, confidence, &mut on_event));
                }
                (None, Some((c, confidence))) if above => {
                    self.active[song] = Some(Self::start(frame, c, confidence, &mut on_event));
                }
                (Some(a), _) if frame - a.last_above >= cfg.release_frames => {
                    on_event(Event::End {
                        song: song as u16,
                        frame,
                        start_frame: a.start_frame,
                    });
                    self.active[song] = None;
                }
                _ => {}
            }
        }
    }

    /// Ends every play in progress at query frame `frame` (end of stream).
    pub fn finish<F: FnMut(Event)>(&mut self, frame: u64, mut on_event: F) {
        for (song, slot) in self.active.iter_mut().enumerate() {
            if let Some(a) = slot.take() {
                on_event(Event::End {
                    song: song as u16,
                    frame,
                    start_frame: a.start_frame,
                });
            }
        }
    }

    fn start<F: FnMut(Event)>(
        frame: u64,
        c: Candidate,
        confidence: f64,
        on_event: &mut F,
    ) -> Active {
        let position = c.tempo * frame as f64 + c.offset;
        on_event(Event::Start {
            song: c.song,
            frame,
            candidate: c,
            confidence,
            position,
        });
        Active {
            start_frame: frame,
            shift: c.shift,
            tempo: c.tempo,
            position,
            frame,
            last_above: frame,
            disagreeing_since: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> TrackerConfig {
        TrackerConfig {
            threshold: 70.0,
            release_frames: 30,
            confirm_frames: 10,
            jump_shift: 2,
            jump_tempo: 0.05,
            jump_frames: 30.0,
        }
    }

    fn candidate(song: u16, shift: i32, tempo: f64, offset: f64) -> Candidate {
        Candidate {
            song,
            evidence: 300,
            shift,
            tempo,
            offset,
        }
    }

    fn run(tracker: &mut Tracker, frame: u64, scored: &[(Candidate, f64)]) -> Vec<Event> {
        let mut events = Vec::new();
        tracker.update(frame, scored, |e| events.push(e));
        events
    }

    #[test]
    fn starts_above_the_threshold_and_ends_after_the_release() {
        let mut tracker = Tracker::new(2, config());
        let c = candidate(1, 0, 1.0, 500.0);
        assert!(run(&mut tracker, 0, &[(c, 50.0)]).is_empty());
        let events = run(&mut tracker, 10, &[(c, 80.0)]);
        assert!(matches!(
            events[..],
            [Event::Start { song: 1, frame: 10, confidence, position, .. }]
                if confidence == 80.0 && position == 510.0
        ));
        assert!(tracker.active(1).is_some());
        assert!(tracker.active(0).is_none());
        // Below the threshold for less than the release: still playing.
        assert!(run(&mut tracker, 20, &[(c, 30.0)]).is_empty());
        assert!(run(&mut tracker, 39, &[]).is_empty());
        let events = run(&mut tracker, 40, &[]);
        assert_eq!(
            events,
            [Event::End {
                song: 1,
                frame: 40,
                start_frame: 10
            }]
        );
        assert!(tracker.active(1).is_none());
    }

    #[test]
    fn a_persistent_jump_starts_a_new_play_but_a_brief_one_does_not() {
        let mut tracker = Tracker::new(1, config());
        let c = candidate(0, 0, 1.0, 500.0);
        run(&mut tracker, 0, &[(c, 90.0)]);
        // The same play continues at the predicted position.
        assert!(run(&mut tracker, 5, &[(c, 90.0)]).is_empty());
        // A riff 200 frames earlier wins for a moment.
        let riff = candidate(0, 0, 1.0, 300.0);
        assert!(run(&mut tracker, 6, &[(riff, 90.0)]).is_empty());
        assert!(run(&mut tracker, 12, &[(riff, 90.0)]).is_empty());
        // Back to the original hypothesis: no new play, disagreement reset.
        assert!(run(&mut tracker, 13, &[(c, 90.0)]).is_empty());
        assert_eq!(tracker.active(0).unwrap().disagreeing_since, None);
        // Now a persistent jump: the song restarted from the top.
        let restart = candidate(0, 0, 1.0, -20.0);
        assert!(run(&mut tracker, 20, &[(restart, 90.0)]).is_empty());
        assert!(run(&mut tracker, 29, &[(restart, 90.0)]).is_empty());
        let events = run(&mut tracker, 30, &[(restart, 90.0)]);
        assert!(matches!(
            events[..],
            [
                Event::End { song: 0, frame: 30, start_frame: 0 },
                Event::Start { song: 0, frame: 30, position, .. }
            ] if position == 10.0
        ));
        assert_eq!(tracker.active(0).unwrap().start_frame, 30);
    }

    #[test]
    fn shift_and_tempo_changes_count_as_jumps() {
        let mut tracker = Tracker::new(1, config());
        run(&mut tracker, 0, &[(candidate(0, 0, 1.0, 0.0), 90.0)]);
        // Disagreement from frame 1 on is confirmed 10 frames later.
        for frame in 1..=11 {
            run(&mut tracker, frame, &[(candidate(0, 3, 1.0, 0.0), 90.0)]);
        }
        assert_eq!(tracker.active(0).unwrap().start_frame, 11);
        for frame in 12..=22 {
            run(&mut tracker, frame, &[(candidate(0, 3, 1.1, 0.0), 90.0)]);
        }
        assert_eq!(tracker.active(0).unwrap().start_frame, 22);
        // Within tolerance: no jump.
        for frame in 23..=40 {
            run(&mut tracker, frame, &[(candidate(0, 4, 1.12, 0.0), 90.0)]);
        }
        assert_eq!(tracker.active(0).unwrap().start_frame, 22);
    }

    #[test]
    fn finish_ends_every_play() {
        let mut tracker = Tracker::new(3, config());
        run(
            &mut tracker,
            0,
            &[
                (candidate(0, 0, 1.0, 0.0), 90.0),
                (candidate(2, 0, 1.0, 0.0), 95.0),
            ],
        );
        let mut events = Vec::new();
        tracker.finish(7, |e| events.push(e));
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|e| matches!(e, Event::End { frame: 7, .. }))
        );
        assert!((0..3).all(|s| tracker.active(s).is_none()));
    }
}
