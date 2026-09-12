//! Per-frame cost of the streaming path for a few configurations.
use std::time::Instant;

use cqt_rs::{Cqt, CqtParams, CqtStream};

fn main() {
    let configs = [
        (
            "legacy",
            CqtParams::new(22_000, 14.568, 7_902.1, 12).unwrap(),
            1_760,
        ),
        (
            "legacy_single_rate",
            CqtParams::builder(22_000, 14.568, 7_902.1)
                .multirate(false)
                .build()
                .unwrap(),
            1_760,
        ),
        (
            "fingerprint",
            CqtParams::builder(44_100, 55.0, 7_040.0)
                .bins_per_octave(24)
                .build()
                .unwrap(),
            512,
        ),
        (
            "fingerprint_single_rate",
            CqtParams::builder(44_100, 55.0, 7_040.0)
                .bins_per_octave(24)
                .multirate(false)
                .build()
                .unwrap(),
            512,
        ),
        (
            "fingerprint_gamma",
            CqtParams::builder(44_100, 55.0, 7_040.0)
                .bins_per_octave(24)
                .gamma(10.0)
                .build()
                .unwrap(),
            512,
        ),
    ];
    for (name, params, hop) in configs {
        let t = Instant::now();
        let cqt = Cqt::new(params);
        let build = t.elapsed();
        let mut stream = CqtStream::new(&cqt, hop).unwrap();
        let block: Vec<f32> = (0..hop).map(|i| (i as f32 * 0.01).sin()).collect();
        stream.push(&cqt, &vec![0.0; cqt.latency_samples() + hop], |_| {});
        let iters = 500;
        let mut frames = 0;
        let t = Instant::now();
        for _ in 0..iters {
            stream.push(&cqt, &block, |_| frames += 1);
        }
        let per_frame = t.elapsed() / frames;
        println!(
            "{name:24} levels={} fft={:?} nnz={:6} taps={:3} latency={:6} build={build:?} frame={per_frame:?}",
            cqt.num_levels(),
            cqt.fft_lengths(),
            cqt.num_nonzeros(),
            cqt.decimation_taps(),
            cqt.latency_samples(),
        );
    }
}
