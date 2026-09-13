# E011 results: independent chunks versus complete songs

Both layouts use the corrected 22-song catalogue, the same pinned executable and unchanged default gates. Every query is ten seconds in a fresh process. All 192 native runs completed.

| Query set | Queries | Chunks: correct / wrong / no match | Full songs: correct / wrong / no match |
| --- | ---: | ---: | ---: |
| exact | 22 | 22 / 0 / 0 | 20 / 0 / 2 |
| clean | 22 | 22 / 0 / 0 | 22 / 0 / 0 |
| frozen | 22 | 14 / 0 / 8 | 15 / 0 / 7 |
| mix | 22 | 15 / 1 / 6 | 16 / 1 / 5 |
| negative | 8 | 8 rejected / 0 falsely accepted | 8 rejected / 0 falsely accepted |

**Exploratory preference rule:** fails. Requires more correct supported mix queries, no lost correct query, and no added wrong-parent starts in any set.

## Source correction and interpretation

E010’s t05 download was mislabeled: the linked file is Exodus (original mix) by Marc Burt. The corrected Roots and Shoots by Dave Kent comes from the archive embedded on the publisher’s release page. Five consistent independent STFT anchors support its occurrence around 791–852 seconds. All 22 mix labels now have algorithmic support; these are not human-verified annotations.

Correcting t05 changes the catalogue from E010. The paired E011 layouts use identical corrected recordings; do not ascribe a difference from the historical E010 score solely to reference layout. The complete-song treatment changes evidence grouping, boundary context and repeated-hash cap scope together. It does not independently prove which mechanism causes a particular miss.

Random-clean cuts are uniform across legal source sample positions. Random-mix cuts are uniform across legal start samples inside supported anchor intervals, not across entire publisher track intervals. The frozen set preserves E010’s actual query bytes, including its interpolated t01 cue. Exact clips are the middle complete chunk of each source. No query was discarded for being difficult.

The random t01 query at 59.594195 seconds is scored against its frozen expected identity t01. A post-outcome STFT diagnostic finds consistent t01 and t02 correspondences near this query: t01 anchors include [52,72], while adjacent t02 anchors cover [51,69] and [60,80] seconds. This is compatible with overlapping tracks or cue ambiguity. The wrong top identity under the frozen single-label protocol must not be interpreted as a proven unrelated-song false alarm. The query and its scored failure remain visible; no post-hoc relabeling or exclusion is applied.

Negative exposure is eight known nonmatching music clips, 80 seconds per layout. This is a small development control, not a production false-alarm-rate estimate. The known mix, originals and related queries are not a recording-disjoint final evaluation.

## Paired changes

- exact: gains none; losses exact-t18, exact-t22.
- clean: gains none; losses none.
- frozen: gains frozen-t13; losses none.
- mix: gains mix-t14; losses none.
- negative: gains none; losses none.

Descriptive paired track-bootstrap intervals condition on the 22 tracks and algorithmic labels in this one known mix (10,000 resamples, seed 20260913). They do not cover annotation error or generalization.

| Set | Full minus chunks, percentage points | Descriptive 95% interval |
| --- | ---: | ---: |
| exact | -9.1 | [-22.7, +0.0] |
| clean | +0.0 | [+0.0, +0.0] |
| frozen | +4.5 | [+0.0, +13.6] |
| mix | +4.5 | [+0.0, +13.6] |

## Index and accepted starts

| Layout | Records | Stored hashes | Dropped occurrences | Approximate index bytes |
| --- | ---: | ---: | ---: | ---: |
| chunks | 864 | 4,906,390 | 3,815 | 117,596,936 |
| full | 22 | 4,721,918 | 381,286 | 115,383,272 |

Index bytes are the native index estimate, not peak process memory. Concurrent run times are not speed evidence.

| Set | Chunk starts / wrong / repeated | Full starts / wrong / repeated |
| --- | ---: | ---: |
| exact | 81 / 0 / 59 | 21 / 0 / 1 |
| clean | 135 / 0 / 113 | 22 / 0 / 0 |
| frozen | 36 / 0 / 22 | 16 / 0 / 1 |
| mix | 47 / 1 / 31 | 17 / 1 / 0 |
| negative | 0 / 0 / 0 | 0 / 0 / 0 |

## Every query

