# E015: evidence survival at the known correct trajectory

**Diagnostic verified; matcher unchanged.** The next candidate experiment is a bounded pair-assisted retrieval fallback for sparse surviving music peaks. Voiceover also needs a separate verification experiment.

The unchanged E014 native executable exported 27 files: 22 references and five programmes, covering every one of 110 controlled plays. All 5,693,564 native triplets were reproduced independently from the exported peaks. The diagnostic examines 452 interior two-second intervals. Correct trajectories are supplied offline; these are not additional recognized plays.

| Treatment | E014 detected | Plays with two oracle checks | Reference peak recovery | Matched-query fraction | Reference triplet recovery |
| --- | ---: | ---: | ---: | ---: | ---: |
| gain | 22/22 | 22/22 | 84.2% | 83.9% | 52.9% |
| noise_10db | 22/22 | 22/22 | 66.8% | 81.9% | 28.1% |
| noise_0db | 8/22 | 22/22 | 31.8% | 84.8% | 3.3% |
| voice_0db | 16/22 | 16/22 | 51.4% | 45.1% | 8.2% |
| combined | 17/22 | 18/22 | 52.0% | 43.7% | 6.9% |

Peak fractions pool counts within the native verifier’s query-defined reference spans. Triplet recovery uses known source geometry and the frozen key/anchor/span tolerances before the index cap. These denominators differ; neither metric is an independent vote count or recognition probability. Even the gain control has incomplete recovery which can reflect excerpt/programme analysis-grid and feature-context changes or PCM quantization; these causes are not isolated here. Relative treatment comparisons use that control.

## What failed

**Heavy noise: retrieval is the immediate bottleneck.** All 14 missed plays have at least two consecutive oracle start checks, while their correct-song retrieval confidence never reaches 70 in the measured interior windows. Peak recovery falls from 84.2% in gain controls to 31.8%, but triplet recovery falls further, from 52.9% to 3.3%. Surviving query peaks are mostly consistent with the song (84.8% matched), so lowering the verification threshold does not address these misses. There may be very little evidence: one interior observation has only one matching peak. Oracle passage does not justify accepting such a case in production.

**Voiceover: evidence is both lost and diluted.** All six missed plays lack two consecutive oracle start checks. Matched-query fraction falls to 45.1%, while reference peak recovery is 51.4%. Some windows retain music correspondence but added speech peaks reduce the fraction; others lose music peaks too. Four of the six misses have at least two consecutive oracle hold checks at 0.3, but not two start checks at 0.4. This supports testing a verifier that accounts for interference after retrieval is fixed; it does not support merely lowering the threshold.

**Combined effects: more than one mechanism fails.** Three misses lack two consecutive oracle start checks. For t04 the correct trajectory has two consecutive passing oracle checks and a retrieved candidate passes, but continuation/start timing still fails. For t16 passing retrieved candidates use a source position incompatible with the supplied trajectory. One detected combined play lacks two interior oracle start checks; the diagnostic excludes boundary windows and recognition can choose another repeated position, so oracle and recognition totals are not interchangeable.

## Every missed play

| Treatment | Song | Attribution order | Oracle start / hold streak | Max retrieval confidence | Reference triplet recovery |
| --- | --- | --- | ---: | ---: | ---: |
| noise_0db | t01 | retrieval_confidence_below_gate | 4 / 4 | 2.44 | 0.00% |
| noise_0db | t03 | retrieval_confidence_below_gate | 4 / 4 | 59.60 | 0.90% |
| noise_0db | t05 | retrieval_confidence_below_gate | 4 / 4 | 66.94 | 2.38% |
| noise_0db | t09 | retrieval_confidence_below_gate | 4 / 4 | 43.66 | 0.59% |
| noise_0db | t10 | retrieval_confidence_below_gate | 4 / 4 | 33.33 | 0.58% |
| noise_0db | t11 | retrieval_confidence_below_gate | 4 / 4 | 33.33 | 0.66% |
| noise_0db | t12 | retrieval_confidence_below_gate | 4 / 4 | 36.51 | 0.41% |
| noise_0db | t13 | retrieval_confidence_below_gate | 4 / 4 | 65.22 | 3.04% |
| noise_0db | t15 | retrieval_confidence_below_gate | 4 / 4 | 36.51 | 1.14% |
| noise_0db | t16 | retrieval_confidence_below_gate | 4 / 4 | 68.75 | 2.26% |
| noise_0db | t17 | retrieval_confidence_below_gate | 4 / 4 | 64.60 | 1.40% |
| noise_0db | t18 | retrieval_confidence_below_gate | 4 / 4 | 4.76 | 0.05% |
| noise_0db | t20 | retrieval_confidence_below_gate | 4 / 4 | 40.30 | 0.88% |
| noise_0db | t21 | retrieval_confidence_below_gate | 4 / 4 | 44.44 | 0.84% |
| voice_0db | t01 | oracle_start_support_insufficient | 0 / 0 | 66.39 | 0.51% |
| voice_0db | t03 | oracle_start_support_insufficient | 1 / 4 | 80.49 | 3.11% |
| voice_0db | t09 | oracle_start_support_insufficient | 0 / 4 | 70.15 | 2.73% |
| voice_0db | t15 | oracle_start_support_insufficient | 0 / 0 | 55.56 | 0.99% |
| voice_0db | t20 | oracle_start_support_insufficient | 0 / 3 | 67.48 | 2.47% |
| voice_0db | t22 | oracle_start_support_insufficient | 1 / 2 | 79.06 | 3.74% |
| combined | t01 | oracle_start_support_insufficient | 1 / 1 | 68.25 | 4.56% |
| combined | t04 | continuation_or_start_timing | 2 / 3 | 79.27 | 4.93% |
| combined | t16 | retrieved_wrong_trajectory | 4 / 5 | 81.04 | 4.24% |
| combined | t18 | oracle_start_support_insufficient | 1 / 3 | 81.57 | 3.52% |
| combined | t19 | oracle_start_support_insufficient | 1 / 5 | 77.78 | 4.78% |

