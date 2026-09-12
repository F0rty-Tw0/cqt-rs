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
    // More audio after a flush is treated as following silence.
    stream.push(&cqt, &signal, |_| count += 1);
    stream.flush(&cqt, |_| count += 1);
    assert_eq!(count, 1 + 16_000 / 200);
}

#[test]
fn rejects_bad_hop() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    assert_eq!(
        CqtStream::new(&cqt, 0).unwrap_err(),
        CqtError::InvalidHopSize
    );
}
