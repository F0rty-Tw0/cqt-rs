//! Multi-rate layout: which bins are analysed at which decimation level.

use crate::filter::HalfBand;
use crate::params::{CqtParams, CqtParamsError, HANN_BANDWIDTH};

/// Stop-band attenuation of the decimation filter in dB.
const ATTENUATION_DB: f64 = 80.0;
/// Longest decimation filter accepted before the configuration is rejected.
pub(crate) const MAX_FILTER_TAPS: usize = 4_095;

/// Bins that share one sample rate and one FFT.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Group {
    /// Index of the first bin.
    pub first_bin: usize,
    /// Number of consecutive bins.
    pub num_bins: usize,
    /// Number of halvings of the sample rate.
    pub level: usize,
    /// FFT length at the group's sample rate.
    pub fft_length: usize,
    /// Window length of each bin at the group's sample rate.
    pub lengths: Vec<usize>,
}

impl Group {
    /// Sample rate of the group in Hz.
    pub fn sample_rate(&self, full_rate: u32) -> f64 {
        f64::from(full_rate) / (1u64 << self.level) as f64
    }

    /// Number of full-rate samples per sample of this group.
    pub fn stride(&self) -> usize {
        1 << self.level
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plan {
    pub groups: Vec<Group>,
    pub num_levels: usize,
    /// Decimation filter, `None` when a single level is used.
    pub filter: Option<HalfBand>,
    /// Full-rate samples between a frame's centre and the last sample needed.
    pub latency: usize,
}

impl Plan {
    /// Extra full-rate samples the decimation cascade needs before the
    /// sample at `level` that a frame requires becomes available.
    ///
    /// Output `n` of a stage needs inputs up to `2n + delay`, so the last
    /// needed input index grows by `delay - 1` per stage relative to the
    /// doubled index.
    pub fn cascade_delay(&self, level: usize) -> usize {
        match &self.filter {
            Some(filter) => filter.delay().saturating_sub(1) * ((1usize << level) - 1),
            None => 0,
        }
    }
}

fn kernel_length_at(params: &CqtParams, bin: usize, level: usize) -> usize {
    let rate = f64::from(params.sample_rate()) / (1u64 << level) as f64;
    let length = ((rate / params.bandwidth_f64(bin)).round() as usize).max(2);
    match params.max_kernel_length() {
        Some(cap) => length.min((cap >> level).max(2)),
        None => length,
    }
}

/// Upper edge of the pass-band of the highest bin of `group`, in Hz.
fn passband_edge(params: &CqtParams, group: &Group) -> f64 {
    let top = group.first_bin + group.num_bins - 1;
    let rate = group.sample_rate(params.sample_rate());
    let bandwidth = rate / *group.lengths.last().unwrap() as f64 * HANN_BANDWIDTH;
    params.center_freq_f64(top) + 0.5 * bandwidth
}

pub(crate) fn plan(params: &CqtParams) -> Result<Plan, CqtParamsError> {
    let num_bins = params.num_bins();
    let bins_per_octave = params.bins_per_octave();
    let num_groups = if params.multirate() {
        num_bins.div_ceil(bins_per_octave)
    } else {
        1
    };

    // Octaves are cut from the top so that the highest group is always a
    // full octave: the top bin of the group `g` octaves below the top then
    // sits at `max_bin_freq / 2^g`, keeping every level as far below its own
    // Nyquist frequency as the full-rate top bin is below the signal's
    // Nyquist frequency. The lowest group may be partial.
    //
    // A group normally runs `g` decimation levels deep. When
    // `max_kernel_length` shortens its kernels so much that their pass-band
    // would reach the decimated Nyquist frequency, it runs at a shallower
    // level instead.
    let mut groups = Vec::with_capacity(num_groups);
    let mut num_levels = 1;
    for g in 0..num_groups {
        let octave = num_groups - 1 - g;
        let (first_bin, last_bin) = if num_groups == 1 {
            (0, num_bins)
        } else {
            let last_bin = num_bins - octave * bins_per_octave;
            (last_bin.saturating_sub(bins_per_octave), last_bin)
        };
        let mut chosen = None;
        for level in (0..=octave).rev() {
            let lengths: Vec<usize> = (first_bin..last_bin)
                .map(|bin| kernel_length_at(params, bin, level))
                .collect();
            let fft_length = lengths.iter().copied().max().unwrap().next_power_of_two();
            let group = Group {
                first_bin,
                num_bins: last_bin - first_bin,
                level,
                fft_length,
                lengths,
            };
            let nyquist = group.sample_rate(params.sample_rate()) / 2.0;
            if passband_edge(params, &group) <= nyquist {
                chosen = Some(group);
                break;
            }
        }
        let Some(group) = chosen else {
            return Err(CqtParamsError::AboveNyquist {
                nyquist: (f64::from(params.sample_rate()) / 2.0) as f32,
            });
        };
        num_levels = num_levels.max(group.level + 1);
        groups.push(group);
    }

    let filter = if num_levels > 1 {
        // The stage feeding level `d` must pass every group at level >= d.
        // Its pass-band edge relative to the input rate is `edge / rate_in`
        // and the transition band reaches to `1/2 - edge / rate_in`, so the
        // half-band transition width is `1 - 4 * edge / rate_in` of Nyquist.
        let mut transition = 1.0f64;
        for d in 1..num_levels {
            let rate_in = f64::from(params.sample_rate()) / (1u64 << (d - 1)) as f64;
            let edge = groups
                .iter()
                .filter(|group| group.level >= d)
                .map(|group| passband_edge(params, group))
                .fold(0.0, f64::max);
            transition = transition.min(1.0 - 4.0 * edge / rate_in);
        }
        // Reject before designing: a transition band near zero would
        // otherwise allocate and evaluate millions of taps just to fail.
        let taps = HalfBand::taps_for(transition, ATTENUATION_DB);
        if taps > MAX_FILTER_TAPS {
            return Err(CqtParamsError::DecimationFilterTooLong {
                taps,
                max: MAX_FILTER_TAPS,
            });
        }
        Some(HalfBand::design(transition, ATTENUATION_DB))
    } else {
        None
    };

    let mut plan = Plan {
        groups,
        num_levels,
        filter,
        latency: 0,
    };
    for group in &plan.groups {
        let stride = group.stride();
        let latency = group.fft_length / 2 * stride + plan.cascade_delay(group.level);
        plan.latency = plan.latency.max(latency);
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_cover_all_bins_top_down() {
        let params = CqtParams::new(44_100, 55.0, 7_040.0, 24).unwrap();
        let plan = plan(&params).unwrap();
        assert_eq!(plan.num_levels, 8);
        assert_eq!(plan.groups.len(), 8);
        let mut next = 0;
        for (g, group) in plan.groups.iter().enumerate() {
            assert_eq!(group.first_bin, next);
            assert_eq!(group.level, 7 - g);
            next += group.num_bins;
        }
        assert_eq!(next, params.num_bins());
        // 169 bins: the lowest group holds the single leftover bin.
        assert_eq!(plan.groups[0].num_bins, 1);
        assert_eq!(plan.groups[7].num_bins, 24);
        // Full octaves share the same FFT length; the partial one is smaller.
        for group in &plan.groups[1..] {
            assert_eq!(group.fft_length, plan.groups[7].fft_length);
        }
        assert!(plan.groups[0].fft_length <= plan.groups[1].fft_length);
        assert!(plan.filter.is_some());
        // Latency matches the single-rate frame half-length plus the
        // cascade delay.
        let delay = plan.filter.as_ref().unwrap().delay();
        assert_eq!(
            plan.latency,
            params.fft_length() / 2 + (delay - 1) * ((1 << 7) - 1)
        );
    }

    #[test]
    fn capped_kernels_use_shallower_levels() {
        let params = CqtParams::builder(22_000, 14.568, 7_902.1)
            .max_kernel_length(2_048)
            .build()
            .unwrap();
        let plan = plan(&params).unwrap();
        // Low octaves are capped and cannot go as deep as their index.
        assert!(plan.groups[0].level < plan.groups.len() - 1);
        assert!(plan.num_levels > 1);
        for pair in plan.groups.windows(2) {
            assert!(pair[0].level >= pair[1].level);
        }
    }

    #[test]
    fn oversized_filter_is_rejected_without_being_built() {
        // The top bin's pass-band nearly touches the Nyquist frequency, so
        // the half-band transition collapses to the designer's clamp and
        // the filter would need about ten million taps.
        let params = CqtParams::builder(16_000, 62.415_375, 7_989.168)
            .bins_per_octave(384)
            .build();
        let err = match params {
            Ok(params) => plan(&params).unwrap_err(),
            Err(err) => err,
        };
        assert!(
            matches!(err, CqtParamsError::DecimationFilterTooLong { taps, .. } if taps > MAX_FILTER_TAPS),
            "{err:?}"
        );
    }

    #[test]
    fn single_rate_has_one_group() {
        let params = CqtParams::builder(44_100, 55.0, 7_040.0)
            .multirate(false)
            .build()
            .unwrap();
        let plan = plan(&params).unwrap();
        assert_eq!(plan.groups.len(), 1);
        assert_eq!(plan.groups[0].level, 0);
        assert!(plan.filter.is_none());
        assert_eq!(plan.latency, params.fft_length() / 2);
    }
}
