//! Watch-list monitor: fingerprints the watched songs, runs a stream through
//! the same pipeline and prints JSON lines with the running match state.
//!
//! ```console
//! monitor --watch song.wav [--watch other.wav ...] --stream radio.wav [options]
//! monitor fingerprint file.wav      # peaks and hashes of one file, for validation
//! ```

use std::collections::VecDeque;
use std::io::{BufWriter, Write};
use std::process::exit;
use std::time::Instant;

use cqt_monitor::{
    Event, FingerprintConfig, Fingerprinter, Hash, Index, IndexBuilder, Matcher, MatcherConfig,
    Peak, PeakTrack, Scored, Tracker, TrackerConfig, Verification, fingerprint,
};
use cqt_rs::{Cqt, CqtParams, CqtStream};

const USAGE: &str = "usage:
  monitor --watch NAME=FILE.wav [--watch ...] --stream FILE.wav [options]
  monitor fingerprint FILE.wav [options]

options (defaults in brackets):
  --hop N              hop size in samples [256]
  --min-freq HZ        lowest bin [55]        --max-freq HZ      highest bin [7040]
  --bins-per-octave N  [24]                   --gamma HZ         variable-Q offset [0]
  --time-radius N      peak window ±frames [24]
  --bin-radius N       peak window ±bins [9]
  --prominence DB      dB above the local mean [15]
  --floor DB           absolute peak floor in dBFS [-80]
  --zone N             hash zone in frames [320]
  --fan-out N          [4]                    --ratio-steps N    [32]
  --window S           evidence window in seconds [5]
  --half N             evidence count giving confidence 50 [40]
  --threshold C        detection threshold, 0..100 [70]
  --verify-seconds S   most recent query peaks aligned with the song's
                       peaks [2]
  --verify-frames N    alignment tolerance in frames [4]
  --verify-bins N      alignment tolerance in bins [1]
  --verify-start A     alignment a start needs, 0..1 [0.4]
  --verify-hold A      alignment that keeps a play going at half the
                       threshold, 0..1 [0.3]
  --release S          seconds below threshold that end a detection [3]
  --report S           seconds between report lines [0.25]
  --block N            samples pushed per call in the stream [4096]";

#[derive(Debug, Clone)]
struct Options {
    hop: usize,
    min_freq: f32,
    max_freq: f32,
    bins_per_octave: usize,
    gamma: f32,
    time_radius: usize,
    bin_radius: usize,
    prominence: f32,
    floor: f32,
    zone: usize,
    fan_out: usize,
    ratio_steps: usize,
    window: f64,
    half: f64,
    threshold: f64,
    verify_seconds: f64,
    verify_frames: u32,
    verify_bins: u32,
    verify_start: f64,
    verify_hold: f64,
    release: f64,
    report: f64,
    block: usize,
    watch: Vec<(String, String)>,
    stream: Option<String>,
    fingerprint: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hop: 256,
            min_freq: 55.0,
            max_freq: 7040.0,
            bins_per_octave: 24,
            gamma: 0.0,
            time_radius: 24,
            bin_radius: 9,
            prominence: 15.0,
            floor: -80.0,
            zone: 320,
            fan_out: 4,
            ratio_steps: 32,
            window: 5.0,
            half: 40.0,
            threshold: 70.0,
            verify_seconds: 2.0,
            verify_frames: 4,
            verify_bins: 1,
            verify_start: 0.4,
            verify_hold: 0.3,
            release: 3.0,
            report: 0.25,
            block: 4096,
            watch: Vec::new(),
            stream: None,
            fingerprint: None,
        }
    }
}

