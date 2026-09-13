# E012 results: modal fitting with complete-song references

The only candidate change is the existing `--modal-fit` option. All 96 candidate runs use the same executable, complete-song database, query bytes and thresholds as their E011 full-song baselines.

| Query set | Queries | Independent chunks | Complete songs, default | Complete songs, modal |
| --- | ---: | ---: | ---: | ---: |
| exact | 22 | 22 correct; 0 wrong; 0 no match | 20 correct; 0 wrong; 2 no match | 22 correct; 0 wrong; 0 no match |
| clean | 22 | 22 correct; 0 wrong; 0 no match | 22 correct; 0 wrong; 0 no match | 22 correct; 0 wrong; 0 no match |
| frozen | 22 | 14 correct; 0 wrong; 8 no match | 15 correct; 0 wrong; 7 no match | 16 correct; 1 wrong; 5 no match |
| mix | 22 | 15 correct; 1 wrong; 6 no match | 16 correct; 1 wrong; 5 no match | 17 correct; 1 wrong; 4 no match |
| negative | 8 | 8 rejected; 0 false | 8 rejected; 0 false | 8 rejected; 0 false |

**Predeclared exploratory acceptance:** fails.

| Gate | Result |
| --- | --- |
| triggering_misses_recovered | pass |
| clean_targets_met | pass |
| more_mix_correct | pass |
| no_lost_correct | pass |
| no_added_wrong_starts | fail |

## Paired gains and losses

- exact: gains exact-t18, exact-t22; losses none.
- clean: gains none; losses none.
- frozen: gains frozen-t18; losses none.
- mix: gains mix-t03; losses none.
- negative: gains none; losses none.

Descriptive paired track bootstrap: 10,000 resamples, seed 20260913; conditional on this known mix and its algorithmic labels. Annotation error and generalization are not covered.

| Set | Modal minus default, percentage points | Descriptive 95% interval |
| --- | ---: | ---: |
| exact | +9.1 | [+0.0, +22.7] |
| clean | +0.0 | [+0.0, +0.0] |
| frozen | +4.5 | [+0.0, +13.6] |
| mix | +4.5 | [+0.0, +13.6] |

## Scope

These are development results on one known mix. The random mix timestamps were frozen before E011, but E012 was motivated by E011’s exposed clean-control failures. This is not a held-out final evaluation. The 22 mix labels have independent algorithmic alignment support, not human cue verification. The corrected t05 catalogue and all sampling restrictions are documented in E011.

The first random mix query has possible overlapping-track/cue ambiguity: independent audio alignment supports both t01 and t02 near its timestamp. Its frozen expected label t01 and resulting scores remain unchanged; see E011 for the post-outcome diagnostic.

Modal fitting changes point estimates, keeping the winning neighbourhood’s evidence and the confidence/alignment gates. Recovery would support an alignment-estimation explanation for affected cases; it would not prove every default miss has the same cause or that reported positions are always the correct occurrence of repeated music. The option remains opt-in and no default/native implementation is changed.

Eight known negative clips provide only 80 seconds of nonmatching music per configuration. Every wrong-parent accepted start is retained below, including those accompanying a correct top prediction. No production false-alarm-rate or speed claim follows.

## Accepted starts

| Set | Default starts / wrong / repeated | Modal starts / wrong / repeated |
| --- | ---: | ---: |
| exact | 21 / 0 / 1 | 25 / 0 / 3 |
| clean | 22 / 0 / 0 | 22 / 0 / 0 |
| frozen | 16 / 0 / 1 | 18 / 1 / 1 |
| mix | 17 / 1 / 0 | 19 / 1 / 1 |
| negative | 0 / 0 / 0 | 0 / 0 / 0 |

## Every paired query

