//! Watch-list monitor: fingerprints the watched songs, runs a stream through
//! the same pipeline and prints JSON lines with the running match state.
//!
//! ```console
//! monitor --watch song.wav [--watch other.wav ...] --stream radio.wav [options]
//! monitor fingerprint file.wav      # peaks and hashes of one file, for validation
//! ```

use std::io::{BufWriter, Write};
use std::process::exit;
use std::time::Instant;

use cqt_monitor::{
    Hash, Index, IndexBuilder, Matcher, MatcherConfig, Peak, PeakPicker, TripletHasher,
};
use cqt_rs::{Cqt, CqtParams, CqtStream, magnitude_to_db};

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
  --fan-out N          [6]                    --ratio-steps N    [32]
  --window S           evidence window in seconds [5]
  --half N             evidence count giving confidence 50 [100]
  --threshold C        detection threshold, 0..100 [70]
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
            fan_out: 6,
            ratio_steps: 32,
            window: 5.0,
            half: 100.0,
            threshold: 70.0,
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

/// Peak picker and hasher chained behind a dB conversion.
struct Fingerprinter {
    picker: PeakPicker,
    hasher: TripletHasher,
    db: Vec<f32>,
    frames: u64,
    peaks: u64,
}

impl Fingerprinter {
    fn new(opts: &Options, num_bins: usize) -> Self {
        Self {
            picker: PeakPicker::new(
                num_bins,
                opts.time_radius,
                opts.bin_radius,
                opts.prominence,
                opts.floor,
            ),
            hasher: TripletHasher::new(opts.zone, opts.fan_out, opts.ratio_steps),
            db: vec![0.0; num_bins],
            frames: 0,
            peaks: 0,
        }
    }

    /// Frames of delay between a pushed frame and the hashes anchored on it.
    fn delay_frames(&self) -> u64 {
        (self.picker.delay_frames() + self.hasher.delay_frames()) as u64
    }

    fn push<P: FnMut(Peak), H: FnMut(Hash)>(
        &mut self,
        magnitudes: &[f32],
        mut on_peak: P,
        on_hash: H,
    ) {
        self.db.copy_from_slice(magnitudes);
        magnitude_to_db(&mut self.db, 1.0, 1e-5, None);
        let hasher = &mut self.hasher;
        let peaks = &mut self.peaks;
        self.picker.push(&self.db, |p| {
            *peaks += 1;
            on_peak(p);
            hasher.push(p);
        });
        self.frames += 1;
        if let Some(decided) = self
            .frames
            .checked_sub(self.picker.delay_frames() as u64 + 1)
        {
            self.hasher.advance(decided, on_hash);
        }
    }

