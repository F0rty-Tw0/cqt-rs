# E014 results: longer retrieval and two-second continuation

Status: completed; default switch rejected. Controlled tracking improves, but mixed-audio and voiceover regressions remain.

See [provenance, integrity replays and reproduction](E014-reproduction.md).

All results use the frozen corrected catalogue and query labels. These are known recordings; synthetic speech is not a test of general human voiceover.

## Frozen ten-second queries

| Group | Legacy modal | Long context | Continuation |
| --- | ---: | ---: | ---: |
| exact | 22/22 | 22/22 | 22/22 |
| clean | 22/22 | 22/22 | 22/22 |
| frozen | 16/22 | 16/22 | 16/22 |
| mix | 17/22 | 16/22 | 16/22 |
| negative | 8/8 | 8/8 | 8/8 |

Negative cells count rejected queries. Other cells count the strongest accepted identity under the unchanged scoring rule. Correct identity does not guarantee position.

| Comparison / group | Gains | Lost identities | Wrong starts before / after | Duplicates before / after |
| --- | --- | --- | ---: | ---: |
| legacy → long / exact | — | — | 0 / 0 | 3 / 0 |
| legacy → continuation / exact | — | — | 0 / 0 | 3 / 0 |
| long → continuation / exact | — | — | 0 / 0 | 0 / 0 |
| legacy → long / clean | — | — | 0 / 0 | 0 / 0 |
| legacy → continuation / clean | — | — | 0 / 0 | 0 / 0 |
| long → continuation / clean | — | — | 0 / 0 | 0 / 0 |
| legacy → long / frozen | frozen-t21 | frozen-t13 | 1 / 1 | 1 / 1 |
| legacy → continuation / frozen | — | — | 1 / 1 | 1 / 0 |
| long → continuation / frozen | frozen-t13 | frozen-t21 | 1 / 1 | 1 / 0 |
| legacy → long / mix | — | mix-t21 | 1 / 1 | 1 / 0 |
| legacy → continuation / mix | — | mix-t21 | 1 / 1 | 1 / 0 |
| long → continuation / mix | — | — | 1 / 1 | 0 / 0 |
| legacy → long / negative | — | — | 0 / 0 | 0 / 0 |
| legacy → continuation / negative | — | — | 0 / 0 | 0 / 0 |
| long → continuation / negative | — | — | 0 / 0 | 0 / 0 |

## Continuous programmes and robustness

| Case / arm | Detected plays | Duplicates | False starts | Premature endings |
| --- | ---: | ---: | ---: | ---: |
| bass_cut12-continuation | 22/22 | 0 | 0 | 0 |
| bass_cut12-long | 22/22 | 1 | 0 | 1 |
| combined-continuation | 17/22 | 0 | 1 | 1 |
| combined-long | 19/22 | 0 | 1 | 1 |
| gain-continuation | 22/22 | 0 | 0 | 0 |
| gain-long | 22/22 | 1 | 0 | 1 |
| noise_0db-continuation | 8/22 | 0 | 1 | 0 |
| noise_0db-long | 6/22 | 0 | 2 | 0 |
| noise_10db-continuation | 22/22 | 0 | 0 | 0 |
| noise_10db-long | 22/22 | 0 | 0 | 0 |
| phase-continuation | 25/25 | 0 | 0 | 0 |
| phase-long | 25/25 | 1 | 0 | 1 |
| pitch_minus2-continuation | 22/22 | 0 | 0 | 0 |
| pitch_minus2-long | 22/22 | 0 | 0 | 0 |
| pitch_plus2-continuation | 22/22 | 0 | 0 | 0 |
| pitch_plus2-long | 22/22 | 1 | 0 | 1 |
| pitch_plus_half-continuation | 22/22 | 0 | 0 | 0 |
| pitch_plus_half-long | 22/22 | 0 | 1 | 0 |
| programme-continuation-block257 | 25/25 | 0 | 0 | 0 |
| programme-continuation-block65536 | 25/25 | 0 | 0 | 0 |
| programme-continuation | 25/25 | 0 | 0 | 0 |
| programme-legacy-baseline | 25/25 | 3 | 0 | 3 |
| programme-legacy | 25/25 | 3 | 0 | 3 |
| programme-long | 25/25 | 3 | 0 | 3 |
| programme-single | 25/25 | 0 | 0 | 0 |
| tempo_088-continuation | 22/22 | 0 | 0 | 0 |
| tempo_088-long | 22/22 | 0 | 0 | 0 |
| tempo_112-continuation | 22/22 | 0 | 0 | 0 |
| tempo_112-long | 22/22 | 1 | 0 | 1 |
| treble_cut12-continuation | 22/22 | 0 | 0 | 0 |
| treble_cut12-long | 22/22 | 1 | 0 | 1 |
| voice_0db-continuation | 16/22 | 0 | 1 | 0 |
| voice_0db-long | 17/22 | 0 | 1 | 0 |

