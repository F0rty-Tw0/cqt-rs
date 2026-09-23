//! Times cqt-rs and the other Rust CQT crates on the README's two grids and
//! writes the test signals for `compare_py.py`. Prints markdown table rows.
//!
//! Usage: compare-rs <signal-dir>

use std::hint::black_box;
use std::path::Path;
use std::time::Instant;

use cicuetea::NsgfCqtSparse;
use cqt_rs::{Cqt, CqtParams};
use non_empty_slice::NonEmptySlice;
use num_complex::Complex;
use qdft::QDFT32;

/// Grid parameters, as in `benches/bench.rs`.
struct Grid {
    name: &'static str,
    sample_rate: u32,
    min_hz: f32,
    max_hz: f32,
    bins_per_octave: usize,
    hop: usize,
    signal: Vec<f32>,
}

/// The bench signal: a linear chirp from `start_hz` to `end_hz`.
fn chirp(sample_rate: u32, start_hz: f32, end_hz: f32, seconds: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let phase = start_hz * t + (end_hz - start_hz) * t * t / (2.0 * seconds);
            (std::f32::consts::TAU * phase).sin()
        })
        .collect()
}

fn grids() -> [Grid; 2] {
    [
        Grid {
            name: "legacy",
            sample_rate: 22_000,
            min_hz: 14.568,
            max_hz: 7_902.1,
            bins_per_octave: 12,
            hop: 1_760,
            signal: chirp(22_000, 440.0, 440.0, 30.0),
        },
        Grid {
            name: "fingerprint",
            sample_rate: 44_100,
            min_hz: 55.0,
            max_hz: 7_040.0,
            bins_per_octave: 24,
            hop: 512,
            signal: chirp(44_100, 55.0, 7_000.0, 10.0),
        },
    ]
}

