//! End-to-end detection on synthetic audio: two watched songs, a stream
//! of noise, an unwatched song and a pitched-up, sped-up play of one
//! watched song. This is the monitor's release gate: it runs the same
//! chain as the `monitor` binary without any audio file.

use std::f64::consts::TAU;

use cqt_monitor::{
    Event, FingerprintConfig, Fingerprinter, IndexBuilder, Matcher, MatcherConfig, PeakTrack,
    Scored, Tracker, TrackerConfig, fingerprint,
};
use cqt_rs::{Cqt, CqtParams, CqtStream};

const SR: u32 = 44_100;
const HOP: usize = 256;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A "song": random notes from a chromatic scale with four harmonics and
/// a decaying envelope, at random onsets.
fn song(seed: u64, seconds: f64) -> Vec<f32> {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let n = (seconds * f64::from(SR)) as usize;
    let mut out = vec![0.0f32; n];
    let mut onset = 0.0;
    while onset < seconds {
        for _ in 0..2 {
            let f0 = 110.0 * 2f64.powf((rng.next() % 48) as f64 / 12.0);
            let duration = 0.3 + 0.4 * rng.uniform();
            let start = (onset * f64::from(SR)) as usize;
            let len = ((duration * f64::from(SR)) as usize).min(n - start);
            for i in 0..len {
                let t = i as f64 / f64::from(SR);
                let env = (-t / 0.25).exp() * (1.0 - (-t / 0.005).exp());
                let mut v = 0.0;
                for h in 1..=4 {
                    v += (TAU * f0 * h as f64 * t).sin() / h as f64;
                }
                out[start + i] += (0.12 * env * v) as f32;
            }
        }
        onset += 0.1 + 0.25 * rng.uniform();
    }
    out
}

fn noise(seed: u64, seconds: f64, amplitude: f32) -> Vec<f32> {
    let mut rng = Rng(seed | 1);
    (0..(seconds * f64::from(SR)) as usize)
        .map(|_| (rng.uniform() * 2.0 - 1.0) as f32 * amplitude)
        .collect()
}

/// `signal[from..]` resampled so that it plays `speed` times faster and
/// higher, `seconds` of output.
fn pitch_fader(signal: &[f32], from: f64, seconds: f64, speed: f64) -> Vec<f32> {
    let n = (seconds * f64::from(SR)) as usize;
    (0..n)
        .map(|i| {
            let pos = from * f64::from(SR) + i as f64 * speed;
            let k = pos as usize;
            let frac = (pos - k as f64) as f32;
            match (signal.get(k), signal.get(k + 1)) {
                (Some(&a), Some(&b)) => a + (b - a) * frac,
                _ => 0.0,
            }
        })
        .collect()
}