All start events are assigned once. Misses remain in the denominator. Timing summaries are conditional on a detected play and remain separate from recall.

## Complete real DJ mix

| Arm | Songs detected | Segments | Frozen locations covered | Random locations covered |
| --- | ---: | ---: | ---: | ---: |
| long | 21/22 | 58 | 12/22 | 14/22 |
| continuation | 21/22 | 109 | 9/22 | 12/22 |
| single | 21/22 | 90 | 9/22 | 12/22 |

| Song | Long segments | Continuation segments | Single-hypothesis segments |
| --- | ---: | ---: | ---: |
| t01 | 2 | 2 | 3 |
| t02 | 5 | 8 | 9 |
| t03 | 4 | 13 | 11 |
| t04 | 6 | 2 | 2 |
| t05 | 3 | 4 | 3 |
| t06 | 1 | 9 | 8 |
| t07 | 1 | 6 | 4 |
| t08 | 1 | 8 | 7 |
| t09 | 3 | 4 | 4 |
| t10 | 0 | 0 | 0 |
| t11 | 1 | 2 | 2 |
| t12 | 2 | 13 | 7 |
| t13 | 3 | 5 | 4 |
| t14 | 1 | 1 | 1 |
| t15 | 6 | 11 | 6 |
| t16 | 1 | 1 | 1 |
| t17 | 2 | 3 | 3 |
| t18 | 1 | 4 | 3 |
| t19 | 2 | 2 | 2 |
| t20 | 1 | 6 | 6 |
| t21 | 8 | 3 | 3 |
| t22 | 4 | 2 | 1 |

## Timing and source-position limits

| Programme / arm | Start notification median / p95 (s) | End notification median / p95 (s) | Supported start error min / max (s) | Supported end error min / max (s) |
| --- | ---: | ---: | ---: | ---: |
| phase-continuation | 5.430 / 5.468 | 7.428 / 7.466 | -1.003 / 1.001 | -0.999 / 1.003 |
| phase-long | 1.993 / 4.187 | 13.968 / 15.728 | — | — |
| programme-continuation-block257 | 6.389 / 6.393 | 6.388 / 6.392 | -0.003 / 0.002 | -2.002 / 0.003 |
| programme-continuation-block65536 | 7.152 / 7.732 | 7.185 / 7.780 | -0.003 / 0.002 | -2.002 / 0.003 |
| programme-continuation | 6.429 / 6.467 | 6.422 / 6.460 | -0.003 / 0.002 | -2.002 / 0.003 |
| programme-legacy-baseline | 1.672 / 2.490 | 9.618 / 10.049 | — | — |
| programme-legacy | 1.672 / 2.490 | 9.618 / 10.049 | — | — |
| programme-long | 2.844 / 3.167 | 14.782 / 15.114 | — | — |
| programme-single | 6.430 / 6.468 | 6.422 / 6.460 | -0.003 / 2.001 | -2.002 / 0.003 |

| Clean source positions / arm | Correct starts evaluated | Within 2 s | Median absolute error (s) | Worst absolute error (s) |
| --- | ---: | ---: | ---: | ---: |
| exact / long | 22 | 21 | 0.000 | 187.740 |
| exact / continuation | 22 | 21 | 0.001 | 187.742 |
| clean / long | 22 | 21 | 0.002 | 14.550 |
| clean / continuation | 22 | 21 | 0.001 | 14.545 |

Repeated source passages can produce the right identity at another location. Supported interval edges are coarse evidence estimates, not audible ground truth.

## Robustness misses and paired regressions

