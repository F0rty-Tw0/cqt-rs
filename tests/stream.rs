mod common;

use cqt_rs::{Cqt, CqtError, CqtParams, CqtStream};

use common::{chirp, noise};

fn collect(cqt: &Cqt, signal: &[f32], hop: usize, chunk: usize) -> Vec<Vec<f32>> {
    let mut stream = CqtStream::new(cqt, hop).unwrap();
    let mut frames = Vec::new();
    for block in signal.chunks(chunk) {
        stream.push(cqt, block, |frame| frames.push(frame.to_vec()));
    }
    stream.flush(cqt, |frame| frames.push(frame.to_vec()));
    assert_eq!(stream.frames_emitted() as usize, frames.len());
    assert_eq!(stream.samples_consumed() as usize, signal.len());
    frames
}

#[test]
fn stream_matches_batch_for_any_chunking() {
    for multirate in [true, false] {
        let cqt = Cqt::new(
            CqtParams::builder(16_000, 100.0, 4_000.0)
                .multirate(multirate)
                .build()
                .unwrap(),
        );
        let signal = noise(16_000 * 2 + 123, 5);
        for hop in [1usize, 160, 512, 4_096] {
            let batch = cqt.process(&signal, hop).unwrap();
            for chunk in [1usize, 7, 480, 4_000, signal.len()] {
                if hop == 1 && chunk == 1 {
                    continue; // too slow to be worth it
                }
                let frames = collect(&cqt, &signal, hop, chunk);
                assert_eq!(frames.len(), batch.nrows(), "hop {hop} chunk {chunk}");
                for (i, frame) in frames.iter().enumerate() {
                    for (bin, value) in frame.iter().enumerate() {
                        assert_eq!(*value, batch[[i, bin]], "frame {i} bin {bin}");
                    }
                }
            }
        }
    }
}

#[test]
fn frames_arrive_with_fixed_latency() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let hop = 4 * cqt.hop_alignment();
    let mut stream = CqtStream::new(&cqt, hop).unwrap();
    let latency = stream.latency_samples();
    assert_eq!(latency, cqt.latency_samples());
    assert_eq!(stream.hop_size(), hop);

    let mut emitted = 0;
    // Frame i needs sample i * hop + latency - 1 to be present.
    for pushed in 1..=(latency + 3 * hop) {
        stream.push(&cqt, &[0.0], |_| emitted += 1);
        let expected = if pushed >= latency {
            1 + (pushed - latency) / hop
        } else {
            0
        };
        assert_eq!(emitted, expected, "after {pushed} samples");
    }
}

#[test]
fn reset_restarts_numbering() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let signal = noise(8_000, 9);
    let mut stream = CqtStream::new(&cqt, 200).unwrap();
    stream.push(&cqt, &signal, |_| {});
    stream.reset(&cqt);
    assert_eq!(stream.frames_emitted(), 0);
    assert_eq!(stream.samples_consumed(), 0);
    let mut again = Vec::new();
    stream.push(&cqt, &signal, |f| again.push(f.to_vec()));
    stream.flush(&cqt, |f| again.push(f.to_vec()));
    let batch = cqt.process(&signal, 200).unwrap();
    assert_eq!(again.len(), batch.nrows());
}

#[test]
fn flush_is_idempotent_and_continues_counting() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let signal = noise(8_000, 1);
    let mut stream = CqtStream::new(&cqt, 200).unwrap();
    let mut count = 0;
    stream.push(&cqt, &signal, |_| count += 1);
    stream.flush(&cqt, |_| count += 1);
    stream.flush(&cqt, |_| count += 1);
    assert_eq!(count, 1 + 8_000 / 200);
    // More audio after a flush follows the padding that the flush
    // appended, exactly as if that silence had been pushed.
    let padding = stream.padding_samples() as usize;
    assert!(padding >= cqt.latency_samples());
    let mut frames = Vec::new();
    stream.push(&cqt, &signal, |f| frames.push(f.to_vec()));
    stream.flush(&cqt, |f| frames.push(f.to_vec()));
    assert_eq!(count + frames.len(), 1 + (16_000 + padding) / 200);
    let mut padded = signal.clone();
    padded.extend(std::iter::repeat_n(0.0, padding));
    padded.extend(&signal);
    let batch = cqt.process(&padded, 200).unwrap();
    assert_eq!(count + frames.len(), batch.nrows());
    for (i, frame) in frames.iter().enumerate() {
        let row = batch.row(count + i);
        for (a, b) in frame.iter().zip(row.iter()) {
            assert!(
                (a - b).abs() <= 1e-4 * (1.0 + b.abs()),
                "frame {}",
                count + i
            );
        }
    }
}