/// Median wall time in ms after one warm-up call; repeats for about 2 s,
/// at least 5 and at most 51 times.
fn median_ms(mut run: impl FnMut()) -> f64 {
    let start = Instant::now();
    run();
    let first = start.elapsed().as_secs_f64();
    let repeats = ((2.0 / first) as usize).clamp(5, 51);
    let mut times: Vec<f64> = (0..repeats)
        .map(|_| {
            let start = Instant::now();
            run();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[repeats / 2]
}

fn cqt_params(grid: &Grid) -> CqtParams {
    CqtParams::new(
        grid.sample_rate,
        grid.min_hz,
        grid.max_hz,
        grid.bins_per_octave,
    )
    .unwrap()
}

fn time_cqt_rs(grid: &Grid) -> f64 {
    let cqt = Cqt::new(cqt_params(grid));
    median_ms(|| {
        black_box(cqt.process(black_box(&grid.signal), grid.hop).unwrap());
    })
}

fn time_cqt_rs_reused(grid: &Grid) -> f64 {
    let cqt = Cqt::new(cqt_params(grid));
    let mut workspace = cqt.workspace();
    median_ms(|| {
        black_box(
            cqt.process_with(&mut workspace, black_box(&grid.signal), grid.hop)
                .unwrap(),
        );
    })
}

/// Returns (forward ms, filterbank build ms, padded seconds).
fn time_cicuetea(grid: &Grid) -> (f64, f64, f64) {
    // ponytail: the NSGT needs a power-of-two block, so the signal is zero
    // padded (and widened to f64) once, outside the timed call.
    let block = grid.signal.len().next_power_of_two();
    let mut padded = vec![0.0f64; block];
    for (out, &sample) in padded.iter_mut().zip(&grid.signal) {
        *out = f64::from(sample);
    }
    let start = Instant::now();
    let mut cqt = NsgfCqtSparse::new(
        f64::from(grid.sample_rate),
        block,
        1.0 / grid.bins_per_octave as f64,
        f64::from(grid.min_hz),
        f64::from(grid.max_hz),
        440.0,
    );
    let build_ms = start.elapsed().as_secs_f64() * 1e3;
    assert!(cqt.is_valid(), "cicuetea rejected the {} grid", grid.name);
    let mut coefs = cqt.get_coefs();
    let forward_ms = median_ms(|| {
        cqt.forward(black_box(&padded), &mut coefs);
        black_box(&coefs);
    });
    (
        forward_ms,
        build_ms,
        block as f64 / f64::from(grid.sample_rate),
    )
}

fn time_dasp(grid: &Grid, bins: usize) -> f64 {
    median_ms(|| {
        let spectrum = dasp_rs::proc::cqt(black_box(&grid.signal), grid.sample_rate)
            .hop_length(grid.hop)
            .fmin(grid.min_hz)
            .n_bins(bins)
            .compute()
            .unwrap();
        black_box(spectrum);
    })
}

fn time_qdft(grid: &Grid) -> f64 {
    let mut qdft = QDFT32::new(
        f64::from(grid.sample_rate),
        (f64::from(grid.min_hz), f64::from(grid.max_hz)),
        grid.bins_per_octave as f64,
        0.0,
        Some((0.5, -0.5)),
    );
    // ponytail: one spectrum per sample would be 1.2 GB for the whole
    // signal, so it is fed one hop at a time into a reused buffer.
    let mut spectra = vec![Complex::<f64>::default(); grid.hop * qdft.size()];
    median_ms(|| {
        for block in grid.signal.chunks(grid.hop) {
            let out = &mut spectra[..block.len() * qdft.size()];
            qdft.qdft(black_box(block), out);
            black_box(&out);
        }
    })
}

fn time_spectrograms(grid: &Grid, octaves: usize) -> f64 {
    let params = spectrograms::CqtParams::new(
        grid.bins_per_octave.try_into().unwrap(),
        octaves.try_into().unwrap(),
        f64::from(grid.min_hz),
    )
    .unwrap();
    let samples = NonEmptySlice::new(&grid.signal).unwrap();
    median_ms(|| {
        let result = spectrograms::cqt(
            black_box(samples),
            f64::from(grid.sample_rate),
            &params,
            grid.hop.try_into().unwrap(),
        )
        .unwrap();
        black_box(result);
    })
}

fn write_signal(dir: &Path, grid: &Grid) {
    let bytes: Vec<u8> = grid.signal.iter().flat_map(|s| s.to_le_bytes()).collect();
    std::fs::write(dir.join(format!("{}.f32", grid.name)), bytes).unwrap();
}

fn row(name: &str, legacy: &str, fingerprint: &str, notes: &str) {
    println!("| {name} | {legacy} | {fingerprint} | {notes} |");
}

fn ms(value: f64) -> String {
    if value < 10.0 {
        format!("{value:.2} ms")
    } else {
        format!("{value:.1} ms")
    }
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: compare-rs <signal-dir>");
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    let [legacy, fingerprint] = grids();

    if cfg!(feature = "parallel") {
        let threads = std::thread::available_parallelism().unwrap();
        let notes = format!("rayon default pool, {threads} logical CPUs");
        row(
            "cqt-rs (`process`, parallel)",
            &ms(time_cqt_rs(&legacy)),
            &ms(time_cqt_rs(&fingerprint)),
            &notes,
        );
        row(
            "cqt-rs (`process_with`, reused workspace, parallel)",
            &ms(time_cqt_rs_reused(&legacy)),
            &ms(time_cqt_rs_reused(&fingerprint)),
            &notes,
        );
        return;
    }

    write_signal(dir, &legacy);
    write_signal(dir, &fingerprint);

    row(
        "cqt-rs (`process`)",
        &ms(time_cqt_rs(&legacy)),
        &ms(time_cqt_rs(&fingerprint)),
        "single-threaded build; filterbank built outside the timed call",
    );
    row(
        "cqt-rs (`process_with`, reused workspace)",
        &ms(time_cqt_rs_reused(&legacy)),
        &ms(time_cqt_rs_reused(&fingerprint)),
        "single-threaded build",
    );

    let (legacy_ms, legacy_build, legacy_padded) = time_cicuetea(&legacy);
    let (fp_ms, fp_build, fp_padded) = time_cicuetea(&fingerprint);
    row(
        "cicuetea (`NsgfCqtSparse`)",
        &ms(legacy_ms),
        &ms(fp_ms),
        &format!(
            "forward only, f64; zero padded to a power of two ({legacy_padded:.1} s / \
             {fp_padded:.1} s); filterbank build {:.2} s / {:.2} s",
            legacy_build / 1e3,
            fp_build / 1e3
        ),
    );

    row(
        "dasp-rs (`cqt`)",
        &ms(time_dasp(&legacy, 109)),
        "n/a",
        "12 bins per octave only; builds its kernels inside the call",
    );

    row(
        "qdft (`QDFT32`)",
        &ms(time_qdft(&legacy)),
        &ms(time_qdft(&fingerprint)),
        "sliding DFT, Hann: one spectrum per input sample, not per hop; fed one hop at a time",
    );

    // Whole octaves only: 9 / 7 octaves = 108 / 168 bins, one short of cqt-rs.
    row(
        "spectrograms (`cqt`)",
        &ms(time_spectrograms(&legacy, 9)),
        &ms(time_spectrograms(&fingerprint, 7)),
        "time-domain correlation per frame, kernels capped at 16384 samples; \
         whole octaves only (108 / 168 bins); builds its kernels inside the call",
    );
}
