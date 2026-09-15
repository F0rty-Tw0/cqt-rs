//! Runs the transform over a WAV file and writes the magnitude spectrogram
//! as a NumPy `.npy` array of shape `(frames, bins)`, for the validation
//! script in `scripts/fingerprint_demo.py`.
//!
//! ```console
//! cargo run --release --example cqt_dump -- input.wav output.npy [hop] [min_freq] [max_freq] [bins_per_octave]
//! ```

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};

use cqt_rs::{Cqt, CqtParams};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: cqt_dump input.wav output.npy [hop] [min_freq] [max_freq] [bins_per_octave]"
        );
        std::process::exit(2);
    }
    let hop: usize = args.get(3).map_or(Ok(512), |s| s.parse())?;
    let min_freq: f32 = args.get(4).map_or(Ok(55.0), |s| s.parse())?;
    let max_freq: f32 = args.get(5).map_or(Ok(7_040.0), |s| s.parse())?;
    let bins_per_octave: usize = args.get(6).map_or(Ok(24), |s| s.parse())?;

    let mut reader = hound::WavReader::open(&args[1])?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    // Mix down to mono.
    let mono: Vec<f32> = samples
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();

    let params = CqtParams::builder(spec.sample_rate, min_freq, max_freq)
        .bins_per_octave(bins_per_octave)
        .build()?;
    let cqt = Cqt::new(params);
    let start = std::time::Instant::now();
    let magnitudes = cqt.process(&mono, hop)?;
    let elapsed = start.elapsed();
    let (frames, bins) = magnitudes.dim();
    eprintln!(
        "{}: {} samples at {} Hz -> {frames} x {bins} in {elapsed:?} ({} levels, latency {} samples)",
        args[1],
        mono.len(),
        spec.sample_rate,
        cqt.num_levels(),
        cqt.latency_samples()
    );

    // Minimal .npy writer (format version 1.0, little-endian f32, C order).
    let mut out = BufWriter::new(File::create(&args[2])?);
    let mut header =
        format!("{{'descr': '<f4', 'fortran_order': False, 'shape': ({frames}, {bins}), }}");
    let padding = 64 - ((10 + header.len() + 1) % 64);
    header.extend(std::iter::repeat_n(' ', padding));
    header.push('\n');
    out.write_all(b"\x93NUMPY\x01\x00")?;
    out.write_all(&(header.len() as u16).to_le_bytes())?;
    out.write_all(header.as_bytes())?;
    for value in magnitudes.iter() {
        out.write_all(&value.to_le_bytes())?;
    }
    out.flush()?;
    Ok(())
}
