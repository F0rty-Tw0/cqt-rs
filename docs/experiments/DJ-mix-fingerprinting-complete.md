# DJ-mix fingerprinting: complete experiment record

Project: **cqt-rs**, [draft PR #4](https://github.com/F0rty-Tw0/cqt-rs/pull/4). Date: **2026-09-13**.

## Outcome

Implemented and tested an experimental **two-second sequential matcher**. All **205 new native runs** completed. It reduced duplicate detections in the controlled programme, but lost song identities in real-mix queries. The no-regression acceptance gate failed, so **the existing defaults remain unchanged**. The experiment does not achieve reliable identification of all 22 songs or precise start/end boundaries.

With modal fitting on both sides: random-mix identification changes **17/22 → 16/22**, and full-mix song coverage changes **21/22 → 19/22**. On the controlled 25-play programme, both identify **25/25**; duplicate starts change **3 → 1**, median start notification **1.672 → 6.429 seconds**, and median end notification **9.618 → 6.422 seconds**. Some boundary estimates remain five to six seconds early.

## What we set out to test

1. Index the original versions of all 22 songs in a DJ mix, initially as ten-second reference chunks with parent-song metadata.
2. Match one ten-second mix excerpt per song against the full catalogue, and explain why complete reference coverage did not yield 100% recognition.
3. Compare exact clean clips, random clean clips and actual mixed audio, and separate coverage from alignment/source-version problems.
4. Test the proposal to divide listening into successive two-second observations, accumulate evidence for the same song and advancing source position, and report when its supported interval starts and ends.

## Earlier experiments and source correction

### E010: coverage with ten-second reference chunks

The original experiment compared 22 midpoint references with 880 independent chunk references. For 21 algorithmically supported mix labels, correct identities increased **2/21 → 13/21**, with no lost correct identities and no accepted wrong-song starts. The 22nd label, t05, was unverified. The originally frozen mix excerpts were selected near supported alignment spans; they were **not random**.

Chunks had separate internal identities. Mapping an accepted chunk back to its parent song happened after detection; evidence from different chunks was not combined before the decision. The chunk experiment produced 35 starts for 13 parent songs, including 22 repeated parent starts.

### E011: corrected originals, exact and random controls

The publisher’s original download link for **t05 / Roots and Shoots** supplied **Exodus by Marc Burt**. The corrected **Roots and Shoots by Dave Kent** came from the publisher-linked Internet Archive release and had independent spectral-alignment support in the mix. Historical E010 inputs and results remain preserved.

The corrected catalogue has **864 ten-second chunk references including shorter tails**, or **22 continuous full-song references**. Changing the reference layout also changes the scope of the per-song repeated-hash cap; it is not a pure window-length comparison.

### E012: modal alignment fitting

The existing `--modal-fit` option chooses a representative occupied alignment cell instead of averaging distinct alignments. This recovered two exact clean controls and some mixed-audio queries, but added a wrong identity on one frozen first-track query. It remained experimental.

| Correct identities / rejections | Corrected chunks | Full songs, default fit | Full songs, modal fit |
| --- | ---: | ---: | ---: |
| Exact database clips | 22/22 | 20/22 | 22/22 |
| Random clean originals | 22/22 | 22/22 | 22/22 |
| Previously frozen mix clips | 14/22 | 15/22 | 16/22 |
| Random mix clips | 15/22 | 16/22 | 17/22 |
| Known negative clips rejected | 8/8 | 8/8 | 8/8 |

The four remaining random modal no-matches were **t04, t10, t13 and t22**. All four approximately corresponding clean-source passages were recognized at the same settings. This supports investigating fingerprint survival in mixed audio; it does not isolate a particular DJ effect.

Random t01 was labelled t01 but accepted as t02. A separate spectral diagnostic supports both tracks near that location, compatible with overlap or cue ambiguity. The original scored label and failure remain visible. The t22 publisher/remix tag discrepancy also remains documented; audio alignment supports the chosen file without proving the two remix names are aliases.

## Catalogue and metadata

The source mix is the [Toucan 2020 mix](https://www.toucanmusic.com/mixes/tou2020), with a decoded duration of **5436.510771 seconds**. The corrected t05 release is [tou285](https://www.toucanmusic.com/releases/tou285), also available on [Internet Archive](https://archive.org/details/tou285). Source URLs, licences, hashes and exact sample cuts are retained in the manifests and evidence archives.

| ID | Song | Decoded original duration (s) | Ten-second references including tail |
| --- | --- | ---: | ---: |
| t01 | Sleep Tight (Sergio's Last Remix) | 385.309 | 39 |
| t02 | Proven Reality | 428.613 | 43 |
| t03 | Turbulence (2020 Remix) | 439.774 | 44 |
| t04 | Beautiful Geometry | 479.425 | 48 |
| t05 | Roots and Shoots | 351.948 | 36 |
| t06 | Scratching The Surface (Phish Funk Disco Mix) | 257.635 | 26 |
| t07 | Chasing Rainbows (Marc Burt and Notch remix) | 434.832 | 44 |
| t08 | Flaw 24 (extended mix) | 340.496 | 35 |
| t09 | Turn The Corner (Notch remix) | 350.717 | 36 |
| t10 | Pioneers | 330.710 | 34 |
| t11 | Tears In My Heart (Notch remix) | 427.752 | 43 |
| t12 | Beast | 388.467 | 39 |
| t13 | The Piano Tune | 424.229 | 43 |
| t14 | Baby Crying | 555.938 | 56 |
| t15 | Electronaut (edit) | 360.745 | 37 |
| t16 | Dancefloor Virus 2007 | 439.693 | 44 |
| t17 | Plastic Explosive (Aerologic Remix) | 298.318 | 30 |
| t18 | Aquarius (JMD Remix) | 324.885 | 33 |
| t19 | Body Sensations (original mix) | 355.019 | 36 |
| t20 | By The Water (Aerologic remix) | 414.662 | 42 |
| t21 | On Target (Remix) | 398.080 | 40 |
| t22 | Are You Feeling It? (Rawbase Remix) | 354.691 | 36 |

The metadata includes stable song ID, title, source URL and attribution, audio hash, sample rate, duration, exact sample boundaries, reference offset and query-selection seed/rule. Native hashes retain song ID, reference anchor frame, frequency bin and span. Keeping one song identity with global offsets lets observations refer to a shared source timeline.

## Implemented two-second algorithm

- Index complete original songs once per process, under one ID per song. Do not make every observation a separate song.
- Keep CQT, peak picking and triplet hashing continuous across observation boundaries.
- Assign every query hash to exactly one half-open observation interval by its anchor frame.
- Close a window only after fingerprint lookahead guarantees that its hashes are complete. Round cumulative boundaries so five observations fit a ten-second excerpt.
- Match each observation separately and verify its query peaks against each song candidate.
- Accumulate up to five consecutive compatible observations, requiring at least two before a start.
- Require consistent pitch, tempo and predicted source position; reset pending evidence after a gap or incompatible hypothesis.
- Keep separate active states for overlapping songs. Confirm a changed trajectory before replacing an active play.
- Allow a short dropout; release an unsupported play after the configured interval, rounded to observation cadence.
- Report supported boundary estimates separately from the time at which a detection is announced. A partial final observation cannot confirm or extend a play.

| Decision setting | Value used |
| --- | --- |
| Observation duration | 2 seconds, quantized at cumulative frame boundaries |
| Confirmation history | 2 to 5 consecutive observations |
| Confidence score | `100 × evidence / (evidence + 40)` |
| Start threshold | 70; at least 94 consistent votes are required |
| Start alignment | At least 0.4 in each contributing observation |
| Alignment hold | At least 0.3 with current evidence and a compatible trajectory |
| Pitch agreement | Within 2 bins |
| Tempo agreement | Within 0.05 |
| Predicted-position agreement | Within 0.5 seconds |
| Release interval | Existing 3 seconds, quantized to observation cadence |
| Fitting comparison | Mean and modal evaluated separately |

Votes can share underlying peaks, including across observation boundaries. The confidence score is not a calibrated probability. Short observations do not create fingerprint matches that are absent from the mixed audio. For example, the five observations of random mix t04 provided only **10 + 11 + 7 + 4 + 12 = 44** candidate votes, with inconsistent source positions and insufficient alignment.

## Usage and output

```sh
cargo run --release -p cqt-monitor -- \
  --sequence-seconds 2 --modal-fit \
  --watch song_1=original_1.wav --watch song_2=original_2.wav \
  --stream mix.wav
```

`--sequence-seconds` is opt-in and replaces `--window`, `--report` and `--verify-seconds` for this mode. `--modal-fit` remains independently optional. Supply all 22 `--watch` entries for the full catalogue. The default path is unchanged when sequence mode is absent.

| JSON field/event | Meaning |
| --- | --- |
| `observation` | Candidate evidence, fit, source position and verification for one observation |
| `window` | Observation bounds, completeness, hash count and peak count, including empty intervals |
| `start` / `end` | Confirmed tracker state changes |
| `t` | Logical query time of the event |
| `consumed` | Input audio received when the notification occurs |
| `estimated_start` / `estimated_end` | First and last supported interval edges; not independently verified audible boundaries |
| `source_at_estimated_start` | Estimated position within the original song at the supported start |

Two-second observations do not promise two-second detection. Fingerprint lookahead, confirmation and input-block delay contribute to notification latency. Repeated source passages can still produce the correct song identity at the wrong source position.

Implementation files: [monitor/src/bin/monitor.rs](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/monitor/src/bin/monitor.rs), [monitor/src/bin/monitor/sequence.rs](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/monitor/src/bin/monitor/sequence.rs), [scripts/sequence_eval.py](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/scripts/sequence_eval.py).

## Complete E013 results


Status: native execution and correctness checks verified; default-switch acceptance rejected.

The opt-in `--sequence-seconds 2` prototype keeps one full-song identity and source timeline, matches disjoint two-second anchor windows over continuous fingerprints, and confirms only after at least two aligned, trajectory-consistent observations. It retains at most five observations, and reports supported start/end estimates separately from notification time. It does not promise identification after two seconds or exact audible boundaries.

### Isolated ten-second queries

All 96 E011 queries were reused unchanged for each sequence configuration: 192 new native runs. Comparators are the recorded E011 full-song mean-fit and E012 full-song modal-fit runs. Both sides use the corrected 22-song catalogue. Modal comparisons add only sequence mode; they do not conflate sequence mode with the modal alignment change.

| Query group | Default → sequence | Modal → sequence modal |
| --- | ---: | ---: |
| exact | 20/22 → 19/22 | 22/22 → 22/22 |
| clean | 22/22 → 16/22 | 22/22 → 21/22 |
| frozen | 15/22 → 11/22 | 16/22 → 14/22 |
| mix | 16/22 → 14/22 | 17/22 → 16/22 |
| negative | 8/8 → 8/8 | 8/8 → 8/8 |

Negative rows count correct rejections. A correct identity does not establish a correct source position.

| Configuration/group | Gains | Lost correct identities | Wrong accepted starts | Duplicate starts |
| --- | --- | --- | ---: | ---: |
| sequence / exact | — | exact-t01 | 0 | 0 |
| sequence / clean | — | clean-t03, clean-t05, clean-t13, clean-t16, clean-t21, clean-t22 | 0 | 0 |
| sequence / frozen | — | frozen-t03, frozen-t05, frozen-t13, frozen-t20 | 0 | 0 |
| sequence / mix | — | mix-t19, mix-t21 | 1 | 0 |
| sequence / negative | — | — | 0 | 0 |
| sequence-modal / exact | — | — | 0 | 0 |
| sequence-modal / clean | — | clean-t16 | 0 | 0 |
| sequence-modal / frozen | — | frozen-t05, frozen-t13 | 1 | 0 |
| sequence-modal / mix | — | mix-t21 | 1 | 1 |
| sequence-modal / negative | — | — | 0 | 0 |

The predeclared no-regression gate fails. Keep this prototype opt-in; do not replace the existing defaults. Confidence remains a decision score, not a probability. The descriptive paired bootstrap intervals in the JSON summary resample these known queries; they do not establish new-recording accuracy.

### Controlled continuous programme

One fixed 584-second programme contains 25 known insertion intervals: all 22 random clean clips, a repeat with a one-second internal dropout, and two overlapping clips with a two-second crossfade. There are 336 seconds outside the union of inserted-play intervals. The dropout remains inside its play interval. Every start is attributed once; extra starts, misses and premature logical endings remain visible.

| Mode | Plays detected | Duplicate starts | False starts | Premature endings |
| --- | ---: | ---: | ---: | ---: |
| default | 25/25 | 3 | 0 | 3 |
| sequence | 23/25 | 1 | 0 | 1 |
| modal | 25/25 | 3 | 0 | 3 |
| sequence-modal | 25/25 | 1 | 0 | 1 |

| Mode | Start notification p50 / p95 (s) | End notification p50 / p95 (s) | Start estimate error min / max (s) | End estimate error min / max (s) |
| --- | ---: | ---: | ---: | ---: |
| default | 1.673 / 2.490 | 9.618 / 10.049 | — / — | — / — |
| sequence | 6.447 / 10.437 | 6.423 / 6.460 | -0.003 / 4.000 | -5.999 / 0.003 |
| modal | 1.672 / 2.490 | 9.618 / 10.049 | — / — | — / — |
| sequence-modal | 6.429 / 6.468 | 6.422 / 6.460 | -0.003 / 1.999 | -5.999 / 0.003 |

Delay distributions include detected plays only; the detection denominators above must accompany them. For duplicated plays, timing uses the first start and its first end, so fragmentation cannot be hidden by selecting a later successful segment. A premature ending means the end event’s logical query time precedes the insertion interval’s end. Notification uses `consumed`, including transform/hash lookahead and input-block delay. Boundary errors are relative to exact insertion samples, not a human audibility annotation. The main programme boundaries align with the two-second grid; tiny typical errors mostly reflect frame quantization and do not establish millisecond boundary accuracy.

Sequence observation and decision content is identical with input blocks of 257, 4096 and 65536 samples; consumed-audio notification timestamps may differ with block size. All 196 sequence outputs in the main matrix cover every emitted hash and peak exactly once in contiguous windows; each ten-second query contains five complete observations.

### Boundary phase diagnostic

After the main results, prepend exactly one second of silence to shift every insertion boundary off the two-second grid. Freeze this byte-identical shift and its labels before two additional modal baseline/sequence runs; no thresholds change. This is a timing sensitivity diagnostic on the same recordings, not a new acceptance set.

| Mode | Detected | Duplicates | Premature endings | Median start / end notification delay (s) | Start estimate error min / max (s) | End estimate error min / max (s) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| modal | 25/25 | 2 | 2 | 1.677 / 9.602 | — | — |
| sequence-modal | 25/25 | 1 | 2 | 5.446 / 7.428 | -1.003 / 3.002 | -5.001 / 1.003 |

Both modes have zero false starts in this shifted programme. Typical sequence boundary errors now approach one second, and some estimates are wrong by five seconds. Together with the six-second early end estimate in the original programme, this rejects a claim of reliable two-second boundary accuracy.

The continuous coverage audit below uses the same `start.t` to `end.t` interval for every mode. The runner’s original sequence supported-edge coverage is retained separately in the raw results; it is not used for the comparative table.

### Entire real DJ mix

This additional diagnostic was declared after early short-query results and is exploratory. Four runs use the same 90.61-minute Toucan mix and corrected full references. Coverage means an active interval overlaps a frozen query location for the expected song. It does not establish presence throughout that location or validate audible cue boundaries. Extra detections are preserved; no forced merging into one result per playlist song.

| Mode | Distinct songs detected | Start/end pairs | Frozen locations covered | Random locations covered |
| --- | ---: | ---: | ---: | ---: |
| default | 21/22 | 80 | 16/22 | 18/22 |
| sequence | 18/22 | 62 | 14/22 | 15/22 |
| modal | 21/22 | 113 | 17/22 | 18/22 |
| sequence-modal | 19/22 | 63 | 16/22 | 17/22 |

| Song | Default pairs | Sequence pairs | Modal pairs | Sequence modal pairs |
| --- | ---: | ---: | ---: | ---: |
| t01 | 7 | 0 | 6 | 0 |
| t02 | 6 | 7 | 7 | 7 |
| t03 | 12 | 2 | 20 | 8 |
| t04 | 6 | 0 | 10 | 2 |
| t05 | 3 | 4 | 3 | 6 |
| t06 | 1 | 2 | 1 | 1 |
| t07 | 1 | 1 | 1 | 1 |
| t08 | 2 | 3 | 2 | 2 |
| t09 | 3 | 2 | 3 | 3 |
| t10 | 0 | 0 | 0 | 0 |
| t11 | 1 | 2 | 1 | 1 |
| t12 | 3 | 3 | 5 | 2 |
| t13 | 7 | 1 | 10 | 4 |
| t14 | 1 | 3 | 1 | 1 |
| t15 | 7 | 2 | 13 | 8 |
| t16 | 1 | 2 | 1 | 2 |
| t17 | 2 | 6 | 3 | 4 |
| t18 | 1 | 7 | 1 | 1 |
| t19 | 2 | 4 | 2 | 2 |
| t20 | 1 | 10 | 1 | 4 |
| t21 | 8 | 1 | 15 | 4 |
| t22 | 5 | 0 | 7 | 0 |

### Source-position ambiguity

Post-first-results diagnostic: compare the strongest accepted correct-identity start against the exact known clean-source position (`source excerpt start + query time`). Repeated sections can verify at another location in the same song. This occurred in the previous matcher too. A correct song label must not be presented as exact source localization.

| Mode / group | Correct identities evaluated | Position within 2 s | Median absolute error (s) | Worst absolute error (s) |
| --- | ---: | ---: | ---: | ---: |
| default / exact | 20 | 19 | 0.003 | 15.489 |
| default / clean | 22 | 21 | 0.004 | 14.544 |
| modal / exact | 22 | 21 | 0.003 | 187.743 |
| modal / clean | 22 | 21 | 0.003 | 14.544 |
| sequence / exact | 19 | 19 | 0.002 | 0.014 |
| sequence / clean | 16 | 16 | 0.001 | 0.016 |
| sequence-modal / exact | 22 | 21 | 0.001 | 187.742 |
| sequence-modal / clean | 21 | 21 | 0.001 | 0.003 |

### Every mix-query outcome

C = correct identity, M = no match, W = wrong identity.

#### frozen

| Song | Default | Sequence | Modal | Sequence modal |
| --- | --- | --- | --- | --- |
| t01 / Sleep Tight (Sergio's Last Remix) | M | M | W | W |
| t02 / Proven Reality | C | C | C | C |
| t03 / Turbulence (2020 Remix) | C | M | C | C |
| t04 / Beautiful Geometry | M | M | M | M |
| t05 / Roots and Shoots | C | M | C | M |
| t06 / Scratching The Surface (Phish Funk Disco Mix) | C | C | C | C |
| t07 / Chasing Rainbows (Marc Burt and Notch remix) | C | C | C | C |
| t08 / Flaw 24 (extended mix) | C | C | C | C |
| t09 / Turn The Corner (Notch remix) | C | C | C | C |
| t10 / Pioneers | M | M | M | M |
| t11 / Tears In My Heart (Notch remix) | C | C | C | C |
| t12 / Beast | C | C | C | C |
| t13 / The Piano Tune | C | M | C | M |
| t14 / Baby Crying | C | C | C | C |
| t15 / Electronaut (edit) | M | M | M | M |
| t16 / Dancefloor Virus 2007 | C | C | C | C |
| t17 / Plastic Explosive (Aerologic Remix) | C | C | C | C |
| t18 / Aquarius (JMD Remix) | M | M | C | C |
| t19 / Body Sensations (original mix) | C | C | C | C |
| t20 / By The Water (Aerologic remix) | C | M | C | C |
| t21 / On Target (Remix) | M | M | M | M |
| t22 / Are You Feeling It? (Rawbase Remix) | M | M | M | M |

#### mix

| Song | Default | Sequence | Modal | Sequence modal |
| --- | --- | --- | --- | --- |
| t01 / Sleep Tight (Sergio's Last Remix) | W | W | W | W |
| t02 / Proven Reality | C | C | C | C |
| t03 / Turbulence (2020 Remix) | M | M | C | C |
| t04 / Beautiful Geometry | M | M | M | M |
| t05 / Roots and Shoots | C | C | C | C |
| t06 / Scratching The Surface (Phish Funk Disco Mix) | C | C | C | C |
| t07 / Chasing Rainbows (Marc Burt and Notch remix) | C | C | C | C |
| t08 / Flaw 24 (extended mix) | C | C | C | C |
| t09 / Turn The Corner (Notch remix) | C | C | C | C |
| t10 / Pioneers | M | M | M | M |
| t11 / Tears In My Heart (Notch remix) | C | C | C | C |
| t12 / Beast | C | C | C | C |
| t13 / The Piano Tune | M | M | M | M |
| t14 / Baby Crying | C | C | C | C |
| t15 / Electronaut (edit) | C | C | C | C |
| t16 / Dancefloor Virus 2007 | C | C | C | C |
| t17 / Plastic Explosive (Aerologic Remix) | C | C | C | C |
| t18 / Aquarius (JMD Remix) | C | C | C | C |
| t19 / Body Sensations (original mix) | C | M | C | C |
| t20 / By The Water (Aerologic remix) | C | C | C | C |
| t21 / On Target (Remix) | C | M | C | M |
| t22 / Are You Feeling It? (Rawbase Remix) | M | M | M | M |

### Provenance and limits

- Evaluated native source: `44a7411d2df1f43056b4fba5ca9d3bd43b2eb7f6`; binary SHA-256 `6450de94d60726f94ab156b7476e214fd09e2d7171e94fffe6c0f6e1fda4e613`.
- 205 new native runs: 192 short-query candidates, four controlled programme configurations, two block-size parity variants, four full-mix configurations, one live-PCM parity run, and two phase-offset timing controls. All commands, exits, inputs, raw observations/events and identities are retained in the evidence archives.
- Seven new Rust sequence tests; all 20 Python regressions pass; live PCM and file input agree on all observation/window/start/end fields for the same ten-second clip. All 12 CI checks passed on the evaluated native source. Final publication checks are separately linked in PR #4.
- Known development recordings and algorithmic mix labels. The t01/t02 overlap ambiguity and previously corrected t05 source identity remain as documented in E011/E012. Only 80 seconds of known negative music per sequence configuration; no production false-alarm rate.
- No speed claim. Short-query wall time includes repeated reference indexing; the full-mix diagnostic overlapped some query processes. Native sample-time delays are separate from wall time.
- Initial code checkpoints required formatting and lint fixes; these were resolved before the evaluated binary. No thresholds or query labels were changed after outcomes.

Reproduction details are included below in the [primary protocol](#frozen-primary-protocol), [continuous diagnostic protocol](#continuous-diagnostic-protocol) and [evidence inventory](#evidence-build-and-reproduction). Next algorithm question: preserve competing source-position hypotheses and verify their predicted continuation, rather than forcing each short interval to commit to one repeated passage. The four weak mixed-audio excerpts also need retrieval/fingerprint-survival work; window size alone does not create absent evidence. Keep the prototype experimental. No merge or release.


## Frozen primary protocol

The following is the protocol as recorded before the main experiment. Its original “planned” status is retained as historical context; results are completed above.


Status: planned; protocol frozen before implementation/results, 2026-09-13.

Hypothesis: separate, contiguous two-second observations can contribute
evidence to one song and an advancing source position, reducing duplicate
chunk identities and providing explicit supported start/end estimates.
Shortening physical WAV references is not the hypothesis: continuous CQT,
peak picking and triplets must retain context across observation edges.

Baseline: PR #4 `472bb2b101e5968fad05e06a794eaf0cbe9e45e8`;
native executable/source and frozen 96-query inputs from E011/E012.
Candidate: opt-in `--sequence-seconds 2`, same full 22-song references,
hashing, per-song index cap, confidence function/threshold (70/half=40),
and verification tolerances. Default matcher/tracker behavior stays intact.

Assign every hash to exactly one half-open interval by its query anchor.
Close intervals only once the fingerprint lookahead guarantees completeness.
Match each interval separately, verify only peaks inside it, and combine up
to five consecutive aligned observations of the same song. Require at least
two full observations, aggregate confidence >=70, alignment >=0.4 on each,
pitch agreement within two bins, tempo within 0.05, and predicted position
within 0.5 seconds. Evidence counts are not probabilities or independent
statistical trials: triplets may share peaks across interval boundaries.

An active song can survive a short gap; require current alignment >=0.3 and
a compatible trajectory to renew support. End after the existing three-second
release interval, rounded up to the observation cadence. Reset pending
evidence on missing/incompatible observations. Confirm a changed trajectory
before replacing an active play. Track each song separately for overlaps.
Partial final intervals are reported but cannot confirm or extend a play.
Report first/last supported interval edges separately from notification
time and source position; these are coarse evidence estimates, not verified
audible boundaries or guaranteed two-second error bounds.

Evaluation, in order:

1. Rust state tests: two weak agreeing observations can cross the unchanged
   aggregate gate; a single strong observation cannot confirm; incompatible
   offsets, missing intervals and duplicates cannot accumulate; overlapping
   songs, dropouts, repeated plays, EOF and bounded history are covered.
   Hash assignment and output must be invariant to audio block size.
2. Reuse all 96 frozen E011 queries: 22 exact, 22 random clean, 22 old mix,
   22 random mix, eight known negatives. Compare mean fit first; repeat with
   existing modal fit as a separately labelled diagnostic, without tuning.
   Report all misses, wrong and duplicate starts, paired outcomes and delays.
3. Controlled continuous programme: the 22 random clean ten-second clips,
   each preceded by six seconds of silence and followed by eight; preserve
   all source positions and exact sample boundaries. Add a repeated play,
   a one-second internal dropout, and a two-second crossfade of the first
   two random clean clips. Baseline/candidate use identical bytes and full
   references. Measure per-play detection, duplicate/false starts, notification
   delay, estimated boundary errors and premature endings separately.

Acceptance: all correctness/CI gates pass; no lost clean or mix identities,
no added wrong/negative starts, no added continuous-play misses or duplicates.
Reject a default switch if any quality gate fails. Preserve an opt-in
prototype only with its measured limitations explicit. This known recording
pool is development data, not held-out evaluation. No speed claim: fresh
process indexing dominates short-query wall time; repetitions are not a
benchmark. Real DJ mix cues are not sufficient for exact boundary claims.

Commands: `python3 scripts/sequence_eval.py --prepare`, then
`python3 scripts/sequence_eval.py --binary <exported-candidate> --run`;
`python3 scripts/sequence_eval.py --report`. Native build/test runs on GitHub
Actions because local Cargo/rustc are unavailable. Retain source/binary/input/
evaluator identities, commands, exits, raw output, runtime and toolchain.
Budget: two candidate 96-query matrices, four controlled-programme runs,
two block-size parity runs; four concurrent query processes with one Rayon
thread each; <=30 minutes native work, <=15 minutes CI per code revision.


## Continuous diagnostic protocol

The following diagnostic was frozen before its four full-mix runs, after the
first isolated-query results. It is retained as historical protocol text.

Planned after early isolated-query outputs, before executing this
diagnostic. The frozen main matrix and acceptance gates are unchanged.

User's aim includes continuous DJ-mix tracking. Compare four configurations
on the entire existing Toucan mix with the corrected full 22-song catalogue:
default mean, two-second sequence mean, default modal, sequence modal. Same
candidate executable, audio, index, thresholds and one Rayon thread per
process. The only within-pair variable is sequence mode. No new tuning.

Record all start/end events, per-song play counts, total detections, and
whether active intervals overlap each of the 44 frozen/random E011 mix-query
locations for its expected song. Mere interval overlap is a coverage measure,
not proof of exact presence throughout a query. Do not call these annotated
audible boundaries or interpret every extra start as a false positive: cues
remain algorithmic and the first-track overlap/label ambiguity persists.
Never collapse repeated detections to manufacture one clean record per song.

Additional budget: four native runs, two concurrent full-mix processes, <=15 minutes
wall time, retaining per-process commands, exits and raw observations. This
is an exploratory known-recording diagnostic with post-outcome selection,
not an independent acceptance set or a speed measurement.

Scheduling update before execution: run at most two full-mix processes
alongside the four short-query processes, using six of the nine available
CPU slots and bounding full-mix PCM memory. This changes wall-time contention,
not sample-time metrics; no timing benchmark is claimed.

Command:
`python3 scripts/sequence_continuous.py --binary target/sequence/binaries/candidate`.

## Evidence, build and reproduction

**Publication scope:** this Markdown contains the complete written experiment
record. Raw E013 ZIP archives and the final supporting source/test edits remain
in the workspace. Automatic approval review rejected publication of an
additional binary archive because authorization covered the Markdown document,
not that payload; no archive is linked from the published branch. The earlier
E010–E012 evidence and evaluated E013 implementation retain their existing links.

Workspace evidence directory: `docs/evidence/E013/`. Runtime audio and working
outputs: `target/sequence/`. These paths are relative to the experiment checkout.


All **205 native runs** completed: 192 frozen query candidates, six controlled
programme/configuration and block-size runs, four full-mix runs, one live-PCM
parity run, and two off-grid boundary controls. The default-switch acceptance
gate is **rejected**; [results](#complete-e013-results) retain every
lost identity, wrong start, duplicate and timing limitation.

### Contents

- `exact.zip`, `clean.zip`, `frozen.zip`, `mix.zip`, `negative.zip`: every native
  stdout/stderr and per-query command/result for both sequence configurations.
- `programme.zip`: four programme configurations plus two block-size variants.
- `provenance.zip`: frozen 96-query/reference/programme manifest, preparation
  snapshot, evaluated source/binary identity, runtime/toolchain/lockfile,
  evaluator snapshots, summaries, source-position audit and live-PCM proof.
- `continuous-{default,sequence,modal,sequence-modal}.zip`: all raw full-mix
  observations/events and per-run results. `continuous-provenance.zip` contains
  its frozen input plan, original results and comparable event-time coverage
  audit. Original supported-edge coverage remains available separately.
- `phase.zip`: the one-second programme shift, exact shifted annotations,
  commands, raw observations and baseline/sequence results; audio is regenerated.
- `archives.json` and `SHA256SUMS`: archive hashes; each archive also has member
  checksums and passes the ZIP CRC check. `independent-recount.json` is a
  standard-library-only recount of accepted native identities and all 205 runs.
- `summary.json`, `audit.json`, `python-tests.*` and `native-ci.json`: readable
  results and verification metadata. Final publication CI is linked in PR #4.

Audio and executables are excluded from Git. Source audio URLs, licences,
attribution and corrected t05 identity are inherited unchanged from
[E011](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/evidence/E011/README.md). The mix and all references/queries were hash-checked
before execution. Preparation used the current workspace paths; each input
also has a content hash independent of its path.

### Native build

Evaluated source: `44a7411d2df1f43056b4fba5ca9d3bd43b2eb7f6`.
Executable SHA-256:
`6450de94d60726f94ab156b7476e214fd09e2d7171e94fffe6c0f6e1fda4e613`.

[Binary export run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34777186001),
artifact `10323677318`, ZIP SHA-256
`ef6973a4e44acfd9f692688880bf91019bf8578086f34ee74b7c314e841da496`.
The artifact used a clean source checkout and Rust 1.98.1; build metadata is
preserved in `provenance.zip`. Initial formatting/lint failures were fixed
before this executable was evaluated. An additional local, unpublished test edit exercises actual emitted windows;
production sequence code remains identical. That test edit is not part of
the evaluated source or this documentation-only publication.

All 12 checks passed on that source: [standard CI](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34777185996),
[CLI proof](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34777186011),
[verifier proof](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34777186010), and
the binary export above. Earlier green checks do not validate later source;
the final-head checks are separately reported in PR #4.

### Recount without audio or Rust

The commands below describe the completed workspace and its retained files.
The new audit, continuous-run, phase and packaging scripts and raw E013 archives
remain local; this publication contains the consolidated Markdown and progress
checkpoint. A fresh Git clone alone does not contain those local files.

```sh
python3 scripts/recount_sequence_archives.py
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

The recount needs only Python's standard library. The complete evaluator test
suite additionally uses numpy, scipy and matplotlib, as specified in GOAL.md.

### Repeat the audio experiment

First reproduce/materialize E011's corrected catalogue and frozen queries,
including its default/modal comparator results. Obtain the pinned executable
from the artifact, or rebuild the stated source with the archived lockfile.
Place it and `candidate-commit.txt` in `target/sequence/binaries`.

```sh
python3 scripts/sequence_eval.py --prepare
python3 scripts/sequence_eval.py --binary target/sequence/binaries/candidate --run
python3 scripts/sequence_eval.py --report
python3 scripts/sequence_audit.py
python3 scripts/sequence_continuous.py --binary target/sequence/binaries/candidate
python3 scripts/sequence_continuous_audit.py
python3 scripts/sequence_phase.py
```

The main runner checkpoints every completed process and validates its binary
and output hash when resuming. The native command and raw outputs are retained
even for failed cases. Four short-query workers and two full-mix workers were
used, each with one Rayon thread. This is not a speed benchmark.

For the live check, take `clean-t01`'s raw signed-16-bit mono PCM from its WAV,
pass the archived watch list with `--stream - --sequence-seconds 2 --modal-fit`,
and compare all observation/window/start/end fields to its file run. This
unpaced pipe check establishes input-path parity, not microphone or wall-clock
real-time latency.

### Interpretation limits

The same known recordings were reused. Only 80 seconds of known negative music
were tested per sequence configuration. Main programme boundaries align with
the two-second grid; the additional phase diagnostic exposes the resulting
timing sensitivity. Full-mix cues are algorithmic and do not provide precise
audible boundaries. The t01/t02 overlap ambiguity remains scored under the
original labels. Correct song identity does not guarantee correct source
position, and lower segment counts do not by themselves establish precision.


## Next steps

1. Keep the tested sequence mode experimental and preserve all misses, duplicates and timing failures.
2. Retain longer retrieval context while evaluating predicted continuation in two-second observations.
3. Preserve competing source-position hypotheses through repeated passages instead of committing prematurely to one occurrence.
4. Test releases driven by fresh alignment separately from confidence supported by older votes, including quiet passages, dropouts and crossfades.
5. Investigate peak/hash survival on t04, t10, t13 and t22; finer observation boundaries alone did not recover them.
6. Establish independent, exclusive mix cue labels and a recording-disjoint evaluation before making broad accuracy or boundary claims.

These are next research steps, not completed improvements. Freeze an E014 hypothesis, comparator, inputs, metrics, acceptance rule and resource budget before another algorithm change. No merge or release has been performed.

## Related historical records

- [docs/experiments/E010-results.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/experiments/E010-results.md)
- [docs/experiments/E011-results.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/experiments/E011-results.md)
- [docs/experiments/E012-results.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/experiments/E012-results.md)
- [docs/experiments/E012-matched-source.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/experiments/E012-matched-source.md)
- [docs/PROGRESS.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/PROGRESS.md)
- [docs/GOAL.md](https://github.com/F0rty-Tw0/cqt-rs/blob/codex/pr4-consolidated-updates/docs/GOAL.md)
