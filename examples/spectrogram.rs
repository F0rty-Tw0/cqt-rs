//! Reads a WAV file, mixes it to mono and writes its CQT spectrogram as SVG.
//!
//! ```console
//! cargo run --release --example spectrogram -- song.wav [out.svg]
//! ```
//!
//! Only WAV is read here, through `hound`. MP3, FLAC and other formats need a
//! decoder such as `symphonia`; once decoded to mono `f32` samples the rest of
//! this example is unchanged.

use std::error::Error;
use std::time::Instant;

use cqt_rs::{Cqt, CqtParams, magnitude_to_db};
use plotters::prelude::*;

/// Most spectrogram columns drawn; longer files draw every n-th frame so the
/// SVG stays a few megabytes.
const MAX_COLUMNS: usize = 400;

/// Five-stop approximation of the viridis colour map for `t` in `0..=1`.
fn viridis(t: f32) -> RGBColor {
    const STOPS: [(f32, f32, f32); 5] = [
        (68.0, 1.0, 84.0),
        (59.0, 82.0, 139.0),
        (33.0, 145.0, 140.0),
        (94.0, 201.0, 98.0),
        (253.0, 231.0, 37.0),
    ];
    let x = t.clamp(0.0, 1.0) * (STOPS.len() - 1) as f32;
    let i = (x.floor() as usize).min(STOPS.len() - 2);
    let f = x - i as f32;
    let (a, b) = (STOPS[i], STOPS[i + 1]);
    RGBColor(
        (a.0 + (b.0 - a.0) * f) as u8,
        (a.1 + (b.1 - a.1) * f) as u8,
        (a.2 + (b.2 - a.2) * f) as u8,
    )
}

/// Decodes every sample to `f32` in `-1..=1` and averages the channels.
fn read_mono(path: &str) -> Result<(u32, Vec<f32>), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| s as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    let channels = spec.channels as usize;
    let mono = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    Ok((spec.sample_rate, mono))
}

fn run(input: &str, output: &str) -> Result<(), Box<dyn Error>> {
    let (sample_rate, signal) = read_mono(input)?;
    // The library rejects a range that does not fit below the file's Nyquist
    // frequency with an error that says so.
    let params = CqtParams::builder(sample_rate, 55.0, 7_040.0)
        .bins_per_octave(24)
        .build()
        .map_err(|e| format!("{sample_rate} Hz audio: {e}"))?;
    let cqt = Cqt::new(params);
    let hop = 512;

    let start = Instant::now();
    let mut result = cqt.process(&signal, hop)?;
    let elapsed = start.elapsed();
    // 0 dB is the loudest bin in the file.
    let peak = result.iter().fold(0f32, |max, &v| max.max(v));
    for mut row in result.rows_mut() {
        magnitude_to_db(row.as_slice_mut().unwrap(), peak, 1e-5, None);
    }
    let (frames, bins) = result.dim();
    let seconds = signal.len() as f32 / sample_rate as f32;
    println!("{input}: {seconds:.1} s at {sample_rate} Hz");
    println!(
        "{frames} frames x {bins} bins in {:.1} ms",
        elapsed.as_secs_f64() * 1_000.0
    );

    let name = std::path::Path::new(input)
        .file_name()
        .map_or(input.into(), |n| n.to_string_lossy());
    let root = SVGBackend::new(output, (960, 480)).into_drawing_area();
    root.fill(&WHITE)?;
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("{name}: CQT magnitude (dB), 24 bins per octave"),
            ("sans-serif", 18),
        )
        .margin(10)
        .x_label_area_size(36)
        .y_label_area_size(56)
        .build_cartesian_2d(0f32..seconds, 0f32..bins as f32)?;
    chart
        .configure_mesh()
        .disable_mesh()
        .x_desc("time (s)")
        .y_desc("centre frequency (Hz)")
        .y_labels(8)
        .y_label_formatter(&|bin| format!("{:.0}", cqt.params().center_freq(*bin as usize)))
        .draw()?;

    // Each column covers `stride` frames and shows the first of them.
    let stride = frames.div_ceil(MAX_COLUMNS).max(1);
    let frame_seconds = hop as f32 / sample_rate as f32;
    let column_seconds = stride as f32 * frame_seconds;
    chart.draw_series((0..frames).step_by(stride).flat_map(|frame| {
        let row = result.row(frame);
        let x = frame as f32 * frame_seconds - frame_seconds / 2.0;
        (0..bins).map(move |bin| {
            let db = row[bin].clamp(-80.0, 0.0);
            Rectangle::new(
                [(x, bin as f32), (x + column_seconds, bin as f32 + 1.0)],
                viridis((db + 80.0) / 80.0).filled(),
            )
        })
    }))?;
    root.present()?;
    println!("wrote {output}");
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(input) = args.first() else {
        eprintln!("usage: spectrogram <input.wav> [out.svg]");
        std::process::exit(2);
    };
    let output = args.get(1).map_or("spectrogram.svg", String::as_str);
    if let Err(e) = run(input, output) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