fn parse_args() -> Options {
    let mut opts = Options::default();
    let mut args = std::env::args().skip(1);
    fn value<T: std::str::FromStr>(flag: &str, v: Option<String>) -> T {
        v.and_then(|s| s.parse().ok()).unwrap_or_else(|| {
            eprintln!("bad or missing value for {flag}\n{USAGE}");
            exit(2)
        })
    }
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "fingerprint" => opts.fingerprint = Some(value("fingerprint", args.next())),
            "--watch" => {
                let spec: String = value("--watch", args.next());
                let (name, path) = match spec.split_once('=') {
                    Some((n, p)) => (n.to_owned(), p.to_owned()),
                    None => (
                        std::path::Path::new(&spec)
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| spec.clone()),
                        spec.clone(),
                    ),
                };
                opts.watch.push((name, path));
            }
            "--stream" => opts.stream = Some(value("--stream", args.next())),
            "--hop" => opts.hop = value(&arg, args.next()),
            "--min-freq" => opts.min_freq = value(&arg, args.next()),
            "--max-freq" => opts.max_freq = value(&arg, args.next()),
            "--bins-per-octave" => opts.bins_per_octave = value(&arg, args.next()),
            "--gamma" => opts.gamma = value(&arg, args.next()),
            "--time-radius" => opts.time_radius = value(&arg, args.next()),
            "--bin-radius" => opts.bin_radius = value(&arg, args.next()),
            "--prominence" => opts.prominence = value(&arg, args.next()),
            "--floor" => opts.floor = value(&arg, args.next()),
            "--zone" => opts.zone = value(&arg, args.next()),
            "--fan-out" => opts.fan_out = value(&arg, args.next()),
            "--ratio-steps" => opts.ratio_steps = value(&arg, args.next()),
            "--window" => opts.window = value(&arg, args.next()),
            "--half" => opts.half = value(&arg, args.next()),
            "--threshold" => opts.threshold = value(&arg, args.next()),
            "--verify-seconds" => opts.verify_seconds = value(&arg, args.next()),
            "--verify-frames" => opts.verify_frames = value(&arg, args.next()),
            "--verify-bins" => opts.verify_bins = value(&arg, args.next()),
            "--verify-start" => opts.verify_start = value(&arg, args.next()),
            "--verify-hold" => opts.verify_hold = value(&arg, args.next()),
            "--release" => opts.release = value(&arg, args.next()),
            "--report" => opts.report = value(&arg, args.next()),
            "--block" => opts.block = value(&arg, args.next()),
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0)
            }
            other => {
                eprintln!("unknown argument {other}\n{USAGE}");
                exit(2)
            }
        }
    }
    if opts.fingerprint.is_none() && (opts.watch.is_empty() || opts.stream.is_none()) {
        eprintln!("{USAGE}");
        exit(2);
    }
    opts
}

/// Reads a WAV file as mono f32 and returns the samples with the rate.
fn read_wav(path: &str) -> (Vec<f32>, u32) {
    let mut reader = hound::WavReader::open(path).unwrap_or_else(|e| {
        eprintln!("cannot open {path}: {e}");
        exit(1)
    });
    let spec = reader.spec();
    let channels = usize::from(spec.channels);
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    let mono = samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    (mono, spec.sample_rate)
}

fn fingerprint_config(opts: &Options) -> FingerprintConfig {
    FingerprintConfig {
        time_radius: opts.time_radius,
        bin_radius: opts.bin_radius,
        prominence: opts.prominence,
        floor: opts.floor,
        zone: opts.zone,
        fan_out: opts.fan_out,
        ratio_steps: opts.ratio_steps,
    }
}

fn build_cqt(opts: &Options, sample_rate: u32) -> Cqt {
    let params = CqtParams::builder(sample_rate, opts.min_freq, opts.max_freq)
        .bins_per_octave(opts.bins_per_octave)
        .gamma(opts.gamma)
        .build()
        .unwrap_or_else(|e| {
            eprintln!("invalid transform parameters: {e}");
            exit(2)
        });
    Cqt::new(params)
}

/// Fingerprints a whole file in batch mode (identical to streaming).
fn fingerprint_file(opts: &Options, cqt: &Cqt, samples: &[f32]) -> (u64, Vec<Peak>, Vec<Hash>) {
    fingerprint(cqt, opts.hop, samples, &fingerprint_config(opts)).unwrap_or_else(|e| {
        eprintln!("transform failed: {e}");
        exit(1)
    })
}

