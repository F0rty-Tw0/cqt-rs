# E010 evidence: 22 DJ-mix clips and complete ten-second chunk coverage

[Results](../../experiments/E010-results.md) ·
[Protocol/reproduction](../../experiments/E010-chunk-queries.md)

All 44 native invocations completed. There are 22 actual ten-second mix
queries, 21 algorithmically supported labels, and one unverified label
(t05, Roots and Shoots). Supported correct identities improve 2/21 to
13/21, with 11 gains, no losses and no accepted wrong-song starts.
The t05 provisional query yields no match on either side and is excluded
from the accuracy denominator. Full 22-labelled-query acceptance is blocked.

## Files and identities

The top-level files contain the result, query records, database metadata
and native logs. `inputs-and-annotations.zip` contains the source manifest,
full annotation maps, build/runtime records and integrity incident evidence
listed below; names inside that archive are relative paths with an internal
`SHA256SUMS`. This keeps the PR diff focused while preserving full proof.

- `database.json`: 22 parent-song metadata records with 880 chunk records,
  exact source offsets/sample counts, PCM identities and generated native
  landmark-hash/peak counts. There are 858 full sections plus 22 unpadded
  tails. The native in-memory index is rebuilt by each invocation; these
  metadata are not a serialized index cache.
- `sources.json` (in the input archive), `queries.json`: frozen source/chunk/query identities,
  exact mix cuts and per-query annotation status. Download original audio
  from the URLs in the source manifest; no audio is included in Git.
- `metadata.json`, `summary.json`: actual executable/evaluator/input hashes,
  run status, independently replayed outcomes, checks, paired results and
  descriptive uncertainty. Parent-song aggregation retains every start.
- `native-runs.zip`: all 44 raw stdout/stderr pairs, full results with exact
  argument lists and input identities, per-run checkpoint metadata, and
  the separately labelled native self-query preflight. ZIP SHA-256:
  `a7cbd718a0d778872c0a780ba639a0c77a62ca505c3c375263119d0ff05fe3a2`.
- `alignment-centered` and `alignment-centered-short` (in the input archive): final STFT anchor
  maps and parameters. This annotator never invokes the CQT matcher.
- `annotation-attempts.zip` (in the input archive): rejected preliminary annotation attempts and
  available preparation logs. A log was found incomplete despite the
  complete source checkpoint; acceptance uses file hashes and sample counts,
  not the progress log's line count.
- `identity-incident.json`, `t19-restoration.json`,
  `annotation-feature-audit.json` (in the input archive): four subsequently truncated decoded
  caches, exact-hash restoration, and independent regeneration of all 23
  final feature arrays. All arrays compare equal. All native chunk and
  query inputs retain their frozen hashes.
- `python-tests.*`: all 14 Python regression tests pass, including four new
  cases for label-independent scoring, chunk aggregation, deterministic
  ties, ignored diagnostic reports, and unknown reference rejection.
- `environment.json`, `rustc.txt`, `Cargo.lock.txt`, `linked-libraries.txt`,
  `candidate-commit.txt`, `candidate-status.txt`, `binaries.sha256`: local
  runtime and the exported build metadata (in the input archive). Rust/linked-library records
  describe the CI build; local platform/Python/FFmpeg records describe the
  evaluated host. CPU model is unavailable because `/proc` is not mounted.
- `SHA256SUMS`: content identities of the retained evidence files.

Both treatments use candidate source
`bace1a92feb7d9e862c21d295fd84df6a6df6626`, binary SHA-256
`d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`,
default gates and modal fit disabled. The
[binary export run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34767444533)
contains artifact 10320783776; downloaded archive SHA-256
`d06acaccaf717e83f858f3e5a1568dad3f101d771b4ab43bb4bf32163220991a`
matched the GitHub artifact digest. PR #3's fixed baseline remains intact.

## Proof review and limitations

Recomputed every prediction from its original raw events; checked all 88
stdout/stderr hashes, terminal `done` events, ten-second durations, reference
counts and identities, and paired query equality. Rechecked all 880 chunk
hashes, all midpoint/query hashes and the actual binary. The 23 final STFT
arrays reproduce byte-for-byte from hash-verified sources, and both short
annotation JSON files replay exactly. Corpus corruption did not alter the
accepted native inputs or final annotation features.

The query labels are algorithmic, not human-verified cue times. These are
digital mix cuts, not microphone captures. This known-mix experiment adds
no independent negative exposure, speech/EQ tests, or new recording-disjoint
corpus. Index memory is an approximate native estimate, not peak RSS; single
elapsed times and overlapping audit work are not a speed benchmark. The
per-chunk repeated-hash cap differs from a full-track grouped index.

Keep t05 unscored until its cue/version is independently verified. The eight
supported misses remain visible. This evidence supports a bounded coverage
finding and completed execution; the broader research goal remains incomplete.