#[test]
fn reset_rebinds_the_stream_to_another_transform() {
    let single = Cqt::new(
        CqtParams::builder(16_000, 100.0, 4_000.0)
            .multirate(false)
            .build()
            .unwrap(),
    );
    let multi = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let signal = noise(8_000, 2);
    let mut stream = CqtStream::new(&single, 200).unwrap();
    stream.push(&single, &signal, |_| {});
    stream.reset(&multi);
    assert_eq!(stream.params(), multi.params());
    assert_eq!(stream.latency_samples(), multi.latency_samples());
    let mut frames = Vec::new();
    stream.push(&multi, &signal, |f| frames.push(f.to_vec()));
    stream.flush(&multi, |f| frames.push(f.to_vec()));
    let batch = multi.process(&signal, 200).unwrap();
    assert_eq!(frames.len(), batch.nrows());
    assert_eq!(frames[0].len(), multi.num_bins());
}

#[test]
#[should_panic(expected = "not created for")]
fn pushing_through_another_transform_panics() {
    let a = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let b = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 24).unwrap());
    let mut stream = CqtStream::new(&a, 200).unwrap();
    stream.push(&b, &noise(1_000, 3), |_| {});
}

#[test]
fn rejects_bad_hop() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    assert_eq!(
        CqtStream::new(&cqt, 0).unwrap_err(),
        CqtError::InvalidHopSize
    );
}

const SR: u32 = 44_100;
const TEN_SECONDS: usize = 10 * SR as usize;

fn fingerprint_cqt() -> Cqt {
    Cqt::new(
        CqtParams::builder(SR, 55.0, 7_040.0)
            .bins_per_octave(24)
            .build()
            .unwrap(),
    )
}

/// Pushes 20 s of chirp at hop 768 in 1 s chunks, checking the frames
/// against the batch transform.
fn coarse_stream(cqt: &Cqt, signal: &[f32], retain: bool) -> CqtStream {
    let mut stream = CqtStream::new(cqt, 768).unwrap();
    if retain {
        stream.retain_samples(TEN_SECONDS);
    }
    let batch = cqt.process(signal, 768).unwrap();
    let mut i = 0;
    for chunk in signal.chunks(SR as usize) {
        stream.push(cqt, chunk, |frame| {
            assert_eq!(frame, batch.row(i).as_slice().unwrap(), "frame {i}");
            i += 1;
        });
    }
    assert_eq!(stream.next_centre(), 768 * i as i64);
    stream
}

/// Pushes `signal` and checks every frame against `batch` at the centres
/// the stream reports, which must lie on `hop`.
fn push_checked(
    cqt: &Cqt,
    stream: &mut CqtStream,
    signal: &[f32],
    batch: &ndarray::Array2<f32>,
    hop: usize,
) -> usize {
    let start = stream.next_centre();
    assert_eq!(start % hop as i64, 0);
    let mut n = 0;
    stream.push(cqt, signal, |frame| {
        let centre = start + (n * hop) as i64;
        let row = (centre / hop as i64) as usize;
        assert_eq!(frame, batch.row(row).as_slice().unwrap(), "centre {centre}");
        n += 1;
    });
    assert_eq!(stream.next_centre(), start + (n * hop) as i64);
    n
}

#[test]
fn replay_re_analyses_retained_history_at_a_finer_hop() {
    let cqt = fingerprint_cqt();
    let signal = chirp(SR, 100.0, 5_000.0, 20.0);
    let mut stream = coarse_stream(&cqt, &signal, true);
    let fine = cqt.process(&signal, 256).unwrap();
    let from = stream.samples_consumed() as i64 - TEN_SECONDS as i64;
    let mut replayed = 0;
    let mut last = None;
    let first = stream
        .replay(&cqt, from, 256, |centre, frame| {
            assert_eq!(centre % 256, 0);
            assert!(last.is_none_or(|last| centre > last));
            let row = (centre / 256) as usize;
            assert_eq!(frame, fine.row(row).as_slice().unwrap(), "centre {centre}");
            last = Some(centre);
            replayed += 1;
        })
        .unwrap()
        .unwrap();
    let grid_from = (from + 255) / 256 * 256;
    assert!(
        (grid_from..grid_from + 256).contains(&first),
        "first {first} vs {grid_from}"
    );
    // Every centre from the first up to the next frame is covered.
    let expected = (stream.next_centre() - first + 255) / 256;
    assert_eq!(replayed, expected);
    // Replay leaves the stream where it was.
    assert_eq!(stream.hop_size(), 768);
    assert_eq!(stream.next_centre(), 768 * stream.frames_emitted() as i64);
}

