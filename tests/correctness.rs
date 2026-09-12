mod common;

use std::f64::consts::TAU;

use approx::assert_abs_diff_eq;
use cqt_rs::{Complex, Cqt, CqtError, CqtParams};
use hann_rs::{HannMode, hann_f64};

use common::{argmax, noise, tones};

/// Direct time-domain evaluation of the transform definition, in f64, for
/// a single-rate transform.
fn direct_cqt(cqt: &Cqt, signal: &[f32], center: usize) -> Vec<Complex<f64>> {
    let params = cqt.params();
    let fft_length = params.fft_length();
    let half = fft_length / 2;
    let sr = f64::from(params.sample_rate());
    (0..cqt.num_bins())
        .map(|bin| {
            let length = params.kernel_length(bin);
            let freq = f64::from(params.center_freq(bin));
            let window = hann_f64(length, HannMode::Periodic);
            let gain = 2.0 / window.iter().sum::<f64>();
            let start = (fft_length - length) / 2;
            let mut acc = Complex::<f64>::default();
            for (i, w) in window.iter().enumerate() {
                let frame_index = start + i;
                let n = frame_index as f64 - half as f64;
                let sample_index = center as isize - half as isize + frame_index as isize;
                if sample_index < 0 || sample_index as usize >= signal.len() {
                    continue;
                }
                let x = f64::from(signal[sample_index as usize]);
                let atom = Complex::from_polar(w * gain, TAU * freq * n / sr);
                acc += x * atom.conj();
            }
            acc
        })
        .collect()
}

#[test]
fn sparse_kernel_matches_direct_evaluation() {
    let params = CqtParams::builder(8_000, 60.0, 3_000.0)
        .bins_per_octave(12)
        .sparsity(0.0)
        .multirate(false)
        .build()
        .unwrap();
    let cqt = Cqt::new(params);
    let signal = noise(8_000, 7);
    let hop = 400;
    let result = cqt.process_complex(&signal, hop).unwrap();

    for frame in [0, 3, 10, 20] {
        let expected = direct_cqt(&cqt, &signal, frame * hop);
        for bin in 0..cqt.num_bins() {
            let got = result[[frame, bin]];
            assert_abs_diff_eq!(f64::from(got.re), expected[bin].re, epsilon = 2e-4);
            assert_abs_diff_eq!(f64::from(got.im), expected[bin].im, epsilon = 2e-4);
        }
    }
}

#[test]
fn default_sparsity_is_a_small_approximation() {
    let params = CqtParams::builder(8_000, 60.0, 3_000.0)
        .multirate(false)
        .build()
        .unwrap();
    let cqt = Cqt::new(params);
    let signal = noise(8_000, 11);
    let hop = 400;
    let result = cqt.process(&signal, hop).unwrap();
    let frame = 10;
    let expected = direct_cqt(&cqt, &signal, frame * hop);
    let peak = expected.iter().map(|c| c.norm()).fold(0.0, f64::max);
    for bin in 0..cqt.num_bins() {
        let error = (f64::from(result[[frame, bin]]) - expected[bin].norm()).abs();
        assert!(
            error < 0.02 * peak,
            "bin {bin}: error {error} vs peak {peak}"
        );
    }
}

#[test]
fn multirate_matches_single_rate() {
    let single = Cqt::new(
        CqtParams::builder(22_050, 32.7, 7_040.0)
            .bins_per_octave(24)
            .multirate(false)
            .build()
            .unwrap(),
    );
    let multi = Cqt::new(
        CqtParams::builder(22_050, 32.7, 7_040.0)
            .bins_per_octave(24)
            .build()
            .unwrap(),
    );
    assert_eq!(multi.num_levels(), 8);
    assert_eq!(single.num_levels(), 1);
    assert!(multi.num_nonzeros() * 10 < single.num_nonzeros());

    let signal: Vec<f32> = noise(44_100, 13)
        .iter()
        .zip(harmonic_signal())
        .map(|(n, h)| 0.05 * n + h)
        .collect();
    let hop = 512;
    let a = single.process(&signal, hop).unwrap();
    let b = multi.process(&signal, hop).unwrap();
    assert_eq!(a.dim(), b.dim());
    let peak = a.iter().cloned().fold(0.0, f32::max);
    let mut worst = 0.0f32;
    for frame in 0..a.nrows() {
        for bin in 0..a.ncols() {
            worst = worst.max((a[[frame, bin]] - b[[frame, bin]]).abs());
        }
    }
    assert!(
        worst < 0.01 * peak,
        "worst deviation {worst} vs peak {peak}"
    );
}