| Treatment | Long misses | Continuation misses | Lost previously detected plays |
| --- | --- | --- | --- |
| gain | — | — | — |
| pitch_minus2 | — | — | — |
| pitch_plus2 | — | — | — |
| pitch_plus_half | — | — | — |
| tempo_088 | — | — | — |
| tempo_112 | — | — | — |
| bass_cut12 | — | — | — |
| treble_cut12 | — | — | — |
| noise_10db | — | — | — |
| noise_0db | clean-t01, clean-t02, clean-t03, clean-t05, clean-t06, clean-t09, clean-t10, clean-t11, clean-t12, clean-t13, clean-t15, clean-t16, clean-t17, clean-t18, clean-t20, clean-t21 | clean-t01, clean-t03, clean-t05, clean-t09, clean-t10, clean-t11, clean-t12, clean-t13, clean-t15, clean-t16, clean-t17, clean-t18, clean-t20, clean-t21 | — |
| voice_0db | clean-t01, clean-t09, clean-t15, clean-t20, clean-t22 | clean-t01, clean-t03, clean-t09, clean-t15, clean-t20, clean-t22 | clean-t03 |
| combined | clean-t01, clean-t15, clean-t18 | clean-t01, clean-t04, clean-t16, clean-t18, clean-t19 | clean-t04, clean-t16, clean-t19 |

Coverage means a logical active interval overlaps the frozen query location; it is not an audible-boundary annotation. Every segment is retained in the audit.

## Every frozen and random mix-query outcome

| Query | Legacy | Long context | Continuation |
| --- | --- | --- | --- |
| frozen-t01 | wrong-match | wrong-match | wrong-match |
| mix-t01 | wrong-match | wrong-match | wrong-match |
| frozen-t02 | correct | correct | correct |
| mix-t02 | correct | correct | correct |
| frozen-t03 | correct | correct | correct |
| mix-t03 | correct | correct | correct |
| frozen-t04 | no-match | no-match | no-match |
| mix-t04 | no-match | no-match | no-match |
| frozen-t05 | correct | correct | correct |
| mix-t05 | correct | correct | correct |
| frozen-t06 | correct | correct | correct |
| mix-t06 | correct | correct | correct |
| frozen-t07 | correct | correct | correct |
| mix-t07 | correct | correct | correct |
| frozen-t08 | correct | correct | correct |
| mix-t08 | correct | correct | correct |
| frozen-t09 | correct | correct | correct |
| mix-t09 | correct | correct | correct |
| frozen-t10 | no-match | no-match | no-match |
| mix-t10 | no-match | no-match | no-match |
| frozen-t11 | correct | correct | correct |
| mix-t11 | correct | correct | correct |
| frozen-t12 | correct | correct | correct |
| mix-t12 | correct | correct | correct |
| frozen-t13 | correct | no-match | correct |
| mix-t13 | no-match | no-match | no-match |
| frozen-t14 | correct | correct | correct |
| mix-t14 | correct | correct | correct |
| frozen-t15 | no-match | no-match | no-match |
| mix-t15 | correct | correct | correct |
| frozen-t16 | correct | correct | correct |
| mix-t16 | correct | correct | correct |
| frozen-t17 | correct | correct | correct |
| mix-t17 | correct | correct | correct |
| frozen-t18 | correct | correct | correct |
| mix-t18 | correct | correct | correct |
| frozen-t19 | correct | correct | correct |
| mix-t19 | correct | correct | correct |
| frozen-t20 | correct | correct | correct |
| mix-t20 | correct | correct | correct |
| frozen-t21 | no-match | correct | no-match |
| mix-t21 | correct | no-match | no-match |
| frozen-t22 | no-match | no-match | no-match |
| mix-t22 | no-match | no-match | no-match |

## Previously weak mix passages

| Query | Continuation outcome | Maximum retrieval confidence | Best fresh alignment when confidence ≥70 |
| --- | --- | ---: | ---: |
| frozen-t04 | no-match | 71.631 | 0.182 |
| mix-t04 | no-match | 33.333 | — |
| frozen-t10 | no-match | 51.807 | — |
| mix-t10 | no-match | 44.444 | — |
| frozen-t13 | correct | 82.533 | 0.472 |
| mix-t13 | no-match | 51.220 | — |
| frozen-t22 | no-match | 61.538 | — |
| mix-t22 | no-match | 52.381 | — |

The start gates remain confidence ≥70 and alignment ≥0.4 on successive fresh observations. Longer windows cannot establish missing fingerprint correspondence by themselves. These diagnostics do not isolate which DJ effect caused a miss.


## Decision

Reject a default switch; retain the prototype as opt-in.

100% matching under the requested range of mix transformations and voiceover remains unproved. Confidence is a decision score, not a probability. No performance, exact-boundary or production false-alarm claim.
