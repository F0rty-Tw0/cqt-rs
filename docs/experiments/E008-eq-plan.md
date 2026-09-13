# E008: EQ robustness (planned, not executed)

The user asked during E007 whether EQ-distorted songs can be matched
confidently. Current answer: moderate EQ tolerance is plausible; a reliable
success rate for this implementation is not yet established.

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
