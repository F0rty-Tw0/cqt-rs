#![allow(dead_code)]

use std::f64::consts::TAU;

/// Deterministic white noise in `[-1, 1]`.
pub fn noise(len: usize, seed: u64) -> Vec<f32> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        })
        .map(|v| v as f32)
        .collect()
}

/// Sum of sinusoids with the given `(frequency, amplitude)` pairs.
pub fn tones(sample_rate: u32, seconds: f64, partials: &[(f64, f64)]) -> Vec<f32> {
    let n = (f64::from(sample_rate) * seconds) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(sample_rate);
            partials
                .iter()
                .map(|&(f, a)| a * (TAU * f * t).sin())
                .sum::<f64>() as f32
        })
        .collect()
}

/// Harmonic tone with `harmonics` partials decaying as `1 / k`.
pub fn harmonic_tone(sample_rate: u32, seconds: f64, f0: f64, harmonics: usize) -> Vec<f32> {
    let partials: Vec<(f64, f64)> = (1..=harmonics)
        .map(|k| (f0 * k as f64, 1.0 / k as f64))
        .collect();
    tones(sample_rate, seconds, &partials)
}

pub fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .fold((0, f32::NEG_INFINITY), |(bi, bv), (i, &v)| {
            if v > bv { (i, v) } else { (bi, bv) }
        })
        .0
}

/// Indices of the `n` largest values, descending.
pub fn top_n(values: &[f32], n: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..values.len()).collect();
    idx.sort_by(|&a, &b| values[b].partial_cmp(&values[a]).unwrap());
    idx.truncate(n);
    idx
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (na * nb)
}
