# E008: EQ robustness (measured in E009)

Executed on 2026-09-13 under the frozen E009 protocol. The opt-in modal
candidate detects 22/22 development snippets and 8/8 held-out snippets under
each of twelve separately applied EQ filters. Baseline development detection
ranges from 19/22 to 22/22; held-out detection is already 8/8 throughout.
No new held-out EQ gain is demonstrated. EQ-negative controls produce no
starts. [Results and filter-specific peak/hash measurements](E009-results.md)
and [proof](../evidence/E009/README.md) preserve all outcomes.

This supports tolerance of the tested shelf, notch and band-limiting settings.
It does not guarantee arbitrary EQ, combined EQ-plus-speech treatments or
clipping, nor calibrate the confidence score as a probability. The original
pre-execution rationale and plan follow.

The user asked during E007 whether EQ-distorted songs can be matched
confidently. Before this execution, EQ tolerance was an architectural
expectation without a measured success rate for this implementation.

`PeakPicker` selects dB maxima relative to a local mean. Uniform gain
cancels from that prominence calculation, except at the absolute floor.
Smooth EQ can approximately preserve local peak positions; sharp filter
edges, deep notches and clipping can move, remove or add them. Hashes use
peak locations rather than their amplitudes. This is an architectural
expectation, not a measured guarantee. Wang's
[fingerprinting paper](https://www.ee.columbia.edu/~dpwe/papers/Wang03-shazam.pdf)
discusses the EQ tolerance of spectral peak coordinates and the limitation
near sharp filter transitions. Its results are not cqt-rs benchmark results.

The older development generator includes a 120 Hz high-pass combined with
FM processing and MP3. That single compound treatment is not an EQ sweep,
and is not the current 22-track experiment.

Next bounded experiment: retain clean references and frozen thresholds;
use independently known original-clip positions, then test bass/treble
shelves at +/-6 and +/-12 dB, broad and narrow notches, high-pass and
low-pass filtering. Keep nonlinear clipping as a separate treatment. Test
EQ alone before combining it with pitch/tempo/voiceover, and add unwatched
music from the same genres to the negative controls. Freeze frequencies,
Q, gains, filter implementations and scoring windows before execution.
Measure track recall, onset delay, false starts, peak survival and matched
hashes; preserve every miss. Confidence is an evidence score, not a
calibrated probability. Reusing E007 recordings expands its stress grid
but does not supply an independent corpus or calibrate a universal score.
