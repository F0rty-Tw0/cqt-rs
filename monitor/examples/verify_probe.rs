//! Deterministic stage timing probe; see docs/experiments/E005-verifier-window.md.

use std::hint::black_box;
use std::time::{Duration, Instant};

use cqt_monitor::{Peak, PeakTrack};

fn main() {
    for (name, size, count) in [
        ("small", 128, 64),
        ("medium", 4096, 256),
        ("large", 65536, 256),
        ("whole", 4096, 4096),
    ] {
        let mut seed = 42u64;
        let reference: Vec<Peak> = (0..size)
            .map(|i| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                Peak {
                    frame: i * 3,
                    bin: (seed % 128) as u32,
                }
            })
            .collect();
        let start = (size - count) as usize / 2;
        let offset = reference[start].frame as f64 - 100.0;
        let query: Vec<Peak> = reference[start..start + count as usize]
            .iter()
            .map(|p| Peak {
                frame: p.frame - reference[start].frame + 100,
                bin: p.bin + 7,
            })
            .collect();
        let track = PeakTrack::new(reference);
        for (kind, shift) in [("match", 7), ("wrong_shift", 70)] {
            let verify = || {
                black_box(&track).verify(
                    black_box(&query),
                    black_box(shift),
                    black_box(1.0),
                    black_box(offset),
                    2,
                    0,
                )
            };
            for _ in 0..32 {
                black_box(verify());
            }
            let expected = verify();
            let started = Instant::now();
            let mut iterations = 0u64;
            while started.elapsed() < Duration::from_millis(200) {
                for _ in 0..16 {
                    black_box(verify());
                }
                iterations += 16;
            }
            let elapsed = started.elapsed().as_secs_f64();
            println!("{{\"case\":\"{name}_{kind}\",\"reference_size\":{size},\"query_size\":{count},\"iterations\":{iterations},\"elapsed_seconds\":{elapsed},\"ns_per_call\":{},\"counts\":[{},{},{},{}]}}",
                elapsed * 1e9 / iterations as f64,
                expected.query_peaks, expected.query_matched, expected.reference_peaks, expected.reference_matched);
        }
    }
}