| Query | Source or mix start | Chunks | Full songs |
| --- | ---: | --- | --- |
| exact-t01: Sleep Tight (Sergio's Last Remix) | 190.000000s | correct (t01) | correct (t01) |
| exact-t02: Proven Reality | 210.000000s | correct (t02) | correct (t02) |
| exact-t03: Turbulence (2020 Remix) | 210.000000s | correct (t03) | correct (t03) |
| exact-t04: Beautiful Geometry | 230.000000s | correct (t04) | correct (t04) |
| exact-t05: Roots and Shoots | 170.000000s | correct (t05) | correct (t05) |
| exact-t06: Scratching The Surface (Phish Funk Disco Mix) | 120.000000s | correct (t06) | correct (t06) |
| exact-t07: Chasing Rainbows (Marc Burt and Notch remix) | 210.000000s | correct (t07) | correct (t07) |
| exact-t08: Flaw 24 (extended mix) | 170.000000s | correct (t08) | correct (t08) |
| exact-t09: Turn The Corner (Notch remix) | 170.000000s | correct (t09) | correct (t09) |
| exact-t10: Pioneers | 160.000000s | correct (t10) | correct (t10) |
| exact-t11: Tears In My Heart (Notch remix) | 210.000000s | correct (t11) | correct (t11) |
| exact-t12: Beast | 190.000000s | correct (t12) | correct (t12) |
| exact-t13: The Piano Tune | 210.000000s | correct (t13) | correct (t13) |
| exact-t14: Baby Crying | 270.000000s | correct (t14) | correct (t14) |
| exact-t15: Electronaut (edit) | 180.000000s | correct (t15) | correct (t15) |
| exact-t16: Dancefloor Virus 2007 | 210.000000s | correct (t16) | correct (t16) |
| exact-t17: Plastic Explosive (Aerologic Remix) | 140.000000s | correct (t17) | correct (t17) |
| exact-t18: Aquarius (JMD Remix) | 160.000000s | correct (t18) | no-match |
| exact-t19: Body Sensations (original mix) | 170.000000s | correct (t19) | correct (t19) |
| exact-t20: By The Water (Aerologic remix) | 200.000000s | correct (t20) | correct (t20) |
| exact-t21: On Target (Remix) | 190.000000s | correct (t21) | correct (t21) |
| exact-t22: Are You Feeling It? (Rawbase Remix) | 170.000000s | correct (t22) | no-match |
| clean-t01: Sleep Tight (Sergio's Last Remix) | 145.839524s | correct (t01) | correct (t01) |
| clean-t02: Proven Reality | 181.148231s | correct (t02) | correct (t02) |
| clean-t03: Turbulence (2020 Remix) | 377.436327s | correct (t03) | correct (t03) |
| clean-t04: Beautiful Geometry | 267.950771s | correct (t04) | correct (t04) |
| clean-t05: Roots and Shoots | 27.818753s | correct (t05) | correct (t05) |
| clean-t06: Scratching The Surface (Phish Funk Disco Mix) | 102.636190s | correct (t06) | correct (t06) |
| clean-t07: Chasing Rainbows (Marc Burt and Notch remix) | 245.489388s | correct (t07) | correct (t07) |
| clean-t08: Flaw 24 (extended mix) | 169.299138s | correct (t08) | correct (t08) |
| clean-t09: Turn The Corner (Notch remix) | 85.319864s | correct (t09) | correct (t09) |
| clean-t10: Pioneers | 224.266168s | correct (t10) | correct (t10) |
| clean-t11: Tears In My Heart (Notch remix) | 95.630204s | correct (t11) | correct (t11) |
| clean-t12: Beast | 327.025261s | correct (t12) | correct (t12) |
| clean-t13: The Piano Tune | 150.398458s | correct (t13) | correct (t13) |
| clean-t14: Baby Crying | 335.509637s | correct (t14) | correct (t14) |
| clean-t15: Electronaut (edit) | 27.396009s | correct (t15) | correct (t15) |
| clean-t16: Dancefloor Virus 2007 | 48.450499s | correct (t16) | correct (t16) |
| clean-t17: Plastic Explosive (Aerologic Remix) | 72.709909s | correct (t17) | correct (t17) |
| clean-t18: Aquarius (JMD Remix) | 290.651542s | correct (t18) | correct (t18) |
| clean-t19: Body Sensations (original mix) | 302.822517s | correct (t19) | correct (t19) |
| clean-t20: By The Water (Aerologic remix) | 349.115488s | correct (t20) | correct (t20) |
| clean-t21: On Target (Remix) | 59.441791s | correct (t21) | correct (t21) |
| clean-t22: Are You Feeling It? (Rawbase Remix) | 214.781429s | correct (t22) | correct (t22) |
| frozen-t01: Sleep Tight (Sergio's Last Remix) | 37.000000s | no-match | no-match |
| frozen-t02: Proven Reality | 218.500000s | correct (t02) | correct (t02) |
| frozen-t03: Turbulence (2020 Remix) | 406.500000s | correct (t03) | correct (t03) |
| frozen-t04: Beautiful Geometry | 648.500000s | no-match | no-match |
| frozen-t05: Roots and Shoots | 835.000000s | correct (t05) | correct (t05) |
| frozen-t06: Scratching The Surface (Phish Funk Disco Mix) | 971.500000s | correct (t06) | correct (t06) |
| frozen-t07: Chasing Rainbows (Marc Burt and Notch remix) | 1224.000000s | correct (t07) | correct (t07) |
| frozen-t08: Flaw 24 (extended mix) | 1508.500000s | correct (t08) | correct (t08) |
| frozen-t09: Turn The Corner (Notch remix) | 1763.500000s | correct (t09) | correct (t09) |
| frozen-t10: Pioneers | 1980.500000s | no-match | no-match |
| frozen-t11: Tears In My Heart (Notch remix) | 2266.500000s | correct (t11) | correct (t11) |
| frozen-t12: Beast | 2604.000000s | correct (t12) | correct (t12) |
| frozen-t13: The Piano Tune | 2874.500000s | no-match | correct (t13) |
| frozen-t14: Baby Crying | 3117.000000s | correct (t14) | correct (t14) |
| frozen-t15: Electronaut (edit) | 3404.500000s | no-match | no-match |
| frozen-t16: Dancefloor Virus 2007 | 3653.000000s | correct (t16) | correct (t16) |
| frozen-t17: Plastic Explosive (Aerologic Remix) | 3892.500000s | correct (t17) | correct (t17) |
| frozen-t18: Aquarius (JMD Remix) | 4108.500000s | no-match | no-match |
| frozen-t19: Body Sensations (original mix) | 4349.000000s | correct (t19) | correct (t19) |
| frozen-t20: By The Water (Aerologic remix) | 4615.000000s | correct (t20) | correct (t20) |
| frozen-t21: On Target (Remix) | 4957.000000s | no-match | no-match |
| frozen-t22: Are You Feeling It? (Rawbase Remix) | 5274.500000s | no-match | no-match |
| mix-t01: Sleep Tight (Sergio's Last Remix) | 59.594195s | wrong-match (t02) | wrong-match (t02) |
| mix-t02: Proven Reality | 256.064671s | correct (t02) | correct (t02) |
| mix-t03: Turbulence (2020 Remix) | 303.781542s | no-match | no-match |
| mix-t04: Beautiful Geometry | 576.507075s | no-match | no-match |
| mix-t05: Roots and Shoots | 817.468299s | correct (t05) | correct (t05) |
| mix-t06: Scratching The Surface (Phish Funk Disco Mix) | 1052.047415s | correct (t06) | correct (t06) |
| mix-t07: Chasing Rainbows (Marc Burt and Notch remix) | 1258.284875s | correct (t07) | correct (t07) |
| mix-t08: Flaw 24 (extended mix) | 1438.428254s | correct (t08) | correct (t08) |
| mix-t09: Turn The Corner (Notch remix) | 1645.671746s | correct (t09) | correct (t09) |
| mix-t10: Pioneers | 1971.946236s | no-match | no-match |
| mix-t11: Tears In My Heart (Notch remix) | 2398.435215s | correct (t11) | correct (t11) |
| mix-t12: Beast | 2506.155601s | correct (t12) | correct (t12) |
| mix-t13: The Piano Tune | 2756.275805s | no-match | no-match |
| mix-t14: Baby Crying | 3067.394059s | no-match | correct (t14) |
| mix-t15: Electronaut (edit) | 3267.464308s | correct (t15) | correct (t15) |
| mix-t16: Dancefloor Virus 2007 | 3693.357234s | correct (t16) | correct (t16) |
| mix-t17: Plastic Explosive (Aerologic Remix) | 3886.753741s | correct (t17) | correct (t17) |
| mix-t18: Aquarius (JMD Remix) | 4065.342494s | correct (t18) | correct (t18) |
| mix-t19: Body Sensations (original mix) | 4370.160567s | correct (t19) | correct (t19) |
| mix-t20: By The Water (Aerologic remix) | 4638.765329s | correct (t20) | correct (t20) |
| mix-t21: On Target (Remix) | 5013.645465s | correct (t21) | correct (t21) |
| mix-t22: Are You Feeling It? (Rawbase Remix) | 5352.501451s | no-match | no-match |
| n01: Cruising for Goblins | 69.357551s | correct-rejection | correct-rejection |
| n02: Go Cart | 101.462041s | correct-rejection | correct-rejection |
| n03: Jerry Five | 75.026122s | correct-rejection | correct-rejection |
| n04: Raving Energy (faster) | 116.822041s | correct-rejection | correct-rejection |
| n05: Reformat | 104.740408s | correct-rejection | correct-rejection |
| n06: Shiny Tech II | 106.333878s | correct-rejection | correct-rejection |
| n07: Pamgaea | 79.284082s | correct-rejection | correct-rejection |
| n08: Blippy Trance | 55.003787s | correct-rejection | correct-rejection |

See [protocol](E011-song-index.md) and [raw evidence](../evidence/E011/README.md).