fn harmonic_signal() -> Vec<f32> {
    tones(
        22_050,
        2.0,
        &[
            (110.0, 0.5),
            (220.0, 0.4),
            (330.0, 0.3),
            (1_760.0, 0.2),
            (5_000.0, 0.1),
        ],
    )
}

#[test]
fn sinusoid_at_bin_center_has_unit_magnitude() {
    let params = CqtParams::new(44_100, 55.0, 7_040.0, 12).unwrap();
    let cqt = Cqt::new(params);
    for bin in [0, 12, 36, 60, cqt.num_bins() - 1] {
        let freq = f64::from(cqt.frequencies()[bin]);
        let signal = tones(44_100, 3.0, &[(freq, 1.0)]);
        let hop = 4_410;
        let result = cqt.process(&signal, hop).unwrap();
        // A frame well inside the signal.
        let frame = result.nrows() / 2;
        let row = result.row(frame);
        assert_eq!(argmax(row.as_slice().unwrap()), bin);
        assert_abs_diff_eq!(row[bin], 1.0, epsilon = 0.02);
        if bin + 1 < cqt.num_bins() {
            assert!(row[bin + 1] < 0.7);
        }
        if bin > 0 {
            assert!(row[bin - 1] < 0.7);
        }
    }
}

#[test]
fn output_shape_and_frame_centering() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    let signal = noise(10_000, 2);
    let hop = 160;
    let result = cqt.process(&signal, hop).unwrap();
    assert_eq!(result.dim(), (1 + 10_000 / 160, cqt.num_bins()));
    assert_eq!(cqt.num_frames(10_000, 160), result.nrows());

    // Frame 0 is centred on sample 0 and frame `n` on the last hop boundary,
    // so with a stationary sinusoid both see half a window of signal and
    // report half the magnitude of an interior frame.
    let bin = 24;
    let freq = f64::from(cqt.frequencies()[bin]);
    let signal = tones(16_000, 1.0, &[(freq, 1.0)]);
    let result = cqt.process(&signal, 400).unwrap();
    let middle = result[[result.nrows() / 2, bin]];
    assert_abs_diff_eq!(middle, 1.0, epsilon = 0.02);
    assert_abs_diff_eq!(result[[0, bin]], 0.5, epsilon = 0.03);
    assert_abs_diff_eq!(result[[result.nrows() - 1, bin]], 0.5, epsilon = 0.03);
}

#[test]
fn rejects_bad_input() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 12).unwrap());
    assert_eq!(cqt.process(&[], 100).unwrap_err(), CqtError::EmptySignal);
    assert_eq!(
        cqt.process(&[0.0; 100], 0).unwrap_err(),
        CqtError::InvalidHopSize
    );
    // Hops beyond the shortest frame are allowed; the top octave then skips
    // samples between frames.
    let big = cqt.frame_spans().iter().copied().max().unwrap() * 2;
    assert_eq!(cqt.process(&[0.0; 100], big).unwrap().nrows(), 1);
}

#[test]
fn magnitude_and_complex_paths_agree() {
    let cqt = Cqt::new(CqtParams::new(16_000, 100.0, 4_000.0, 24).unwrap());
    let signal = noise(16_000, 3);
    let magnitudes = cqt.process(&signal, 256).unwrap();
    let complex = cqt.process_complex(&signal, 256).unwrap();
    for (m, c) in magnitudes.iter().zip(complex.iter()) {
        assert_abs_diff_eq!(*m, c.norm(), epsilon = 1e-6);
    }
}

#[test]
fn transform_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Cqt>();
    assert_send_sync::<CqtParams>();
}
