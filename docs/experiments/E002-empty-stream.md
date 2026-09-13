# E002: valid monitor JSON on empty streams

Status: running; real-binary CI proof pending.

First executing proof (candidate `6cfd2e2`) failed as intended on an
unexpected additional defect: [job 103713828212](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34753483349/job/103713828212)
captured `"event":"stream","file":"-","seconds":nu`. The baseline applies
`{:.2}` to an already-formatted String, truncating it to two characters.
The original ratio-only fix did not address this and was not accepted.
The revised candidate fixes both observed formatting defects. The proof
explicitly requires each known baseline defect; arbitrary invalid output
still fails. Added 12.34 s and 123.45 s WAV cases catch parseable but wrong
duration numbers, as well as malformed JSON on shorter WAV/live input.

Baseline: PR #3, `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`.
Candidate branch: `codex/monitor-empty-stream-json`, based on PR #5.

Hypothesis and predeclared acceptance rule: zero consumed samples cause
the `done.realtime_fraction` calculation to emit a non-JSON infinity/NaN.
The same real-binary reproducer must exhibit that exact baseline defect
and accept every candidate event as strict JSON. Nonempty inputs retain a
finite numeric ratio. All non-timing events must match the baseline.

Implementation: emit `null` when the actual consumed-sample count is zero.
Use the sample count rather than the rounded `audio_seconds` field: a
one-sample stream is nonempty even though its displayed duration is 0.00.
The legacy `cpu_seconds` field still reports wall time; this change does
not claim to fix process CPU accounting or improve recognition/throughput.

Cases: empty stdin, empty WAV, one-sample stdin, one-sample WAV, 2053-sample
stdin (partial block), and 1 s, 12.34 s and 123.45 s WAVs. Fixtures contain deterministic
mono s16 silence at 44100 Hz. Both binaries use the same reference WAV,
fixture inputs, Rust 1.98 toolchain and copied Cargo.lock.

Proof command after successful builds:

```sh
python3 scripts/prove_monitor_cli.py --baseline target/proof-baseline-build/release/monitor --candidate target/release/monitor
```

The `Monitor CLI before-after proof` workflow captures stdout/stderr,
exit codes, executable/input/verifier hashes, source revisions, toolchain
and dependency lockfile in an artifact retained for 90 days. Its parser
accepts only the predicted baseline failure, not build errors or arbitrary
invalid JSON. Preserve verified results in this record after inspecting
the actual logs and artifact. Candidate-only edge cases also run in the
normal monitor CI gate, so this becomes an ongoing regression check.

Budget: one two-build CI proof plus normal correctness gates, with a
10-minute proof-job timeout. Re-run only to debug a concrete failure.
