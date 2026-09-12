//! Half-band decimation filter for the multi-rate transform.

use std::f64::consts::PI;

/// Symmetric, odd-length half-band low-pass filter used to halve the sample
/// rate between octaves.
///
/// Only the centre tap and the odd taps are non-zero, so `y[n]` costs about
/// a quarter of the tap count in multiply-adds:
/// `y[n] = h0 * x[2n] + Σ_m h_m * (x[2n - m] + x[2n + m])` over odd `m`.
#[derive(Debug, Clone, PartialEq)]
pub struct HalfBand {
    centre: f32,
    /// `(offset, coefficient)` for odd offsets `1, 3, 5, ...`.
    odd_taps: Vec<(usize, f32)>,
    delay: usize,
}

impl HalfBand {
    /// Designs a Kaiser-windowed half-band filter with the given normalized
    /// transition width (`0..1`, as a fraction of the input Nyquist
    /// frequency) and stop-band attenuation in dB. The pass-band therefore
    /// extends to `(1 - transition) / 2` of the input Nyquist frequency.
    pub fn design(transition: f64, attenuation_db: f64) -> Self {
        let taps = Self::taps_for(transition, attenuation_db);
        let beta = if attenuation_db > 50.0 {
            0.1102 * (attenuation_db - 8.7)
        } else if attenuation_db >= 21.0 {
            0.5842 * (attenuation_db - 21.0).powf(0.4) + 0.07886 * (attenuation_db - 21.0)
        } else {
            0.0
        };
        let delay = (taps - 1) / 2;

        let window = |n: f64| {
            let x = n / delay as f64;
            bessel_i0(beta * (1.0 - x * x).max(0.0).sqrt()) / bessel_i0(beta)
        };
        let mut odd_taps: Vec<(usize, f64)> = (1..=delay)
            .step_by(2)
            .map(|m| {
                let n = m as f64;
                let h = (PI * n / 2.0).sin() / (PI * n) * window(n);
                (m, h)
            })
            .collect();
        // Normalize the DC gain to exactly one.
        let sum = 0.5 + 2.0 * odd_taps.iter().map(|&(_, h)| h).sum::<f64>();
        let centre = 0.5 / sum;
        for (_, h) in &mut odd_taps {
            *h /= sum;
        }
        Self {
            centre: centre as f32,
            odd_taps: odd_taps.into_iter().map(|(m, h)| (m, h as f32)).collect(),
            delay,
        }
    }

    /// Number of taps [`HalfBand::design`] would use for the given
    /// transition width and attenuation, without building the filter.
    pub fn taps_for(transition: f64, attenuation_db: f64) -> usize {
        let transition = transition.clamp(1e-6, 1.0);
        let delta_omega = transition * PI;
        let taps = ((attenuation_db - 8.0) / (2.285 * delta_omega)).ceil() as usize + 1;
        // Odd length so the delay is an integer; 4k + 3 keeps the outermost
        // taps non-zero.
        let taps = taps.max(3);
        if taps % 4 == 3 {
            taps
        } else {
            taps + (7 - taps % 4) % 4
        }
    }

    /// Number of taps `2 * delay + 1`.
    pub fn taps(&self) -> usize {
        2 * self.delay + 1
    }

    /// Group delay in input samples.
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// Computes one decimated output sample centred on `input[centre]`. The
    /// caller guarantees `input[centre - delay..=centre + delay]` exists.
    #[inline(always)]
    pub fn sample(&self, input: &[f32], centre: usize) -> f32 {
        let mut acc = self.centre * input[centre];
        for &(m, h) in &self.odd_taps {
            acc += h * (input[centre - m] + input[centre + m]);
        }
        acc
    }

