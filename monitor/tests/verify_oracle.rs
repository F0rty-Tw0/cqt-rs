//! Exhaustive searches provide an independent oracle for optimized verification.

use cqt_monitor::{Peak, PeakTrack, Verification};

fn oracle(
    reference: &[Peak],
    query: &[Peak],
    shift: i32,
    tempo: f64,
    offset: f64,
    ft: u32,
    bt: u32,
) -> Verification {
    if query.is_empty() || tempo <= 0.0 || !tempo.is_finite() || !offset.is_finite() {
        return Verification::default();
    }
    let mut reference = reference.to_vec();
    reference.sort_unstable();
    reference.dedup();
    let close = |peaks: &[Peak], frame: f64, bin: i64| {
        peaks.iter().any(|p| {
            p.frame as f64 >= frame - f64::from(ft)
                && p.frame as f64 <= frame + f64::from(ft)
                && (i64::from(p.bin) - bin).unsigned_abs() <= u64::from(bt)
        })
    };
    let mut result = Verification {
        query_peaks: query.len() as u32,
        ..Verification::default()
    };
    for q in query {
        if close(
            &reference,
            tempo * q.frame as f64 + offset,
            i64::from(q.bin) - i64::from(shift),
        ) {
            result.query_matched += 1;
        }
    }
    let lo = tempo * query[0].frame as f64 + offset - f64::from(ft);
    let hi = tempo * query[query.len() - 1].frame as f64 + offset + f64::from(ft);
    for r in &reference {
        if r.frame as f64 >= lo && r.frame as f64 <= hi {
            result.reference_peaks += 1;
            if close(
                query,
                (r.frame as f64 - offset) / tempo,
                i64::from(r.bin) + i64::from(shift),
            ) {
                result.reference_matched += 1;
            }
        }
    }
    result
}

#[test]
fn seeded_grid_matches_exhaustive_oracle() {
    let mut comparisons = 0;
    for seed in 1..=4u64 {
        let mut state = seed;
        let mut reference = Vec::new();
        for _ in 0..128 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            reference.push(Peak {
                frame: state % 160,
                bin: ((state >> 8) % 32) as u32,
            });
        }
        // Constructor must still sort and deduplicate the reference.
        reference.extend_from_within(..8);
        let track = PeakTrack::new(reference.clone());
        let mut query: Vec<Peak> = reference[..24]
            .iter()
            .map(|p| Peak {
                frame: p.frame / 4,
                bin: p.bin + 3,
            })
            .collect();
        query.sort_unstable();
        for tempo in [0.88, 1.0, 1.12, 2.0] {
            for offset in [-200.0, -0.5, 0.0, 47.25, 200.0] {
                for shift in [i32::MIN, -3, 0, 3, i32::MAX] {
                    for ft in [0, 1, 5] {
                        for bt in [0, 2] {
                            let expected = oracle(&reference, &query, shift, tempo, offset, ft, bt);
                            let actual = track.verify(&query, shift, tempo, offset, ft, bt);
                            assert_eq!(
                                actual, expected,
                                "seed={seed} tempo={tempo} offset={offset} shift={shift} ft={ft} bt={bt}"
                            );
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 2400);
}

#[test]
fn empty_boundaries_and_extreme_frames_match_oracle() {
    let reference = vec![
        Peak { frame: 0, bin: 0 },
        Peak { frame: 8, bin: 8 },
        Peak { frame: 12, bin: 12 },
        Peak {
            frame: 1u64 << 53,
            bin: u32::MAX,
        },
        Peak {
            frame: u64::MAX,
            bin: 10,
        },
    ];
    for raw in [&[][..], &reference[..]] {
        let track = PeakTrack::new(raw.to_vec());
        for query in [vec![], vec![Peak { frame: 10, bin: 10 }], reference.clone()] {
            for tempo in [
                0.0,
                -1.0,
                f64::NAN,
                f64::INFINITY,
                f64::MIN_POSITIVE,
                1.0,
                f64::MAX,
            ] {
                for offset in [f64::NAN, -f64::MAX, -10.0, 0.0, f64::MAX] {
                    for ft in [0, 2, u32::MAX] {
                        assert_eq!(
                            track.verify(&query, 0, tempo, offset, ft, 2),
                            oracle(raw, &query, 0, tempo, offset, ft, 2)
                        );
                    }
                }
            }
        }
    }
    // Both endpoints are inclusive; a too-small reference slice loses these.
    let track = PeakTrack::new(reference);
    let result = track.verify(&[Peak { frame: 10, bin: 10 }], 0, 1.0, 0.0, 2, 2);
    assert_eq!(
        (
            result.query_matched,
            result.reference_peaks,
            result.reference_matched
        ),
        (1, 2, 2)
    );
}
