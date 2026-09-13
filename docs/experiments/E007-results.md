# E007 findings: a real DJ mix with ten-second references

[Toucan Music 2005 to 2020](https://www.toucanmusic.com/mixes/tou2020): 90.61 minutes and 22 listed tracks. A seeded draw selected this mix from ten entries in the label's catalogue before recognition results.

**The requested short-reference matrix completed. The full experiment did not:** the 35-minute job limit interrupted the extra full-track controls. The table retains every completed case; missing diagnostics are not counted as misses or successes.

References are clean ten-second midpoint excerpts from the exact original track versions. Pitch is raised two semitones; tempo is independently reduced 10%. The combined treatment adds 12 seconds of synthetic voice every 30 seconds, matched to local music RMS (0 dB during speech). Slowed streams last 100.68 minutes.

| Stream | PR #3 found | PR #7 found | Coverage (each) | Start events (each) |
| --- | ---: | ---: | ---: | ---: |
| clean | 14/22 | 14/22 | 63.6% | 19 |
| pitch | 12/22 | 12/22 | 54.5% | 13 |
| tempo | 12/22 | 12/22 | 54.5% | 13 |
| combined | 5/22 | 5/22 | 22.7% | 6 |

A listed identity counts once if the monitor emits a start event. Extra starts remain visible; they can represent fragmentation, repetitions or errors. This is track-list coverage, not verified per-play recall or in-mix precision.

Original-snippet control: **21/22** in the known playback windows. Speech/silence negative control: **0 starts over five minutes**, for both versions. The two versions process the same negative audio, so this is five minutes of unique exposure. With zero starts, the one-sided 95% Poisson upper rate bound is 35.9/hour under that model; the small control cannot establish a low field false-alarm rate.

All non-timing events agree exactly in **6 completed baseline/candidate pairs**. PR #7 therefore shows no recognition improvement on these cases. Its earlier verifier microbenchmark improvement must not be presented as an end-to-end speedup here.

## Every track

Y = at least one start for that identity; — = none. Short-reference columns are identical for both versions. The final column is the completed PR #3 full-track diagnostic only.

| # | Artist — exact version | Clean | Pitch | Slower | Combined | PR #3 full clean |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Psychadelik Pedestrian — Sleep Tight (Sergio's Last Remix) | — | — | Y | — | Y |
| 2 | JVS — Proven Reality | Y | — | — | — | Y |
| 3 | Marc Burt — Turbulence (2020 Remix) | Y | Y | — | — | Y |
| 4 | Phish Funk — Beautiful Geometry | — | — | — | — | Y |
| 5 | Dave Kent — Roots and Shoots | — | — | — | — | — |
| 6 | Redmann — Scratching The Surface (Phish Funk Disco Mix) | Y | Y | Y | Y | Y |
| 7 | Frau Holle — Chasing Rainbows (Marc Burt and Notch remix) | Y | Y | Y | Y | Y |
| 8 | Notch — Flaw 24 (extended mix) | Y | Y | Y | — | Y |
| 9 | Redmann — Turn The Corner (Notch remix) | Y | Y | Y | Y | Y |
| 10 | Marc Burt — Pioneers | — | — | — | — | — |
| 11 | Psychadelik Pedestrian — Tears In My Heart (Notch remix) | Y | Y | Y | — | Y |
| 12 | Beat Doctor — Beast | Y | Y | Y | Y | Y |
| 13 | Beat Doctor — The Piano Tune | Y | — | — | — | Y |
| 14 | Fioko — Baby Crying | Y | Y | Y | — | Y |
| 15 | Aerologic — Electronaut (edit) | — | — | — | — | Y |
| 16 | Silverknight & Beat Doctor — Dancefloor Virus 2007 | Y | Y | Y | Y | Y |
| 17 | Redmann — Plastic Explosive (Aerologic Remix) | Y | Y | Y | — | Y |
| 18 | Phish Funk — Aquarius (JMD Remix) | Y | Y | Y | — | Y |
| 19 | Base II — Body Sensations (original mix) | — | — | — | — | Y |
| 20 | Space Invaderz & Psychadelik Pedestrian — By The Water (Aerologic remix) | Y | Y | Y | — | Y |
| 21 | JMD — On Target (Remix) | — | — | — | — | Y |
| 22 | Redmann — Are You Feeling It? (Rawbase Remix) | — | — | — | — | Y |

## What the failures show

The combined treatment recovers 5/22 identities: Redmann — Scratching The Surface (Phish Funk Disco Mix); Frau Holle — Chasing Rainbows (Marc Burt and Notch remix); Redmann — Turn The Corner (Notch remix); Beat Doctor — Beast; and Silverknight & Beat Doctor — Dancefloor Virus 2007.

Sleep Tight (Sergio’s Last Remix), t01, also fails its exact-original snippet control. In the emitted winning-candidate reports its confidence reaches 94.8, but whenever confidence is at least 70 its maximum query alignment is only 0.217 (the start gate is 0.4). Reports reaching alignment 0.4 have confidence no higher than 23.1. High confidence and high alignment occur at different times. The start gate explains the observed rejection; why the matcher selects those positions remains a debugging question. Reports show the highest-evidence song, not every internal candidate.

The completed PR #3 full-reference clean diagnostic finds 20/22 identities (77 starts), versus 14/22 for short references. It misses Roots and Shoots and Pioneers. Longer references help coverage in this case, but the comparison does not isolate omitted excerpts from additional fingerprint evidence or version/alignment problems.

Pitch and tempo separately yield 12/22, but they do not miss precisely the same tracks. Slowing recovers t01 while losing other clean detections. The 5/22 combined result cannot isolate voiceover damage: a pitch-plus-tempo-only treatment and a voice-only treatment would be needed. EQ has not been measured; see [E008](E008-eq-plan.md).

## Timing and unfinished diagnostics

Single-run short-reference wall times are about 39–44 seconds for a 90.61–100.68-minute mix. These are descriptive elapsed times, not process CPU, algorithmic latency, memory measurements or repeated speed benchmarks. Full-reference PR #3 clean processing took 505.863 seconds; indexing/matching costs also change with longer references.

Thirteen of sixteen planned runs finished. Full-reference PR #7 clean was interrupted at approximately 4504.5 seconds of consumed audio; both full-reference combined runs never started. The cancelled run is preserved and excluded from completed counts. No full-reference parity or combined full-reference result is claimed.

## Proof and limitations

Validated 26 completed-run stdout/stderr hashes, all per-track counts and first starts, all reported maxima, and 6 full non-timing event-parity pairs against raw JSONL. The known PR #3 stream-duration formatting defect is repaired only for parsing; raw output remains unchanged. The original artifact ZIP digest also matches GitHub's recorded digest.

The executing harness is commit `64bc53d448697f860586139bb256c9b1764e982b`; baseline is `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`; candidate is `176bce2fef1d1463ff7b815f4f0ee62a21deb435`. CI logs record clean pinned builds, the shared Cargo.lock, Rust 1.98.1 and one Rayon thread. Source/transform hashes, commands, host details and every completed raw output are retained.

**Proof gap:** cancellation prevented the original script’s final metadata write, losing actual executable hashes and its runtime identity document. Pinned build logs identify the source, but cannot reconstruct the exact lost binary hashes. The full acceptance gate remains blocked. The harness now writes identities before work and atomically checkpoints JSON; a hard-kill regression fails on the executed harness and passes after the fix. This repair does not retroactively supply the missing hashes.

The independently downloaded mix has SHA-256 `39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103`, matching the CI source manifest. Its MD5 `9aa5133f8cc5917e6f09ef1f6bef0391` also matches the Internet Archive source metadata. Other source and generated-audio hashes were recorded by the harness, not all independently redownloaded.

The publisher supplies track identities but no independent cue times. Some midpoint excerpts may be omitted from the DJ edit; exact onset delay and correctness of every in-mix start remain unverified. One mix from one catalogue cannot establish generalization, and confidence scores are not probabilities. No thresholds or excerpts were retuned after results.

See [evidence and reproduction](../evidence/E007/README.md) for the artifact, validation manifest, raw numerical summaries and failure logs. Audio is CC BY-NC-SA 4.0 with Toucan/artist attribution for this noncommercial experiment; this is not blanket permission for commercial use.