    /// Computes `out.len()` consecutive decimated samples; output `i` is
    /// centred on `input[first_centre + 2 * i]`. The caller guarantees that
    /// `input[first_centre - delay..=first_centre + 2 * (out.len() - 1) + delay]`
    /// exists. `odd` is scratch space that is reused between calls.
    ///
    /// The taps at odd offsets only touch samples of the opposite parity to
    /// the centres, so those samples are de-interleaved once into `odd` and
    /// every tap then becomes a contiguous multiply-add over the block.
    pub fn decimate_block(
        &self,
        input: &[f32],
        first_centre: usize,
        out: &mut [f32],
        odd: &mut Vec<f32>,
    ) {
        let len = out.len();
        if len == 0 {
            return;
        }
        let delay = self.delay;
        let first_odd = first_centre - delay;
        odd.clear();
        odd.extend(
            input[first_odd..first_centre + 2 * len - 1 + delay]
                .iter()
                .step_by(2),
        );
        debug_assert_eq!(odd.len(), len + delay);

        let centre = self.centre;
        for (o, &x) in out.iter_mut().zip(input[first_centre..].iter().step_by(2)) {
            *o = centre * x;
        }
        for &(m, h) in &self.odd_taps {
            let a = (delay - m) / 2;
            let b = (delay + m) / 2;
            let left = &odd[a..a + len];
            let right = &odd[b..b + len];
            for ((o, l), r) in out.iter_mut().zip(left).zip(right) {
                *o += h * (l + r);
            }
        }
    }
}

/// Modified Bessel function of the first kind, order zero.
fn bessel_i0(x: f64) -> f64 {
    let half = x / 2.0;
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..64 {
        term *= (half / k as f64) * (half / k as f64);
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(filter: &HalfBand, normalized_freq: f64) -> f64 {
        // Frequency response of the symmetric filter at `normalized_freq`
        // (1 = Nyquist).
        let omega = PI * normalized_freq;
        let mut h = f64::from(filter.centre);
        for &(m, c) in &filter.odd_taps {
            h += 2.0 * f64::from(c) * (omega * m as f64).cos();
        }
        h
    }

    #[test]
    fn passes_low_and_rejects_high_frequencies() {
        let filter = HalfBand::design(0.2, 80.0);
        assert_eq!(filter.taps() % 2, 1);
        assert!((response(&filter, 0.0) - 1.0).abs() < 1e-6);
        assert!((response(&filter, 0.3) - 1.0).abs() < 2e-4);
        assert!((response(&filter, 0.39) - 1.0).abs() < 2e-4);
        assert!(response(&filter, 0.5).abs() - 0.5 < 1e-6);
        assert!(response(&filter, 0.61).abs() < 2e-4);
        assert!(response(&filter, 0.9).abs() < 2e-4);
    }

    #[test]
    fn narrower_transition_needs_more_taps() {
        assert!(HalfBand::design(0.1, 80.0).taps() > HalfBand::design(0.4, 80.0).taps());
    }

    #[test]
    fn tap_count_is_known_before_design() {
        for &(transition, attenuation) in &[(0.1, 80.0), (0.4, 80.0), (0.9, 30.0), (1e-9, 80.0)] {
            if transition < 1e-4 {
                // Far too long to build; only the count is checked.
                assert!(HalfBand::taps_for(transition, attenuation) > 1_000_000);
                continue;
            }
            assert_eq!(
                HalfBand::design(transition, attenuation).taps(),
                HalfBand::taps_for(transition, attenuation)
            );
        }
    }

    #[test]
    fn block_and_sample_paths_agree() {
        let filter = HalfBand::design(0.25, 80.0);
        let input: Vec<f32> = (0..2_000)
            .map(|i| ((i * 7919) % 997) as f32 / 500.0 - 1.0)
            .collect();
        let first_centre = filter.delay() + 1;
        let len = 700;
        let mut block = vec![0.0; len];
        let mut odd = Vec::new();
        filter.decimate_block(&input, first_centre, &mut block, &mut odd);
        for (i, &value) in block.iter().enumerate() {
            let expected = filter.sample(&input, first_centre + 2 * i);
            assert!(
                (value - expected).abs() < 1e-6,
                "{i}: {value} vs {expected}"
            );
        }
    }

    #[test]
    fn decimates_a_constant_without_gain() {
        let filter = HalfBand::design(0.3, 60.0);
        let input = vec![0.75f32; 4 * filter.taps()];
        let y = filter.sample(&input, 2 * filter.taps());
        assert!((y - 0.75).abs() < 1e-6);
    }
}