| Query | Default | Modal |
| --- | --- | --- |
| exact-t01 | correct (t01) | correct (t01) |
| exact-t02 | correct (t02) | correct (t02) |
| exact-t03 | correct (t03) | correct (t03) |
| exact-t04 | correct (t04) | correct (t04) |
| exact-t05 | correct (t05) | correct (t05) |
| exact-t06 | correct (t06) | correct (t06) |
| exact-t07 | correct (t07) | correct (t07) |
| exact-t08 | correct (t08) | correct (t08) |
| exact-t09 | correct (t09) | correct (t09) |
| exact-t10 | correct (t10) | correct (t10) |
| exact-t11 | correct (t11) | correct (t11) |
| exact-t12 | correct (t12) | correct (t12) |
| exact-t13 | correct (t13) | correct (t13) |
| exact-t14 | correct (t14) | correct (t14) |
| exact-t15 | correct (t15) | correct (t15) |
| exact-t16 | correct (t16) | correct (t16) |
| exact-t17 | correct (t17) | correct (t17) |
| exact-t18 | no-match | correct (t18) |
| exact-t19 | correct (t19) | correct (t19) |
| exact-t20 | correct (t20) | correct (t20) |
| exact-t21 | correct (t21) | correct (t21) |
| exact-t22 | no-match | correct (t22) |
| clean-t01 | correct (t01) | correct (t01) |
| clean-t02 | correct (t02) | correct (t02) |
| clean-t03 | correct (t03) | correct (t03) |
| clean-t04 | correct (t04) | correct (t04) |
| clean-t05 | correct (t05) | correct (t05) |
| clean-t06 | correct (t06) | correct (t06) |
| clean-t07 | correct (t07) | correct (t07) |
| clean-t08 | correct (t08) | correct (t08) |
| clean-t09 | correct (t09) | correct (t09) |
| clean-t10 | correct (t10) | correct (t10) |
| clean-t11 | correct (t11) | correct (t11) |
| clean-t12 | correct (t12) | correct (t12) |
| clean-t13 | correct (t13) | correct (t13) |
| clean-t14 | correct (t14) | correct (t14) |
| clean-t15 | correct (t15) | correct (t15) |
| clean-t16 | correct (t16) | correct (t16) |
| clean-t17 | correct (t17) | correct (t17) |
| clean-t18 | correct (t18) | correct (t18) |
| clean-t19 | correct (t19) | correct (t19) |
| clean-t20 | correct (t20) | correct (t20) |
| clean-t21 | correct (t21) | correct (t21) |
| clean-t22 | correct (t22) | correct (t22) |
| frozen-t01 | no-match | wrong-match (t02) |
| frozen-t02 | correct (t02) | correct (t02) |
| frozen-t03 | correct (t03) | correct (t03) |
| frozen-t04 | no-match | no-match |
| frozen-t05 | correct (t05) | correct (t05) |
| frozen-t06 | correct (t06) | correct (t06) |
| frozen-t07 | correct (t07) | correct (t07) |
| frozen-t08 | correct (t08) | correct (t08) |
| frozen-t09 | correct (t09) | correct (t09) |
| frozen-t10 | no-match | no-match |
| frozen-t11 | correct (t11) | correct (t11) |
| frozen-t12 | correct (t12) | correct (t12) |
| frozen-t13 | correct (t13) | correct (t13) |
| frozen-t14 | correct (t14) | correct (t14) |
| frozen-t15 | no-match | no-match |
| frozen-t16 | correct (t16) | correct (t16) |
| frozen-t17 | correct (t17) | correct (t17) |
| frozen-t18 | no-match | correct (t18) |
| frozen-t19 | correct (t19) | correct (t19) |
| frozen-t20 | correct (t20) | correct (t20) |
| frozen-t21 | no-match | no-match |
| frozen-t22 | no-match | no-match |
| mix-t01 | wrong-match (t02) | wrong-match (t02) |
| mix-t02 | correct (t02) | correct (t02) |
| mix-t03 | no-match | correct (t03) |
| mix-t04 | no-match | no-match |
| mix-t05 | correct (t05) | correct (t05) |
| mix-t06 | correct (t06) | correct (t06) |
| mix-t07 | correct (t07) | correct (t07) |
| mix-t08 | correct (t08) | correct (t08) |
| mix-t09 | correct (t09) | correct (t09) |
| mix-t10 | no-match | no-match |
| mix-t11 | correct (t11) | correct (t11) |
| mix-t12 | correct (t12) | correct (t12) |
| mix-t13 | no-match | no-match |
| mix-t14 | correct (t14) | correct (t14) |
| mix-t15 | correct (t15) | correct (t15) |
| mix-t16 | correct (t16) | correct (t16) |
| mix-t17 | correct (t17) | correct (t17) |
| mix-t18 | correct (t18) | correct (t18) |
| mix-t19 | correct (t19) | correct (t19) |
| mix-t20 | correct (t20) | correct (t20) |
| mix-t21 | correct (t21) | correct (t21) |
| mix-t22 | no-match | no-match |
| n01 | correct-rejection | correct-rejection |
| n02 | correct-rejection | correct-rejection |
| n03 | correct-rejection | correct-rejection |
| n04 | correct-rejection | correct-rejection |
| n05 | correct-rejection | correct-rejection |
| n06 | correct-rejection | correct-rejection |
| n07 | correct-rejection | correct-rejection |
| n08 | correct-rejection | correct-rejection |

See [E012 protocol](E012-full-modal.md), [E011 results](E011-results.md) and [preserved raw evidence](../evidence/E011/README.md).

## Matched-source diagnostic

All 4/4 approximately corresponding clean-source clips are recognized; their random mix queries remain no-match. This supports investigating fingerprint survival in mixed audio, without isolating a particular DJ effect. These adaptive controls are separate from the main matrix. See [positions, method and evidence](E012-matched-source.md).
