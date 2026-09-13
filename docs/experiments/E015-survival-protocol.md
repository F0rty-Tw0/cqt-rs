# E015: locate evidence loss with known source trajectories

Status: frozen before E015 extraction or diagnostic outcomes. Diagnostic only;
no matcher, thresholds, candidate budget or production behavior changes.

## Question and source

For E014 failures, does the correct trajectory lose spectral peaks, triplet
fingerprints, retrieval support, or fresh continuation verification? Use the
exact E014 executable from source `748dcd0e4ed4d1431a39a6878e99f1ee7075ccae`,
SHA-256 `1b1c5968ce134bda1b559da9b8bdade829c30070c4960700da6015988c0ed9d7`.
Existing `monitor fingerprint FILE.wav` exports native peaks/hashes. No build
or simulated replacement of the audio frontend is needed.

## Frozen cases

Use all 22 E014 full references and all 22 plays in each of five existing
programmes: gain, noise_10db, noise_0db, voice_0db, combined. Keep successes
and failures: 110 plays, 27 native extraction runs. The gain programme controls
for source content and programme placement. Preserve all original PCM hashes,
truth, native commands, binary identity and extraction outputs.

The known mapping is `source_seconds = source_start + tempo *
(query_seconds - insertion_start)`, with tempo 1 except combined 0.88;
query pitch is zero except combined +4 CQT bins (+2 semitones).
This is supplied truth for offline diagnosis, not a recognition success.
No correct alignment is available for real DJ mixtures, so do not fabricate
oracle mappings for those recordings.

## Measurements

Use every global two-second interval wholly inside each insertion, with a
0.15-second margin at both insertion edges to limit peak-picker boundary effects.
Keep every play even when no interior window passes. Map peaks with the existing
four-reference-frame / one-bin tolerance. Reproduce the native verifier's
forward and inverse checks, including its query-defined reference span.
Report unique reference peaks recovered and matched-query fraction separately.
Counts need not be one-to-one; do not call matched-query counts independent votes.

For every eligible query hash, require its anchor and end inside the insertion
with the same margin. Count whether a reference hash exists with the native
key tolerances (±1 for both bin differences and ratio), anchor error ≤4 reference
frames, anchor-bin error ≤1, and span error ≤8 reference frames. Report before
and after the existing per-song/key index cap (8), query hashes supported,
reference hashes recovered, and anchor counts. These are geometric survival
measurements, not fitted vote counts or confidence. Do not assign a production
retrieval threshold to them.

Validate an independent Python triplet generator against all exported native
hash rows. Recount verification counts for recorded E014 hypotheses from native
peaks; retain mismatches due to serialized parameter precision or batch/stream
semantics explicitly. Check exact self alignment and known-shift/tempo synthetic
fixtures, injected unmatched peaks, and removed peaks.

For each play, compare the oracle's fresh checks to the E014 retrieval/active/
pending observations and accepted events. Count oracle checks ≥0.4 (start) and
≥0.3 (hold), longest consecutive streaks, and opportunities for two consecutive
checks. Record below-threshold retrieval, no compatible source position, pending
prediction failure, and hold failures separately. A single passing check is not
a start. Report wrong source positions even for correct song identities.

## Decision and evidence

Select the next single mechanism from measured losses. If peaks disappear,
prioritize peak robustness; if peaks survive while triplets fail, investigate
triplet partner selection / a bounded pair fallback; if true-alignment checks
pass while the tracker fails, investigate trajectory fitting/continuation.
Do not implement or tune that next mechanism in E015.

This is an explanatory development experiment on known recordings and synthetic
speech. It cannot establish real-human voiceover, recording-disjoint accuracy,
or universal 100% matching. Keep E014's rejected default-switch decision.

Budget: 27 extraction runs, up to four workers with one Rayon thread each,
≤15 minutes extraction and ≤15 minutes diagnostic computation; no speed claim.
Use synchronous per-run writes with atomic completion, strict resume identities,
and a complete evidence inventory. Preserve raw text compressed without audio
or executables. Commit protocol, evaluator, regression tests, all per-play and
per-window findings, limitations and exact next action; update existing PR #4.
The user explicitly authorized committing all findings and updating that PR.

Pre-extraction correction: source inspection found `builder.build(8)`; the
initial protocol text incorrectly said 64. Corrected before any E015 output.
