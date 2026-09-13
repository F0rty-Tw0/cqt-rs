# E014 provenance and reproduction

The [frozen protocol](E014-long-context.md) was published at
`890a7181a3d107e8a34b0582caf449355fccbd9c` before implementation or outcomes.
See [measured results](E014-results.md) and [evidence inventory](../evidence/E014/README.md).

## Exact native source and checks

Evaluated source: `748dcd0e4ed4d1431a39a6878e99f1ee7075ccae`.
Executable SHA-256:
`1b1c5968ce134bda1b559da9b8bdade829c30070c4960700da6015988c0ed9d7`.
Downloaded binary archive SHA-256:
`5a77001516a12a06cd1eac99af8b584336878f681bf43e4840ca8b54f6063e24`.
All 12 checks passed on that exact source:
[CI](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784380994),
[CLI proof](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784381011),
[verifier proof](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784381033),
[binary build](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34784381000).
Eight new Rust behavioral tests pass. Native source is unchanged by the final
report/evaluator corrections. Final publication-head checks are tracked separately
in PR #4; the evaluated-source links above do not certify later commits.

Baseline source `36098740e78556e41e3e4bbd477508f191331d21` was exported and
compared against the candidate's legacy mode on the identical clean query and
25-play programme. Accepted semantic output is unchanged. Historical E012
modal query outputs are recounted from checked-in `docs/evidence/E011/modal-*.zip`,
with query PCM and raw-output hashes checked. E013 raw archives are unavailable
here; its reported figures are historical context, not a newly verified paired
comparison. The long arm changes both retrieval length and reporting cadence
relative to legacy; that comparison does not isolate window length alone.

## Reproduce fresh native runs

Use the repository root, Python 3.12 with NumPy/SciPy, ffmpeg including rubberband
and flite, and the Rust toolchain/lockfile recorded in the evidence directory.
Recover the exact source recordings, then verify every decoded reference and
query against the archived hashes; changed bytes block the comparison.

```sh
python3 scripts/continuation_inputs.py
python3 scripts/continuation_eval.py prepare
python3 scripts/continuation_eval.py prepare-robustness
cargo build --release --package cqt-monitor
mkdir -p target/continuation/candidate-v3
cp target/release/monitor target/continuation/candidate-v3/candidate
git rev-parse HEAD > target/continuation/candidate-v3/candidate-commit.txt
python3 scripts/continuation_eval.py run --stage queries
python3 scripts/continuation_eval.py run --stage programme
python3 scripts/continuation_eval.py run --stage full-mix
python3 scripts/continuation_eval.py run --stage robustness
python3 scripts/continuation_eval.py run --stage parity
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/continuation_report.py
python3 scripts/continuation_publish.py
python3 scripts/continuation_package.py
```

Parity additionally requires the baseline executable and commit marker under
`target/continuation/baseline/`; build it in a separate checkout at the pinned
baseline. Do not reuse outputs from a different binary, evaluator, configuration
or reference set. The original commands and identities are retained per run;
a fresh build has its own binary identity. `continuation_report.py` pairs native
start/end events independently of the runner's controlled-programme scorer,
asserts the complete 231-case set and recounts all outcomes. Descriptive paired
bootstrap intervals are in `audit.json`; known overlapping development material
does not support population accuracy inference.

## Execution and integrity deviations

There are 231 scored cases and two additional exact-command integrity replays,
233 completed native invocations in total. No quality thresholds, query labels,
transforms, or native source changed after outcomes were inspected. The replay
runs exceed the original 192-primary-query invocation budget by two; they repair
evidence integrity, not a quality failure. No speed claim is made. The
[resource-only scheduling amendment](E014-scheduling.md) was recorded before
robustness outcomes and kept total native concurrency at six with one Rayon
thread per process. Per-run elapsed times include indexing and contention.

Initial decoded t19/t22 files were incomplete. Strict preparation rejected them;
atomic decoding regenerated the exact frozen hashes before evaluation. Commands
and old/new hashes are in `decoded-repair.json`.

The initial evaluator could not resume its own cache because JSON converts
reference-signature tuples to arrays. `continuation_resume.py` normalized only
that representation before invoking the original strict identity guard, enabling
parity to finish. The executed evaluator snapshot and its hash remain in the raw
archive. The current evaluator fixes the guard; a regression verifies unchanged
identities pass while changed reference/evaluator hashes remain rejected.

The independent recount then detected truncated stdout files for
`clean-t21-continuation` and `mix-t21-continuation`. The cause of truncation was
not established. Original files and records were preserved under
`integrity-replay/`, and the exact recorded binary, input, configuration and
frozen evaluator were replayed. Both replays have complete hash-verified outputs
and the same recorded outcomes (correct and no-match). Their full stdout hashes
are different from the original recorded hashes; byte equivalence is not claimed.
The final audit uses the replay logs. Stage summaries still retain original
records; `runs/*.result.json` plus `integrity-replay.json` identify the final
scored evidence. No hash check was disabled or original record rewritten.

## Interpretation and next bounded question

The continuation arm retains 22/22 on each tested pitch, tempo, EQ, gain and
10 dB noise cell, and removes the controlled programme's three duplicate starts.
It does not meet the requested goal: 0 dB noise is 8/22, synthetic voiceover is
16/22 (long: 17), combined treatment is 17/22 (long: 19), and random mix queries
are 16/22 (legacy: 17). Full-mix coverage remains 21/22, missing t10, with more
segments and fewer frozen/random locations covered than the long arm. Keeping
three candidate trajectories adds segments relative to one without recovering
another song. Two-second verification does not mean two-second notification;
programme median start notification is 6.429 seconds. Exact identity can still
point about 188 seconds away in a repeated source passage.

The next useful experiment is to measure which fingerprints and fresh peak
correspondences survive noise, real speech and track overlap, then freeze a
single retrieval or verification change before another evaluation. Obtain a
recording-disjoint set with human-annotated active intervals for any general
recognition claim. Do not tune thresholds on the E014 failures and describe the
same data as held out. E014 is complete and rejected for a default switch;
the broader goal remains open. No merge or release.
