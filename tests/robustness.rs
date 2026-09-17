//! Properties a fingerprinting front end relies on: pitch shifts move energy
//! along the bin axis, tempo changes move it along the frame axis, and mild
//! distortion leaves the dominant bins in place.

mod common;

use cqt_rs::{Cqt, CqtParams, magnitude_to_db};

use common::{argmax, cosine_similarity, harmonic_tone, noise, tones, top_n};

const SR: u32 = 22_050;

fn fingerprint_cqt() -> Cqt {
    Cqt::new(
        CqtParams::builder(SR, 55.0, 7_040.0)
            .bins_per_octave(24)
            .build()
            .unwrap(),
    )
}

fn middle_row(cqt: &Cqt, signal: &[f32], hop: usize) -> Vec<f32> {
    let result = cqt.process(signal, hop).unwrap();
    result.row(result.nrows() / 2).to_vec()
}

#[test]
fn pitch_shift_translates_bins() {
    let cqt = fingerprint_cqt();
    let bins_per_octave = cqt.params().bins_per_octave() as f64;
    let reference = middle_row(&cqt, &harmonic_tone(SR, 2.0, 220.0, 6), 512);

    for shift_bins in [1usize, 3, 7, 24] {
        let ratio = 2f64.powf(shift_bins as f64 / bins_per_octave);
        let shifted = middle_row(&cqt, &harmonic_tone(SR, 2.0, 220.0 * ratio, 6), 512);

        // The fundamental and every harmonic move by exactly `shift_bins`.
        assert_eq!(argmax(&shifted), argmax(&reference) + shift_bins);

        // The whole pattern is a translation: compare the overlapping region.
        let n = reference.len() - shift_bins;
        let similarity = cosine_similarity(&reference[..n], &shifted[shift_bins..]);
        assert!(
            similarity > 0.99,
            "shift {shift_bins}: similarity {similarity}"
        );
    }
}

#[test]
fn tempo_change_scales_frame_positions() {
    let cqt = fingerprint_cqt();
    let hop = 256;
    let notes = [(0.3, 330.0), (0.8, 440.0), (1.4, 554.0)];

    // Three short notes; at a different tempo they start at scaled times.
    fn score(notes: &[(f64, f64)], speed: f64) -> Vec<f32> {
        let seconds = 2.0 / speed;
        let mut signal = vec![0.0f32; (f64::from(SR) * seconds) as usize];
        for &(start, freq) in notes {
            let note = tones(SR, 0.15 / speed, &[(freq, 0.8)]);
            let offset = (start / speed * f64::from(SR)) as usize;
            for (j, sample) in note.iter().enumerate() {
                signal[offset + j] += sample;
            }
        }
        signal
    }

    // First frame in which each note's bin rises above half its peak.
    fn onsets(cqt: &Cqt, notes: &[(f64, f64)], signal: &[f32], hop: usize) -> Vec<usize> {
        let result = cqt.process(signal, hop).unwrap();
        notes
            .iter()
            .map(|&(_, freq)| {
                let distances: Vec<f32> = cqt
                    .frequencies()
                    .iter()
                    .map(|&f| -(f - freq as f32).abs())
                    .collect();
                let column: Vec<f32> = result.column(argmax(&distances)).to_vec();
                let peak = column.iter().cloned().fold(0.0, f32::max);
                column.iter().position(|&v| v > 0.5 * peak).unwrap()
            })
            .collect()
    }

    let normal = onsets(&cqt, &notes, &score(&notes, 1.0), hop);
    let faster = onsets(&cqt, &notes, &score(&notes, 1.25), hop);
    // Every onset moves earlier, and inter-onset intervals shrink by the
    // speed factor (the kernel delay is the same in both and cancels).
    for (a, b) in normal.iter().zip(&faster) {
        assert!(b < a, "{normal:?} vs {faster:?}");
    }
    let interval = |o: &[usize]| (o[2] - o[0]) as f64;
    let ratio = interval(&normal) / interval(&faster);
    assert!((ratio - 1.25).abs() < 0.05, "interval ratio {ratio}");
}

#[test]
fn hard_clipping_keeps_dominant_bins() {
    let cqt = fingerprint_cqt();
    let clean = harmonic_tone(SR, 2.0, 196.0, 5);
    let clipped: Vec<f32> = clean.iter().map(|s| s.clamp(-0.25, 0.25)).collect();

    let a = middle_row(&cqt, &clean, 512);
    let b = middle_row(&cqt, &clipped, 512);
    assert_eq!(argmax(&a), argmax(&b));
    let top_clean = top_n(&a, 5);
    let top_clipped = top_n(&b, 5);
    let overlap = top_clean.iter().filter(|b| top_clipped.contains(b)).count();
    assert!(overlap >= 4, "top bins {top_clean:?} vs {top_clipped:?}");
}

#[test]
fn additive_noise_keeps_dominant_bins() {
    let cqt = fingerprint_cqt();
    let clean = harmonic_tone(SR, 2.0, 261.6, 4);
    let noisy: Vec<f32> = clean
        .iter()
        .zip(noise(clean.len(), 42))
        .map(|(s, n)| s + 0.2 * n)
        .collect();

    let a = middle_row(&cqt, &clean, 512);
    let b = middle_row(&cqt, &noisy, 512);
    assert_eq!(argmax(&a), argmax(&b));
    let top_clean = top_n(&a, 4);
    let top_noisy = top_n(&b, 4);
    assert!(top_clean.iter().all(|bin| top_noisy.contains(bin)));

    // In decibels the peaks stay within a couple of dB of each other.
    let (mut a_db, mut b_db) = (a.clone(), b.clone());
    magnitude_to_db(&mut a_db, 1.0, 1e-5, Some(80.0));
    magnitude_to_db(&mut b_db, 1.0, 1e-5, Some(80.0));
    for bin in top_clean {
        assert!((a_db[bin] - b_db[bin]).abs() < 2.0);
    }
}

#[test]
fn speed_change_moves_both_axes() {
    // Playing audio faster (resampling) shifts pitch and compresses time.
    let cqt = fingerprint_cqt();
    let bins_per_octave = cqt.params().bins_per_octave() as f64;
    let ratio = 2f64.powf(2.0 / bins_per_octave); // +2 bins ≈ 5.9 % faster
    let hop = 256;

    let normal = harmonic_tone(SR, 1.0, 300.0, 3);
    let faster = harmonic_tone(SR, 1.0 / ratio, 300.0 * ratio, 3);
    let a = cqt.process(&normal, hop).unwrap();
    let b = cqt.process(&faster, hop).unwrap();
    let frames_ratio = (a.nrows() - 1) as f64 / (b.nrows() - 1) as f64;
    assert!((frames_ratio - ratio).abs() < 0.02);
    let a_mid = a.row(a.nrows() / 2).to_vec();
    let b_mid = b.row(b.nrows() / 2).to_vec();
    assert_eq!(argmax(&b_mid), argmax(&a_mid) + 2);
}