    fn flush<P: FnMut(Peak), H: FnMut(Hash)>(&mut self, mut on_peak: P, mut on_hash: H) {
        let hasher = &mut self.hasher;
        let peaks = &mut self.peaks;
        self.picker.flush(|p| {
            *peaks += 1;
            on_peak(p);
            hasher.push(p);
        });
        self.hasher.flush(&mut on_hash);
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
fn fingerprint_file(opts: &Options, cqt: &Cqt, samples: &[f32]) -> (u64, u64, Vec<Hash>) {
    let magnitudes = cqt.process(samples, opts.hop).unwrap_or_else(|e| {
        eprintln!("transform failed: {e}");
        exit(1)
    });
    let mut fp = Fingerprinter::new(opts, cqt.num_bins());
    let mut hashes = Vec::new();
    for row in magnitudes.rows() {
        fp.push(row.as_slice().unwrap(), |_| {}, |h| hashes.push(h));
    }
    fp.flush(|_| {}, |h| hashes.push(h));
    (fp.frames, fp.peaks, hashes)
}

fn main() {
    let opts = parse_args();
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if let Some(path) = &opts.fingerprint {
        let (samples, sample_rate) = read_wav(path);
        let cqt = build_cqt(&opts, sample_rate);
        let magnitudes = cqt.process(&samples, opts.hop).unwrap();
        let mut fp = Fingerprinter::new(&opts, cqt.num_bins());
        let mut peaks = Vec::new();
        let mut hashes = Vec::new();
        for row in magnitudes.rows() {
            fp.push(
                row.as_slice().unwrap(),
                |p| peaks.push(p),
                |h| hashes.push(h),
            );
        }
        fp.flush(|p| peaks.push(p), |h| hashes.push(h));
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
            "{{\"event\":\"index\",\"song\":\"{name}\",\"seconds\":{:.2},\"frames\":{frames},\"peaks\":{peaks},\"hashes\":{count}}}",
            samples.len() as f64 / f64::from(sample_rate)
        )
        .unwrap();
    }
    let index: Index = builder.build(8);
    let (cqt, sample_rate) = cqt.unwrap();
    writeln!(
        out,
        "{{\"event\":\"index_done\",\"songs\":{},\"hashes\":{},\"bytes\":{},\"seconds\":{:.3}}}",
        index.names().len(),
        index.len(),
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
    let mut fp = Fingerprinter::new(&opts, cqt.num_bins());
    let delay = fp.delay_frames();
    let report_every = (opts.report * frames_per_second).round().max(1.0) as u64;
    let mut stream = CqtStream::new(&cqt, opts.hop).unwrap_or_else(|e| {
        eprintln!("bad hop size: {e}");
        exit(2)
    });
    writeln!(
        out,
        "{{\"event\":\"stream\",\"file\":\"{stream_path}\",\"seconds\":{:.2},\"latency_seconds\":{:.3},\"fingerprint_delay_seconds\":{:.3},\"window_seconds\":{},\"half\":{},\"threshold\":{}}}",
        samples.len() as f64 / f64::from(sample_rate),
        cqt.latency_samples() as f64 / f64::from(sample_rate),
        delay as f64 / frames_per_second,
        opts.window,
        opts.half,
        opts.threshold
    )
    .unwrap();

    /// One detection in progress.
    #[derive(Clone, Copy)]
    struct Active {
        start: f64,
        shift: i32,
        tempo: f64,
        /// Song position (frames) predicted for the last report frame.
        position: f64,
        frame: u64,
        last_above: f64,
        /// Consecutive reports whose hypothesis disagrees with this one.
        jumps: u32,
    }
    struct State {
        active: Vec<Option<Active>>,
        frames: u64,
        consumed: u64,
    }
    let mut state = State {
        active: vec![None; index.names().len()],
        frames: 0,
        consumed: 0,
    };
    let hop = opts.hop as f64;
    let sr = f64::from(sample_rate);
    let frames_per_second = sr / hop;
    let names = index.names();

    let report = |matcher: &mut Matcher,
                  state: &mut State,
                  out: &mut BufWriter<std::io::StdoutLock>,
                  final_flush: bool| {
        let frame = state.frames.saturating_sub(1);
        matcher.advance(frame.saturating_sub(delay));
        let t = frame as f64 * hop / sr;
        let consumed = state.consumed as f64 / sr;
        let candidates = matcher.best_per_song();
        let best = candidates.iter().max_by_key(|c| c.evidence).copied();
        if !final_flush {
            let (name, evidence, confidence, shift, tempo, position) = match best {
                Some(c) => (
                    format!("\"{}\"", names[usize::from(c.song)]),
                    c.evidence,
                    matcher.confidence(c.evidence),
                    c.shift,
                    c.tempo,
                    (c.tempo * frame as f64 + c.offset) * hop / sr,
                ),
                None => ("null".to_owned(), 0, 0.0, 0, 1.0, 0.0),
            };
            writeln!(
                out,
                "{{\"event\":\"report\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":{name},\"evidence\":{evidence},\"confidence\":{confidence:.1},\"shift\":{shift},\"tempo\":{tempo:.4},\"position\":{position:.2},\"votes\":{}}}",
                matcher.votes_in_window()
            )
            .unwrap();
        }
        for (song, name) in names.iter().enumerate() {
            let candidate = candidates
                .iter()
                .find(|c| usize::from(c.song) == song)
                .copied();
            let confidence = candidate.map_or(0.0, |c| matcher.confidence(c.evidence));
            let above = confidence >= opts.threshold && !final_flush;
            let end_line = |out: &mut BufWriter<std::io::StdoutLock>, a: &Active| {
                writeln!(
                    out,
                    "{{\"event\":\"end\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":\"{name}\",\"start\":{:.3}}}",
                    a.start
                )
                .unwrap();
            };
            match (state.active[song], above, candidate) {
                (Some(mut a), true, Some(c)) => {
                    // Same play, or a new play of the same song (the
                    // hypothesis jumped)?
                    let position = c.tempo * frame as f64 + c.offset;
                    let predicted = a.position + a.tempo * (frame - a.frame) as f64;
                    let jumped = (c.shift - a.shift).abs() > 2
                        || (c.tempo - a.tempo).abs() > 0.05
                        || (position - predicted).abs() > 3.0 * frames_per_second;
                    // A new play of the same song is declared only when the
                    // new hypothesis persists for a second; at the tail of a
                    // play a repeated riff can briefly win the vote.
                    a.last_above = consumed;
                    if jumped {
                        a.jumps += 1;
                    } else {
                        a.jumps = 0;
                        a.position = position;
                        a.frame = frame;
                    }
                    if a.jumps as f64 * opts.report < 1.0 {
                        state.active[song] = Some(a);
                        continue;
                    }
                    end_line(out, &a);
                    state.active[song] = None;
                    // Fall through to start the new play.
                    let position = (c.tempo * frame as f64 + c.offset) * hop / sr;
                    writeln!(
                        out,
                        "{{\"event\":\"start\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":\"{name}\",\"evidence\":{},\"confidence\":{confidence:.1},\"shift\":{},\"tempo\":{:.4},\"position\":{position:.2}}}",
                        c.evidence, c.shift, c.tempo
                    )
                    .unwrap();
                    state.active[song] = Some(Active {
                        start: consumed,
                        shift: c.shift,
                        tempo: c.tempo,
                        position: c.tempo * frame as f64 + c.offset,
                        frame,
                        last_above: consumed,
                        jumps: 0,
                    });
                }
                (None, true, Some(c)) => {
                    let position = (c.tempo * frame as f64 + c.offset) * hop / sr;
                    writeln!(
                        out,
                        "{{\"event\":\"start\",\"t\":{t:.3},\"consumed\":{consumed:.3},\"song\":\"{name}\",\"evidence\":{},\"confidence\":{confidence:.1},\"shift\":{},\"tempo\":{:.4},\"position\":{position:.2}}}",
                        c.evidence, c.shift, c.tempo
                    )
                    .unwrap();
                    state.active[song] = Some(Active {
                        start: consumed,
                        shift: c.shift,
                        tempo: c.tempo,
                        position: c.tempo * frame as f64 + c.offset,
                        frame,
                        last_above: consumed,
                        jumps: 0,
                    });
                }
                (Some(a), false, _) if consumed - a.last_above >= opts.release || final_flush => {
                    end_line(out, &a);
                    state.active[song] = None;
                }
                _ => {}
            }
        }
    };

    let started = Instant::now();
    let mut pending_report = false;
    for block in samples.chunks(opts.block) {
        state.consumed += block.len() as u64;
        let matcher_ref = &mut matcher;
        let fp_ref = &mut fp;
        let state_ref = &mut state;
        let index_ref = &index;
        stream.push(&cqt, block, |magnitudes| {
            fp_ref.push(magnitudes, |_| {}, |h| matcher_ref.push(index_ref, &h));
            state_ref.frames += 1;
            if state_ref.frames.is_multiple_of(report_every) {
                pending_report = true;
            }
        });
        if pending_report {
            report(&mut matcher, &mut state, &mut out, false);
            pending_report = false;
        }
    }
    {
        let matcher_ref = &mut matcher;
        let fp_ref = &mut fp;
        let state_ref = &mut state;
        let index_ref = &index;
        stream.flush(&cqt, |magnitudes| {
            fp_ref.push(magnitudes, |_| {}, |h| matcher_ref.push(index_ref, &h));
            state_ref.frames += 1;
        });
        fp_ref.flush(|_| {}, |h| matcher_ref.push(index_ref, &h));
    }
    report(&mut matcher, &mut state, &mut out, true);
    let cpu = started.elapsed().as_secs_f64();
    let audio = samples.len() as f64 / sr;
    writeln!(
        out,
        "{{\"event\":\"done\",\"audio_seconds\":{audio:.2},\"cpu_seconds\":{cpu:.3},\"realtime_fraction\":{:.4},\"frames\":{},\"peaks\":{},\"lookups\":{},\"matches\":{}}}",
        cpu / audio,
        state.frames,
        fp.peaks,
        matcher.lookups(),
        matcher.matches()
    )
    .unwrap();
}
