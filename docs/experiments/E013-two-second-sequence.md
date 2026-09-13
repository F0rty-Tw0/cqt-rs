# E013 — successive two-second observations

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