Attributions follow a fixed order: oracle start support, retrieval confidence, retrieved start gate, true-position compatibility, then continuation/timing. They locate an immediate blocking condition; they do not prove other stages are healthy. All successes and their errors are retained in the per-play data.

## Index cap and validation

| Treatment | Geometrically supported query hashes | After native per-song/key cap of 8 |
| --- | ---: | ---: |
| gain | 65449 | 61079 |
| noise_10db | 34751 | 31558 |
| noise_0db | 4034 | 3416 |
| voice_0db | 10086 | 9159 |
| combined | 8535 | 7618 |

The cap removes additional support but is not the principal heavy-noise collapse: only 4,034 geometrically supported query hashes remain before the cap, compared with 65,449 in gain controls. Increasing the cap alone cannot restore missing triplets.

All reference index counts and all five programme peak/hash counts agree with E014. Of 1,917 recorded native verification observations, 1,885 recount exactly at serialized parameters. The remaining 32 differ by at most two counts; all native values lie inside the uncertainty envelope implied by six-decimal source positions and eight-decimal tempo. The nominal differences and bounds remain in the audit. No matcher tolerance was changed. Synthetic mapping, peak insertion/deletion, index-cap, streak and independent brute-force tests validate the diagnostic.

## Next bounded experiment

Freeze E016 before implementation: add a bounded pair-assisted candidate retrieval path using the same 10-second context and two-second checks, preserving pitch/tempo estimation. Measure whether it recovers the 14 heavy-noise misses, at matched false-start policy and unchanged clean/pitch/BPM/EQ regression outcomes. Use support from distinct anchors over time; a high fraction from one peak is insufficient. Keep candidate and memory budgets explicit. Pair survival and extra collisions must be measured; improvement is a hypothesis.

Keep the verifier unchanged in that experiment so the retrieval effect is identifiable. Voiceover start failures are expected to remain possible. A later independent verifier experiment should compare matched support against chance/background density, calibrated on separate negatives, rather than treating every unrelated speech peak as contradictory evidence. Real human speech and recordings excluded from development are still required for a general accuracy claim.

## Provenance and reproduction

Protocol: [E015](E015-survival-protocol.md), published at `b3c6751e545ed600915f3a77023bc40405e4e0f9`. Evaluated executable source remains `748dcd0e4ed4d1431a39a6878e99f1ee7075ccae`, SHA-256 `1b1c5968ce134bda1b559da9b8bdade829c30070c4960700da6015988c0ed9d7`. No native code or matching thresholds changed. [Exact-source CI](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784380994) passed; final-head checks are tracked separately on PR #4.

The original protocol incorrectly listed the index cap as 64; source inspection corrected it to 8 before extraction. The only recount addition after the initial results was an uncertainty envelope for already-rounded native log parameters; all nominal count differences remain visible. No audio labels, transforms or native outputs were altered.

Recover E014’s exact inputs using [its reproduction instructions](E014-reproduction.md), then run:

```sh
for i in $(seq -w 1 22); do python3 scripts/survival_extract.py reference-t$i; done
for case in gain noise_10db noise_0db voice_0db combined; do python3 scripts/survival_extract.py "$case"; done
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/survival_diagnostic.py
python3 scripts/survival_report.py
```

The extractor verifies input/binary identities and writes completed files atomically. Strict signature/hash checks protect reuse. [Evidence](../evidence/E015/README.md) includes every play/window, the compressed complete audit, extraction provenance and raw archive identity. This known-recording/synthetic-speech diagnostic does not validate arbitrary pitch/BPM/EQ/noise ranges, real DJ source trajectories, exact boundaries, speed or universal 100% recognition. E014’s default-switch rejection remains in force. No merge or release.