#[test]
fn detects_a_pitched_and_sped_up_play_and_nothing_else() {
    let cqt = Cqt::new(
        CqtParams::builder(SR, 55.0, 7_040.0)
            .bins_per_octave(24)
            .build()
            .unwrap(),
    );
    let fp_config = FingerprintConfig::default();
    let frames_per_second = f64::from(SR) / HOP as f64;

    // Watch list: song A and song C.
    let song_a = song(1, 45.0);
    let song_c = song(3, 40.0);
    let mut builder = IndexBuilder::new();
    let mut tracks = Vec::new();
    for s in [&song_a, &song_c] {
        let (frames, peaks, hashes) = fingerprint(&cqt, HOP, s, &fp_config).unwrap();
        assert!(peaks.len() > 300, "{} peaks", peaks.len());
        builder.add_song("song", frames as u32, hashes);
        tracks.push(PeakTrack::new(peaks));
    }
    let index = builder.build(8);

    // Stream: noise, song B (unwatched), song A from 10 s on at 1.04×
    // (pitch fader +4 %), noise.
    let speed = 1.04;
    let mut stream = noise(7, 6.0, 0.02);
    stream.extend(song(2, 12.0));
    let play_start = stream.len() as f64 / f64::from(SR);
    stream.extend(pitch_fader(&song_a, 10.0, 20.0, speed));
    let play_end = stream.len() as f64 / f64::from(SR);
    stream.extend(noise(8, 6.0, 0.02));

    let matcher_config = MatcherConfig {
        window_frames: (5.0 * frames_per_second).round() as u64,
        offset_step: 0.5 * frames_per_second,
        ..MatcherConfig::default()
    };
    let mut matcher = Matcher::new(matcher_config.clone());
    let mut tracker = Tracker::new(
        2,
        TrackerConfig {
            threshold: 70.0,
            release_frames: (3.0 * frames_per_second).round() as u64,
            confirm_frames: frames_per_second.round() as u64,
            jump_shift: 2,
            jump_tempo: 0.05,
            jump_frames: 3.0 * frames_per_second,
            start_alignment: 0.4,
            hold_alignment: 0.3,
            hold_confidence: 35.0,
            min_play_frames: 0,
        },
    );
    let mut fp = Fingerprinter::new(cqt.num_bins(), &fp_config);
    let delay = fp.delay_frames();
    let mut cqt_stream = CqtStream::new(&cqt, HOP).unwrap();
    let report_every = (0.25 * frames_per_second).round() as u64;
    let mut frames = 0u64;
    let mut recent = std::collections::VecDeque::new();
    let mut events: Vec<(f64, Event)> = Vec::new();
    let mut max_confidence_outside = 0.0f64;

    for block in stream.chunks(4096) {
        let (fp_ref, matcher_ref, frames_ref, recent_ref) =
            (&mut fp, &mut matcher, &mut frames, &mut recent);
        let mut report = false;
        cqt_stream.push(&cqt, block, |magnitudes| {
            fp_ref.push(
                magnitudes,
                |p| recent_ref.push_back(p),
                |h| matcher_ref.push(&index, &h),
            );
            *frames_ref += 1;
            report |= frames_ref.is_multiple_of(report_every);
        });
        if !report {
            continue;
        }
        let frame = frames - 1;
        let t = frame as f64 / frames_per_second;
        matcher.advance(frame.saturating_sub(delay));
        let horizon = frame.saturating_sub((2.0 * frames_per_second) as u64);
        while recent.front().is_some_and(|p| p.frame < horizon) {
            recent.pop_front();
        }
        let query = recent.make_contiguous();
        let scored: Vec<Scored> = matcher
            .best_per_song()
            .into_iter()
            .map(|c| Scored {
                candidate: c,
                confidence: matcher.confidence(c.evidence),
                alignment: tracks[usize::from(c.song)]
                    .verify(query, c.shift, c.tempo, c.offset, 4, 1)
                    .query_fraction(),
            })
            .collect();
        // Reports whose evidence window lies entirely outside the play.
        let window_start = t - delay as f64 / frames_per_second - 5.0;
        if t < play_start || window_start > play_end {
            for s in &scored {
                max_confidence_outside = max_confidence_outside.max(s.confidence);
            }
        }
        tracker.update(frame, &scored, |e| events.push((t, e)));
    }
    tracker.finish(frames, |e| {
        events.push((frames as f64 / frames_per_second, e))
    });

    let starts: Vec<_> = events
        .iter()
        .filter(|(_, e)| matches!(e, Event::Start { .. }))
        .collect();
    assert_eq!(starts.len(), 1, "starts: {starts:?}");
    let (
        t_start,
        Event::Start {
            song,
            candidate,
            position,
            ..
        },
    ) = *starts[0]
    else {
        unreachable!()
    };
    assert_eq!(song, 0, "song A is watched song 0");
    assert!(
        t_start >= play_start && t_start <= play_start + 5.0,
        "start at {t_start:.2} s, play from {play_start:.2} s"
    );
    assert!(
        (candidate.tempo - speed).abs() < 0.02,
        "tempo {}",
        candidate.tempo
    );
    let expected_shift = 24.0 * speed.log2();
    assert!(
        (f64::from(candidate.shift) - expected_shift).abs() < 1.0,
        "shift {} expected {expected_shift:.2}",
        candidate.shift
    );
    let expected_position = 10.0 + (t_start - play_start) * speed;
    let position_seconds = position / frames_per_second;
    assert!(
        (position_seconds - expected_position).abs() < 0.5,
        "position {position_seconds:.2} s expected {expected_position:.2} s"
    );
    let ends: Vec<_> = events
        .iter()
        .filter(|(_, e)| matches!(e, Event::End { .. }))
        .collect();
    assert_eq!(ends.len(), 1);
    assert!(
        ends[0].0 >= play_end && ends[0].0 <= play_end + 10.0,
        "end at {:.2} s, play until {play_end:.2} s",
        ends[0].0
    );
    assert!(
        max_confidence_outside < 50.0,
        "confidence {max_confidence_outside:.1} outside the play"
    );
}