fn main() {
    let opts = parse_args();
    let mut out: Out = BufWriter::new(std::io::stdout().lock());

    if let Some(path) = &opts.fingerprint {
        let (samples, sample_rate) = read_wav(path);
        let cqt = build_cqt(&opts, sample_rate);
        let (_, peaks, hashes) = fingerprint_file(&opts, &cqt, &samples);
        for p in peaks {
            writeln!(out, "P {} {}", p.frame, p.bin).unwrap();
        }
        for h in hashes {
            writeln!(out, "H {} {} {} {}", h.key, h.frame, h.bin, h.span).unwrap();
        }
        return;
    }

    // Index the watched songs.
    let mut builder = IndexBuilder::new();
    let mut tracks: Vec<PeakTrack> = Vec::new();
    let mut cqt: Option<(Cqt, u32)> = None;
    let index_start = Instant::now();
    for (name, path) in &opts.watch {
        let (samples, sample_rate) = read_wav(path);
        let transform = match &cqt {
            Some((c, rate)) if *rate == sample_rate => c,
            Some((_, rate)) => {
                eprintln!("{path}: sample rate {sample_rate} differs from {rate}");
                exit(1)
            }
            None => {
                cqt = Some((build_cqt(&opts, sample_rate), sample_rate));
                &cqt.as_ref().unwrap().0
            }
        };
        let (frames, peaks, hashes) = fingerprint_file(&opts, transform, &samples);
        let count = hashes.len();
        builder.add_song(name, frames as u32, hashes);
        writeln!(
            out,
            "{{\"event\":\"index\",\"song\":{},\"seconds\":{:.2},\"frames\":{frames},\"peaks\":{},\"hashes\":{count}}}",
            json_string(name),
            samples.len() as f64 / f64::from(sample_rate),
            peaks.len()
        )
        .unwrap();
        tracks.push(PeakTrack::new(peaks));
    }
    let index: Index = builder.build(8);
    let (cqt, sample_rate) = cqt.unwrap();
    writeln!(
        out,
        "{{\"event\":\"index_done\",\"songs\":{},\"hashes\":{},\"dropped\":{},\"bytes\":{},\"seconds\":{:.3}}}",
        index.names().len(),
        index.len(),
        index.dropped(),
        index.memory_bytes(),
        index_start.elapsed().as_secs_f64()
    )
    .unwrap();

    // Stream.
    let stream_path = opts.stream.as_ref().unwrap();
    let (samples, stream_rate) = read_wav(stream_path);
    if stream_rate != sample_rate {
        eprintln!(
            "{stream_path}: sample rate {stream_rate} differs from the watched songs ({sample_rate})"
        );
        exit(1);
    }
    let frames_per_second = f64::from(sample_rate) / opts.hop as f64;
    let mut matcher = Matcher::new(MatcherConfig {
        window_frames: (opts.window * frames_per_second).round() as u64,
        offset_step: 0.5 * frames_per_second,
        half: opts.half,
        ..MatcherConfig::default()
    });
    let mut tracker = Tracker::new(
        index.names().len(),
        TrackerConfig {
            threshold: opts.threshold,
            release_frames: (opts.release * frames_per_second).round() as u64,
            confirm_frames: frames_per_second.round() as u64,
            jump_shift: 2,
            jump_tempo: 0.05,
            jump_frames: 3.0 * frames_per_second,
            start_alignment: opts.verify_start,
            hold_alignment: opts.verify_hold,
            hold_confidence: 0.5 * opts.threshold,
        },
    );
    let mut fp = Fingerprinter::new(cqt.num_bins(), &fingerprint_config(&opts));
    let delay = fp.delay_frames();
    let report_every = (opts.report * frames_per_second).round().max(1.0) as u64;
    let mut stream = CqtStream::new(&cqt, opts.hop).unwrap_or_else(|e| {
        eprintln!("bad hop size: {e}");
        exit(2)
    });
    writeln!(
        out,
        "{{\"event\":\"stream\",\"file\":{},\"seconds\":{:.2},\"latency_seconds\":{:.3},\"fingerprint_delay_seconds\":{:.3},\"window_seconds\":{},\"half\":{},\"threshold\":{},\"fan_out\":{},\"verify_seconds\":{},\"verify_start\":{},\"verify_hold\":{}}}",
        json_string(stream_path),
        samples.len() as f64 / f64::from(sample_rate),
        cqt.latency_samples() as f64 / f64::from(sample_rate),
        delay as f64 / frames_per_second,
        opts.window,
        opts.half,
        opts.threshold,
        opts.fan_out,
        opts.verify_seconds,
        opts.verify_start,
        opts.verify_hold
    )
    .unwrap();

    let ctx = Context {
        names: index.names(),
        tracks: &tracks,
        seconds_per_frame: opts.hop as f64 / f64::from(sample_rate),
        sample_rate: f64::from(sample_rate),
        delay,
        verify_span: (opts.verify_seconds * frames_per_second).round() as u64,
        verify_frames: opts.verify_frames,
        verify_bins: opts.verify_bins,
    };
    let mut frames = 0u64;
    let mut consumed = 0u64;
    let mut delays = Histogram::new(delay as usize + 1);
    // Query peaks not older than the evidence window, for verification.
    let mut recent: VecDeque<Peak> = VecDeque::new();
    let started = Instant::now();
    let mut pending_report = false;
    for block in samples.chunks(opts.block) {
        consumed += block.len() as u64;
        let (matcher_ref, fp_ref, frames_ref, delays_ref, recent_ref) =
            (&mut matcher, &mut fp, &mut frames, &mut delays, &mut recent);
        stream.push(&cqt, block, |magnitudes| {
            fp_ref.push(
                magnitudes,
                |p| recent_ref.push_back(p),
                |h| {
                    delays_ref.record(*frames_ref - h.frame);
                    matcher_ref.push(&index, &h);
                },
            );
            *frames_ref += 1;
            if frames_ref.is_multiple_of(report_every) {
                pending_report = true;
            }
        });
        if pending_report {
            ctx.report(
                &mut matcher,
                &mut tracker,
                &mut recent,
                frames,
                consumed,
                &mut out,
                false,
            );
            pending_report = false;
        }
    }
    {
        let (matcher_ref, fp_ref, frames_ref, delays_ref, recent_ref) =
            (&mut matcher, &mut fp, &mut frames, &mut delays, &mut recent);
        stream.flush(&cqt, |magnitudes| {
            fp_ref.push(
                magnitudes,
                |p| recent_ref.push_back(p),
                |h| {
                    delays_ref.record(*frames_ref - h.frame);
                    matcher_ref.push(&index, &h);
                },
            );
            *frames_ref += 1;
        });
        fp_ref.flush(
            |p| recent_ref.push_back(p),
            |h| matcher_ref.push(&index, &h),
        );
    }
    ctx.report(
        &mut matcher,
        &mut tracker,
        &mut recent,
        frames,
        consumed,
        &mut out,
        true,
    );
    let cpu = started.elapsed().as_secs_f64();
    let audio = samples.len() as f64 / ctx.sample_rate;
    writeln!(
        out,
        "{{\"event\":\"done\",\"audio_seconds\":{audio:.2},\"cpu_seconds\":{cpu:.3},\"realtime_fraction\":{:.4},\"frames\":{frames},\"peaks\":{},\"lookups\":{},\"matches\":{},\"hash_delay_median_seconds\":{:.3},\"hash_delay_max_seconds\":{:.3}}}",
        cpu / audio,
        fp.peaks(),
        matcher.lookups(),
        matcher.matches(),
        delays.quantile(0.5) as f64 * ctx.seconds_per_frame,
        delays.max() as f64 * ctx.seconds_per_frame,
    )
    .unwrap();
}

