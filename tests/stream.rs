mod common;

use cqt_rs::{Cqt, CqtError, CqtParams, CqtStream};

use common::noise;

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
