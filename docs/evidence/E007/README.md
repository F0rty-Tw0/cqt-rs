# E007 evidence: first execution and cancellation repair

The [findings](../../experiments/E007-results.md) report completed recognition
observations with explicit proof gaps. The full acceptance gate is blocked;
the partial workflow is not a passing run.

- [Original CI run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34755314457)
- [Job log](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34755314457/job/103718583013)
- [Original artifact, including raw JSONL and listening samples](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34755314457/artifacts/10316868225)
- ZIP SHA-256: `1c9528f87339f10f478f94d6daa34f3b47326f9c5ddf66fb0a3ad6e2e926f64e`
- ZIP size: 7,557,215 bytes. Scheduled artifact expiry: 2026-12-12.
- Harness: `64bc53d448697f860586139bb256c9b1764e982b`.
- Baseline: `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a`.
- Candidate: `176bce2fef1d1463ff7b815f4f0ee62a21deb435`.

`results.json` retains the 13 completed run summaries, every start,
per-track misses/first starts/maxima, commands, wall times and raw hashes.
`validation.json` records the independently recalculated counts and six
identical event-parity pairs, 26 checked raw-output hashes, the partial
candidate output hash and all 49 original artifact file identities.
`first-run.log` preserves build, download, transformation, recognition and
cancellation logs. Full raw JSONL is in the linked artifact, not duplicated
as tens of megabytes of repository text. If it expires, regenerate the
experiment before using raw-event claims for a new acceptance decision.

`references.json`, `streams.json`, `downloads.json`, `voiceover.json`,
`self-truth.json` and `preparation-commands.json` preserve source and audio
identities, fixed excerpt positions, exact speech/RMS spans and transforms.
There are 202 spoken blocks. The maximum recorded RMS mismatch is
1.39e-17; all common headroom scales are 1.0. These are harness-recorded
measurements. `source-identity-check.json` independently verifies the mix
against both the CI SHA-256 and the
[Internet Archive source metadata](https://archive.org/metadata/tou2020).
MP3 container duration and decoded PCM duration differ by approximately
0.04 seconds; scoring uses the decoded PCM length.

`evaluator-at-run.py` is the exact executed harness, copied from the pinned
commit. Its SHA-256 is recorded in `artifact.json`; it differs from the
repaired current harness. The original `metadata.json` is absent: Python
was killed before its finally block, losing the in-memory binary hashes.
Do not create a replacement document claiming those identities are known.

The normal [nine-check correctness run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34755314423)
passed for the original harness commit. `checkpoint-baseline.log` and
`checkpoint-candidate.log` reproduce the metadata loss with a hard SIGKILL
and validate the repair, respectively. `checkpoint-results.json` preserves
commands and expected failure/pass exits. This tests proof persistence,
not recognition quality. The fix writes identities before expensive work
and replaces JSON checkpoints atomically. It does not recover lost hashes.

## Review the original evidence

Download the ZIP from the artifact link, verify its SHA-256 above and
extract it to `target/real-mix-first`. Then, from the repository root:

```sh
python3 scripts/report_real_mix.py target/real-mix-first --allow-incomplete --report docs/experiments/E007-results.md --validation docs/evidence/E007/validation.json
python3 -m unittest discover -s scripts -p 'test_real_mix_checkpoint.py' -v
```

The reporter is specific to this first run and deliberately requires
`--allow-incomplete`. It does not mark the partial candidate output as a
completed run, and it does not infer results for the two unstarted cases.

## Repeat all frozen cases with the repaired harness

Use a Rust-capable Linux host with Rust 1.98.1, Python/NumPy, FFmpeg with
rubberband/flite, curl, and sufficient scratch space for several GB of WAV.
Keep the original Cargo.lock from this directory. Build in fresh worktrees:

```sh
git worktree add --detach target/e007-baseline 7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a
git worktree add --detach target/e007-candidate 176bce2fef1d1463ff7b815f4f0ee62a21deb435
cp docs/evidence/E007/Cargo.lock.txt target/e007-baseline/Cargo.lock
cp docs/evidence/E007/Cargo.lock.txt target/e007-candidate/Cargo.lock
cargo +1.98.1 build --release --locked -p cqt-monitor --manifest-path target/e007-baseline/Cargo.toml --target-dir target/e007-baseline-build
cargo +1.98.1 build --release --locked -p cqt-monitor --manifest-path target/e007-candidate/Cargo.toml --target-dir target/e007-candidate-build
RAYON_NUM_THREADS=1 python3 scripts/real_mix_eval.py --baseline target/e007-baseline-build/release/monitor --candidate target/e007-candidate-build/release/monitor --work target/real-mix-repeat
```

The workflow now has a 60-minute evaluation-step limit within a 65-minute
job and runs manually. A rerun has not been executed. On a fresh run, retain
source/binary identities and confirm every source hash still matches the
frozen first-run manifest before treating results as paired with it. Source
URLs can change over time. Preserve failures; never move the excerpts or
thresholds to make the test pass.

Audio attribution and CC BY-NC-SA 4.0 terms are in `ATTRIBUTION.txt`.