type Out = BufWriter<std::io::StdoutLock<'static>>;

/// What the report and event writers need to know about the stream.
struct Context<'a> {
    names: &'a [String],
    tracks: &'a [PeakTrack],
    seconds_per_frame: f64,
    sample_rate: f64,
    /// Worst-case frames between a pushed frame and its hashes.
    delay: u64,
    /// Most recent query frames whose peaks are verified.
    verify_span: u64,
    verify_frames: u32,
    verify_bins: u32,
}

impl Context<'_> {
    /// Writes a report line for the state after `frames` frames and
    /// `consumed` samples, then feeds the tracker and writes its events.
    /// With `final_flush` every play is ended instead.
    #[allow(clippy::too_many_arguments)]
    fn report(
        &self,
        matcher: &mut Matcher,
        tracker: &mut Tracker,
        recent: &mut VecDeque<Peak>,
        frames: u64,
        consumed: u64,
        out: &mut Out,
        final_flush: bool,
    ) {
        let frame = frames.saturating_sub(1);
        matcher.advance(frame.saturating_sub(self.delay));
        let t = frame as f64 * self.seconds_per_frame;
        let consumed = consumed as f64 / self.sample_rate;
        // The most recent query peaks: the votes lag the audio by the
        // window, but whether the song is playing *now* is decided by
        // the newest peaks, which are known up to the picker's delay.
        let horizon = frame.saturating_sub(self.verify_span);
        while recent.front().is_some_and(|p| p.frame < horizon) {
            recent.pop_front();
        }
        let query = recent.make_contiguous();
        let verified: Vec<(Scored, Verification)> = matcher
            .best_per_song()
            .into_iter()
            .map(|c| {
                let v = self.tracks[usize::from(c.song)].verify(
                    query,
                    c.shift,
                    c.tempo,
                    c.offset,
                    self.verify_frames,
                    self.verify_bins,
                );
                let scored = Scored {
                    candidate: c,
                    confidence: matcher.confidence(c.evidence),
                    alignment: v.query_fraction(),
                };
                (scored, v)
            })
            .collect();
        if !final_flush {
            let best = verified.iter().max_by_key(|(s, _)| s.candidate.evidence);
            let (name, evidence, confidence, shift, tempo, position, v) = match best {
                Some((s, v)) => (
                    json_string(&self.names[usize::from(s.candidate.song)]),
                    s.candidate.evidence,
                    s.confidence,
                    s.candidate.shift,
                    s.candidate.tempo,
                    (s.candidate.tempo * frame as f64 + s.candidate.offset)
                        * self.seconds_per_frame,
                    *v,
                ),
                None => (
                    "null".to_owned(),
                    0,
                    0.0,
                    0,
                    1.0,
                    0.0,
                    Verification::default(),
                ),
            };
            writeln!(
                out,
                "{{\"event\":\"report\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":{name},\"evidence\":{evidence},\"confidence\":{confidence:.1},\"shift\":{shift},\"tempo\":{tempo:.4},\"position\":{position:.2},\"votes\":{},{}}}",
                matcher.votes_in_window(),
                verification_json(&v)
            )
            .unwrap();
        }
        let scored: Vec<Scored> = verified.iter().map(|(s, _)| *s).collect();
        let on_event = |event: Event| {
            let v = match event {
                Event::Start { song, .. } => verified
                    .iter()
                    .find(|(s, _)| s.candidate.song == song)
                    .map_or(Verification::default(), |(_, v)| *v),
                Event::End { .. } => Verification::default(),
            };
            self.write_event(out, &event, t, consumed, &v)
        };
        if final_flush {
            tracker.finish(frame, on_event);
        } else {
            tracker.update(frame, &scored, on_event);
        }
    }

    fn write_event(
        &self,
        out: &mut Out,
        event: &Event,
        t: f64,
        consumed: f64,
        verification: &Verification,
    ) {
        match *event {
            Event::Start {
                song,
                candidate: c,
                confidence,
                position,
                ..
            } => writeln!(
                out,
                "{{\"event\":\"start\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":{},\"evidence\":{},\"confidence\":{confidence:.1},\"shift\":{},\"tempo\":{:.4},\"position\":{:.2},{}}}",
                json_string(&self.names[usize::from(song)]),
                c.evidence,
                c.shift,
                c.tempo,
                position * self.seconds_per_frame,
                verification_json(verification)
            ),
            Event::End {
                song, start_frame, ..
            } => writeln!(
                out,
                "{{\"event\":\"end\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":{},\"start\":{:.3}}}",
                json_string(&self.names[usize::from(song)]),
                start_frame as f64 * self.seconds_per_frame
            ),
        }
        .unwrap();
    }
}