#[test]
fn set_hop_keeps_centres_on_the_new_grid_and_increasing() {
    let cqt = fingerprint_cqt();
    let signal = chirp(SR, 100.0, 5_000.0, 30.0);
    let twenty = 20 * SR as usize;
    let twenty_five = 25 * SR as usize;
    let mut stream = coarse_stream(&cqt, &signal[..twenty], true);
    let emitted = stream.frames_emitted();

    stream.set_hop(&cqt, 256).unwrap();
    assert_eq!(stream.hop_size(), 256);
    // 768 is a multiple of 256, so the next centre is unchanged.
    assert_eq!(stream.next_centre(), 768 * emitted as i64);
    let fine = cqt.process(&signal, 256).unwrap();
    let n = push_checked(&cqt, &mut stream, &signal[twenty..twenty_five], &fine, 256);
    assert_eq!(n, 5 * SR as usize / 256);
    assert_eq!(stream.frames_emitted(), emitted + n as u64);

    let before = stream.next_centre();
    stream.set_hop(&cqt, 768).unwrap();
    assert!(stream.next_centre() >= before);
    assert!(stream.next_centre() < before + 768);
    let coarse = cqt.process(&signal, 768).unwrap();
    let n = push_checked(&cqt, &mut stream, &signal[twenty_five..], &coarse, 768);
    assert!(n >= 5 * SR as usize / 768 - 1, "{n}");
}

#[test]
fn replay_without_retention_finds_little_history() {
    let cqt = fingerprint_cqt();
    let signal = chirp(SR, 100.0, 5_000.0, 20.0);
    let from = signal.len() as i64 - TEN_SECONDS as i64;

    let mut retained = coarse_stream(&cqt, &signal, true);
    let mut with = 0;
    retained.replay(&cqt, from, 256, |_, _| with += 1).unwrap();

    let mut dropped = coarse_stream(&cqt, &signal, false);
    let mut without = 0;
    let first = dropped
        .replay(&cqt, from, 256, |_, _| without += 1)
        .unwrap();
    assert!(without < with, "{without} vs {with}");
    if let Some(first) = first {
        assert!(first > from + TEN_SECONDS as i64 / 2, "{first}");
    }
}

#[test]
fn set_hop_rejects_bad_hop() {
    let cqt = fingerprint_cqt();
    let mut stream = CqtStream::new(&cqt, 768).unwrap();
    assert_eq!(
        stream.set_hop(&cqt, 0).unwrap_err(),
        CqtError::InvalidHopSize
    );
    assert_eq!(stream.hop_size(), 768);
    assert_eq!(
        stream.replay(&cqt, 0, 0, |_, _| {}).unwrap_err(),
        CqtError::InvalidHopSize
    );
}

/// The chirp's instantaneous frequency must track `start + (end - start) t / T`
/// rather than run at twice the slope; the frame at t = 0 is skipped because
/// its half-empty window smears the first 0.2 s.
#[test]
fn chirp_sweeps_from_start_to_end_frequency() {
    let cqt = fingerprint_cqt();
    let hop = 4_410;
    let signal = chirp(SR, 100.0, 5_000.0, 20.0);
    let frames = cqt.process(&signal, hop).unwrap();
    let freqs = cqt.frequencies();
    let nearest_bin =
        |hz: f32| common::argmax(&freqs.iter().map(|f| -(f - hz).abs()).collect::<Vec<_>>());
    for row in [1, frames.nrows() - 1] {
        let t = (row * hop) as f32 / SR as f32;
        let expected = nearest_bin(100.0 + (5_000.0 - 100.0) * t / 20.0);
        let bin = common::argmax(frames.row(row).as_slice().unwrap());
        assert!(
            bin.abs_diff(expected) <= 1,
            "row {row}: bin {bin}, expected {expected}"
        );
    }
}
