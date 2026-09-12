//! Feeds a synthetic signal through the streaming API in small blocks, the
//! way a real-time capture callback would, and prints the strongest bin of
//! every frame.

use cqt_rs::{Cqt, CqtParams, CqtStream, magnitude_to_db};

fn main() {
    let sample_rate = 44_100;
    let params = CqtParams::builder(sample_rate, 55.0, 7_040.0)
        .bins_per_octave(24)
        .gamma(10.0) // shorter low-frequency windows: lower latency
        .build()
        .unwrap();
    let cqt = Cqt::new(params);
    let hop = 512;
    let mut stream = CqtStream::new(&cqt, hop).unwrap();

    println!("{}", cqt.params());
    println!(
        "{} sample rates, fft lengths {:?}, latency {:.1} ms, {} non-zero kernel coefficients",
        cqt.num_levels(),
        cqt.fft_lengths(),
        cqt.latency_samples() as f32 * 1_000.0 / sample_rate as f32,
        cqt.num_nonzeros()
    );

    // A rising arpeggio: A3, C#4, E4, A4, each 250 ms.
    let notes = [220.0f32, 277.18, 329.63, 440.0];
    let signal: Vec<f32> = (0..sample_rate)
        .map(|n| {
            let t = n as f32 / sample_rate as f32;
            let note = notes[((t * 4.0) as usize).min(3)];
            0.5 * (std::f32::consts::TAU * note * t).sin()
        })
        .collect();

    let mut frame_index = 0usize;
    let mut db = vec![0.0f32; cqt.num_bins()];
    for block in signal.chunks(256) {
        stream.push(&cqt, block, |magnitudes| {
            db.copy_from_slice(magnitudes);
            magnitude_to_db(&mut db, 1.0, 1e-5, Some(60.0));
            let (bin, level) =
                db.iter()
                    .enumerate()
                    .fold((0, f32::NEG_INFINITY), |best, (i, &v)| {
                        if v > best.1 { (i, v) } else { best }
                    });
            if frame_index.is_multiple_of(8) {
                println!(
                    "frame {frame_index:3} @ {:6.1} ms: {:7.1} Hz {level:6.1} dB",
                    frame_index as f32 * hop as f32 * 1_000.0 / sample_rate as f32,
                    cqt.frequencies()[bin]
                );
            }
            frame_index += 1;
        });
    }
    stream.flush(&cqt, |_| frame_index += 1);
    println!("{frame_index} frames emitted");
}