/// The verification fields of a report or start line.
fn verification_json(v: &Verification) -> String {
    format!(
        "\"verify_q\":{:.3},\"verify_r\":{:.3},\"query_peaks\":{},\"query_matched\":{},\"reference_peaks\":{},\"reference_matched\":{}",
        v.query_fraction(),
        v.reference_fraction(),
        v.query_peaks,
        v.query_matched,
        v.reference_peaks,
        v.reference_matched
    )
}

/// Counts of integer values `0..len`, larger values in the last slot.
struct Histogram {
    counts: Vec<u64>,
    total: u64,
}

impl Histogram {
    fn new(len: usize) -> Self {
        Self {
            counts: vec![0; len.max(1)],
            total: 0,
        }
    }

    fn record(&mut self, value: u64) {
        let slot = (value as usize).min(self.counts.len() - 1);
        self.counts[slot] += 1;
        self.total += 1;
    }

    /// Smallest value with at least `q` of the counts at or below it.
    fn quantile(&self, q: f64) -> usize {
        let target = (q * self.total as f64).ceil() as u64;
        let mut seen = 0;
        for (value, &count) in self.counts.iter().enumerate() {
            seen += count;
            if seen >= target {
                return value;
            }
        }
        self.counts.len() - 1
    }

    fn max(&self) -> usize {
        self.counts.iter().rposition(|&c| c > 0).unwrap_or(0)
    }
}

/// Quotes `s` as a JSON string.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_string("plain"), "\"plain\"");
        assert_eq!(
            json_string("The \"Live\" Mix\\ \n\u{1}"),
            "\"The \\\"Live\\\" Mix\\\\ \\n\\u0001\""
        );
        assert_eq!(json_string("ünïcödé"), "\"ünïcödé\"");
    }

    #[test]
    fn histogram_quantiles() {
        let mut h = Histogram::new(10);
        for v in [1, 1, 2, 3, 50] {
            h.record(v);
        }
        assert_eq!(h.quantile(0.5), 2);
        assert_eq!(h.quantile(0.0), 0);
        assert_eq!(h.quantile(1.0), 9);
        assert_eq!(h.max(), 9);
        assert_eq!(Histogram::new(4).quantile(0.5), 0);
    }
}
