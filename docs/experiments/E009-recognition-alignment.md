# E009: alignment diagnosis and EQ evaluation

Status: executed and independently validated, 2026-09-13. User authorized
implementation toward higher matches and actual EQ testing. Draft PR #9
stacks on PR #8 and preserves its prior results. The text below is the frozen
pre-evaluation protocol. See [results](E009-results.md) and
[proof](../evidence/E009/README.md) for outcomes and retained failures.

Hypothesis to investigate before claiming a defect: high hash evidence can
select an incorrect temporal alignment for repetitive short references.
A bounded refinement or alternative-hypothesis verifier might recover the
correct position without lowering confidence or alignment gates. Inspect
actual peaks/hashes and reproduce t01's exact-original miss first. Do not
claim the cause merely from the old confidence/verification maxima.

Baselines: PR #3 `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`, and PR #8
`cdf428e9d6a82127cf36aedd8ddea27f82de81ec` (PR #7 detector behavior).
Build all executables with Rust 1.98.1 and the preserved E007 Cargo.lock.
CI exports native executables and hashes so short investigations can run
locally without the unavailable local Rust compiler. Preserve build SHA,
raw outputs, commands, input hashes, OS and elapsed timing scope.

Development: the 22 E007 originals and their fixed 10-second midpoint
references. They are now development/regression recordings; do not call
new transformations of them held out. Keep the full E007 mix and original
four treatments as regression coverage. Freeze any independent source
recordings before opening their detector results and before candidate
selection. Report all losses and every unmatched-music start.

EQ grid, frozen before results: FFmpeg bass shelves at 200 Hz and treble
shelves at 3000 Hz, +/-6 and +/-12 dB, Q=0.707; bell cuts at 1000 Hz,
-12 dB with Q=0.7 and Q=5; high-pass at 200 Hz and low-pass at 3000 Hz,
2 poles each. Apply these separately, with clean references and common
headroom; no clipping in EQ cases. Keep hard clipping as a separate case.
Include clean, pitch +2 semitones, tempo 0.9, voice-only and combined
pitch/tempo/voice for controlled original-snippet playback. Voice is 0 dB
local RMS, the original fixed text. Keep detector defaults frozen.

Metrics: correctly identified known snippet windows, misses, false starts
outside truth or on unwatched music, duplicate starts, start/end delay,
peak survival and exact hash overlap for isolated EQ; per-track outcomes.
For the real mix use publisher-list coverage with the existing cue-time
limitation. Report uncertainty by recording, not by correlated frames.
Elapsed times are descriptive until five alternating timing pairs exist.

Acceptance: reproduce the original failure and recover it without loss of
previously recovered development controls or additional false starts;
preserve Rust correctness, feature/API semantics and unsafe-code prohibition.
Any quality/performance tradeoff or held-out loss stays visible. A single
recording repair does not demonstrate broad recognition robustness. Do
not weaken a gate, move a reference, or discard difficult cases to pass.

Resource budget: native build CI <=10 minutes per concrete candidate; local
controlled-grid evaluation <=15 minutes plus full-mix transformation and
comparison <=20 minutes. Preserve binary identity before processing and
checkpoint per completed case. No merge or release is authorized.

## Candidate A, frozen before evaluation

The original snippet miss reproduces on both exported native baselines;
its ten-second WAV hash exactly matches E007. An offline vote snapshot
using the actual exported fingerprints shows a winning neighbourhood
combining distinct offsets: for anchor-frame cutoff 1600 its mean offset
is about -954 frames while the known original offset is -861.33 frames;
the vote median is about -1027. A centre with one own vote combines large
clusters in neighbouring offset cells. This is a multimodal fit, not a
reason to lower the verifier threshold.

Candidate A: preserve the neighbourhood search and evidence score, and
optionally estimate shift/tempo/offset from its most occupied cell. Add an
opt-in `--modal-fit`; default behavior and existing public method semantics
stay intact. A constructed two-offset regression checks that the original
mean predicts an unsupported position while the modal estimate aligns a
real correspondence. Preserve exact coherent-mode behavior through rebase
and expiry. This candidate might select a repeated passage at the wrong
position; measure position errors and false starts, not identity alone.

`experiments/recognition-holdout.json` freezes eight new Kevin MacLeod
recordings and eight separate electronic-music negatives before any of
their detector outputs. Original midpoint clips stay fixed. They are a
small recording-disjoint test, not proof across all genres or artists.
Sources and CC BY 4.0 attribution come from Incompetech's publisher
catalogue. No held-out result may be used to choose the candidate.
