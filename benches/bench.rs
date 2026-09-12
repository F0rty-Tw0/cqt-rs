use std::hint::black_box;

use cqt_rs::{Cqt, CqtParams, CqtStream};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};

fn chirp(sample_rate: u32, start_hz: f32, end_hz: f32, seconds: f32) -> Vec<f32> {
    let n = (sample_rate as f32 * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let f = start_hz + (end_hz - start_hz) * t / seconds;
            (std::f32::consts::TAU * f * t).sin()
        })
        .collect()
}

/// Same signal and grid as the 0.1 benchmark: 30 s at 22 kHz, 14.568 Hz to
/// 7902 Hz, 12 bins per octave, hop 1760.
fn legacy_params() -> CqtParams {
    CqtParams::new(22_000, 14.568, 7_902.1, 12).unwrap()
}

/// The legacy grid analysed with one FFT at the full sample rate.
fn legacy_single_rate_params() -> CqtParams {
    CqtParams::builder(22_000, 14.568, 7_902.1)
        .multirate(false)
        .build()
        .unwrap()
}

/// The legacy grid with kernels capped at the 2048-sample window 0.1 used.
fn legacy_capped_params() -> CqtParams {
    CqtParams::builder(22_000, 14.568, 7_902.1)
        .max_kernel_length(2_048)
        .build()
        .unwrap()
}

/// A fingerprinting configuration: 55 Hz to 7040 Hz, 24 bins per octave.
fn fingerprint_params() -> CqtParams {
    CqtParams::builder(44_100, 55.0, 7_040.0)
        .bins_per_octave(24)
        .build()
        .unwrap()
}

fn bench_kernel(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("kernel");
    for (name, params) in [
        ("legacy", legacy_params()),
        ("legacy_single_rate", legacy_single_rate_params()),
        ("legacy_capped", legacy_capped_params()),
        ("fingerprint", fingerprint_params()),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter(|| black_box(Cqt::new(black_box(params.clone()))));
        });
    }
    group.finish();
}

fn bench_process(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("process");
    let legacy_signal = chirp(22_000, 440.0, 440.0, 30.0);
    group.throughput(Throughput::Elements(legacy_signal.len() as u64));
    for (name, params, hop) in [
        ("legacy_30s", legacy_params(), 1_760),
        ("legacy_single_rate_30s", legacy_single_rate_params(), 1_760),
        ("legacy_capped_30s", legacy_capped_params(), 1_760),
    ] {
        let cqt = Cqt::new(params);
        group.bench_function(name, |bencher| {
            bencher.iter(|| black_box(cqt.process(black_box(&legacy_signal), hop).unwrap()));
        });
    }
    let cqt = Cqt::new(legacy_params());
    let mut workspace = cqt.workspace();
    group.bench_function("legacy_30s_reused_workspace", |bencher| {
        bencher.iter(|| {
            black_box(
                cqt.process_with(&mut workspace, black_box(&legacy_signal), 1_760)
                    .unwrap(),
            )
        });
    });
    let signal = chirp(44_100, 55.0, 7_000.0, 10.0);
    group.throughput(Throughput::Elements(signal.len() as u64));
    let cqt = Cqt::new(fingerprint_params());
    group.bench_function("fingerprint_10s", |bencher| {
        bencher.iter(|| black_box(cqt.process(black_box(&signal), 512).unwrap()));
    });
    let mut workspace = cqt.workspace();
    group.bench_function("fingerprint_10s_reused_workspace", |bencher| {
        bencher.iter(|| {
            black_box(
                cqt.process_with(&mut workspace, black_box(&signal), 512)
                    .unwrap(),
            )
        });
    });
    group.finish();
}

fn bench_stream(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("stream");
    for (name, params, hop) in [
        ("legacy_frame", legacy_params(), 1_760),
        (
            "legacy_single_rate_frame",
            legacy_single_rate_params(),
            1_760,
        ),
        ("fingerprint_frame", fingerprint_params(), 512),
    ] {
        let cqt = Cqt::new(params);
        let mut stream = CqtStream::new(&cqt, hop).unwrap();
        let block = chirp(cqt.params().sample_rate(), 440.0, 440.0, 1.0);
        let block = &block[..hop];
        // Prime the buffer so every push emits exactly one frame.
        stream.push(&cqt, &vec![0.0; cqt.latency_samples() + hop], |_| {});
        group.throughput(Throughput::Elements(hop as u64));
        group.bench_function(name, |bencher| {
            bencher.iter(|| {
                stream.push(&cqt, black_box(block), |frame| {
                    black_box(frame);
                });
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_kernel, bench_process, bench_stream);
criterion_main!(benches);
