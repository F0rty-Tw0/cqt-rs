# E010 results: full chunk coverage versus midpoint references

**2/21 → 13/21 supported song identities.** All 44 native runs completed on 22 actual ten-second mix clips. t05 has an unverified label and is excluded from this accuracy denominator.

| Reference configuration | Correct | Wrong top identity | No match |
| --- | ---: | ---: | ---: |
| midpoint | 2 | 0 | 19 |
| chunks | 13 | 0 | 8 |

Gains: t02, t03, t06, t07, t08, t09, t12, t14, t16, t17, t19. Lost correct identities: none. The predeclared exploratory acceptance rule passes.

Paired difference: 52.4 percentage points; descriptive track-bootstrap 95% interval [33.3, 71.4] points (10,000 resamples, seed 20260913). This conditions on 21 tracks from one known mix and algorithmic labels; it does not cover annotation error or generalization to new mixes.

| Song | Mix cut starts | Midpoint | All chunks |
| --- | ---: | --- | --- |
| t01: Sleep Tight (Sergio's Last Remix) | 37.0s | no-match | no-match |
| t02: Proven Reality | 218.5s | no-match | correct (t02) |
| t03: Turbulence (2020 Remix) | 406.5s | no-match | correct (t03) |
| t04: Beautiful Geometry | 648.5s | no-match | no-match |
| t05 (unverified): Roots and Shoots | 835.0s | no-match | no-match |
| t06: Scratching The Surface (Phish Funk Disco Mix) | 971.5s | no-match | correct (t06) |
| t07: Chasing Rainbows (Marc Burt and Notch remix) | 1224.0s | no-match | correct (t07) |
| t08: Flaw 24 (extended mix) | 1508.5s | no-match | correct (t08) |
| t09: Turn The Corner (Notch remix) | 1763.5s | no-match | correct (t09) |
| t10: Pioneers | 1980.5s | no-match | no-match |
| t11: Tears In My Heart (Notch remix) | 2266.5s | correct (t11) | correct (t11) |
| t12: Beast | 2604.0s | no-match | correct (t12) |
| t13: The Piano Tune | 2874.5s | no-match | no-match |
| t14: Baby Crying | 3117.0s | no-match | correct (t14) |
| t15: Electronaut (edit) | 3404.5s | no-match | no-match |
| t16: Dancefloor Virus 2007 | 3653.0s | no-match | correct (t16) |
| t17: Plastic Explosive (Aerologic Remix) | 3892.5s | no-match | correct (t17) |
| t18: Aquarius (JMD Remix) | 4108.5s | no-match | no-match |
| t19: Body Sensations (original mix) | 4349.0s | no-match | correct (t19) |
| t20: By The Water (Aerologic remix) | 4615.0s | correct (t20) | correct (t20) |
| t21: On Target (Remix) | 4957.0s | no-match | no-match |
| t22: Are You Feeling It? (Rawbase Remix) | 5274.5s | no-match | no-match |

## Interpretation and limits

Both configurations use the same pinned executable, default gates and modal fit disabled. Every query is a fresh native process; each searches the entire reference set. The chunk configuration indexes 858 complete sections and 22 unpadded tails, mapped to 22 parent songs. The repeated-hash cap applies per chunk, so this treatment changes segmentation as well as reference coverage.

Accepted starts on supported queries: midpoint 2, chunks 35. Wrong-parent starts: 0 versus 0. Repeated parent starts: 0 versus 22. Top prediction is the strongest accepted start, with deterministic ties; chunk scores are not summed.

Native index storage: 3,372,328 bytes for midpoint and 119,553,164 bytes for chunks. These are the executable's approximate index-byte estimates, not process peak RSS. Index construction occurs again in each process. Single-run timings and concurrent annotation/test work are not speed evidence.

The mix clips are digital cuts, not microphone recordings. No negative music, crossfade-specific, pitch/EQ/voice treatment or fresh recording-disjoint evaluation is added here. t05 remains an annotation gap; its provisional outcome must not be described as verified accuracy. Reported position includes the reference chunk offset, but repeated passages can make position ambiguous.

Raw output hashes, terminal ten-second durations, index counts, frozen query identity and every outcome were checked again. Four truncated decoded caches were repaired to exact original hashes before final annotation; prepared native inputs kept their hashes. The overall research goal remains incomplete.

See [protocol and reproduction](E010-chunk-queries.md) and [evidence](../evidence/E010/README.md).
