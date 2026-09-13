# E014 evidence

[Results](../../experiments/E014-results.md) and
[provenance/reproduction](../../experiments/E014-reproduction.md).

231 scored cases, 233 completed native invocations including two integrity
replays. `accepted-events.jsonl` retains every start/end/done event, truth,
and raw-output hash. `runs.json.gz` retains commands, binary/input/evaluator
hashes, source commits, exits and elapsed times. `audit.json.gz` contains the
independently recounted outcomes, per-play errors and paired comparisons.
`manifest.json.gz` and `robustness.json.gz` contain complete input/transform
provenance. Gzip files decompress to the exact local JSON exports.
`integrity-replay.json` identifies original truncated logs and replacements.

The initial bulk upload stalled and a smaller publication was blocked by
approval review. The user subsequently explicitly authorized the diagnostic,
PR update and committing all findings. This publication uses compressed numeric
proofs and the complete raw archive supplied with the experiment deliverable.

`E014-evidence.zip` is supplied with the experiment deliverable. It contains
880 members, raw JSONL/stderr for all scored runs, the original damaged logs,
executed evaluator, recovery scripts, toolchain and input provenance; no audio
or executable bytes. ZIP CRC and all member SHA-256 checks passed.
Archive SHA-256: `31b842358e59850fa7b562871e517c594ae51d87af48651c2b34a59f675a5efb`.
Archive size: 15888763 bytes. See `evidence-archive.json`.

Defaults are unchanged. The default-switch quality gate is rejected. Finite
22/22 treatment results are not universal matching accuracy. The native source
identified in `native-ci.json` passed all 12 checks; final publication-head
checks are separately available on PR #4.
