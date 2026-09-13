# E009 proof, attribution and reproduction

[Measured report](../../experiments/E009-results.md),
[frozen protocol](../../experiments/E009-recognition-alignment.md),
[independent validation](validation.json) and [per-track outcomes](per-track.json).
This is a completed, bounded recognition experiment. The broader accuracy,
latency and performance goal remains incomplete; the candidate is opt-in.

## Source and executable identities

| Side | Source commit | Executable SHA-256 |
| --- | --- | --- |
| PR #3 | `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a` | `3a4ecba589371fee42cc1d8834acdb6d4c7fe4fc085ff5d7663b111ccd296389` |
| PR #7 behavior, PR #8 harness | `cdf428e9d6a82127cf36aedd8ddea27f82de81ec` | `253ddcf4ec517bbbf3b25eb9c5c948044d985cbcf1160285e03ddc42d28c2866` |
| Modal candidate | `69c09c4be5fb391a9e3d93ff4465cc4864d0ddf4` | `d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc` |

Build: Ubuntu 22.04 GitHub runner, Rust 1.98.1, release, preserved E007
Cargo.lock. [Native binary build](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759390428),
artifact `10318287547`, ZIP SHA-256
`c80ae73616970baa67a6441042c8676bb57813b48d678152e8b88ae2a00ef683`.
The workflow retains binaries for 90 days; source, lockfile and workflow remain
in Git. Exact executable identity may differ on a different build host.
Compiler and linked-library records are alongside this file; build source/dirty-state records remain in the local evidence.

Native evaluation ran in the Linux chat execution environment, with
`RAYON_NUM_THREADS=1`; metadata records Python, NumPy, FFmpeg and platform.
Local Cargo was unavailable, so the verified exported binaries were executed
directly. CPU model and process CPU measurements are unavailable. Single wall
times, some overlapping other preparation work, do not establish a speedup.

## Published reproduction record

The user requested a lightweight reproduction guide instead of the large raw
archive. See [the complete method and commands](../../experiments/E009-reproduce.md)
and [source URLs, cut positions and SHA-256 manifest](source-manifest.json).
Per-track detection outcomes and the local validation summary are published here.
Raw logs, fingerprints, detailed input manifests and replay records remain in the
execution workspace; they are **not uploaded to this PR**. The local archive
was 6,810,916 bytes, SHA-256
`4827bbe994355091c883665e7b2cc11aaf610e75e4e0db48f4b947c010ecc1e5`.
Automatic approval review initially rejected its upload; the user then chose
this reproduction-focused deliverable. No archive upload is required to run the
experiment again.

The independent local audit rehashed inputs/binaries and 324 selected output
files, recomputed recognition, delay and position metrics, checked 44 baseline
parity pairs and default behavior, and recomputed 390 fingerprint comparisons.
These describe completed local validation; a reproduction recipe alone is not a
publicly downloadable raw-output proof. Full validation at the original paths:

```sh
python3 scripts/validate_recognition_lab.py --label modal-a --include-mix
```

Fresh runs write their own absolute paths and identities. Use the reproduction
guide to generate new evidence; do not change preserved hashes to make differing
source media pass.

## Failures retained

- Initial candidate `8346eb9` failed the monitor format gate; two formatting
  changes fixed it at `69c09c4`. All nine normal checks passed on that exact
  [candidate run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759390406).
  The two new Rust tests cover competing offsets and coherent-mode behavior
  across rebase/expiry. Python truth-scoring regressions cover an early false
  start, later valid start, duplicate, miss and unwatched music.
- A separate [CLI proof run](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759390405)
  failed before detection because a restored cache already contained its
  worktree directory. Commit `2ccb2f6` moves that source worktree to runner
  temporary storage. [Normal CI](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759946604),
  [CLI before/after proof](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759946614)
  and [binary export](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759946574)
  passed at `2ccb2f6`. The audio results above belong to the `69c09c4` binary;
  the later change is workflow-only. Final documentation-head checks are linked
  in [PR #9](https://github.com/F0rty-Tw0/cqt-rs/pull/9).
- Final hash review found nine truncated auxiliary audio caches, two empty
  fingerprint dumps and eight truncated native output logs. Cause unknown.
  All reference and assembled detector-input hashes remained correct. The nine
  caches and two dumps were reconstructed only after exact original-hash
  agreement. Eight exact native commands were replayed into separate files;
  all non-timing events matched their original digests. Original damaged logs,
  original expected records, observed mismatches, reconstruction commands and
  replay metadata are preserved locally. The selected matrix contains 162 auditable
  executions, including those eight replays, not 170 independent cases.
- E007's missing binary identities and interrupted full-reference diagnostics
  remain unresolved historical limitations. This new ten-second-reference run
  does not retroactively fill them. The modal estimate can still choose a
  repeated passage with an approximately 8.7-second position error.

## Sources and scope

Development: the 22 publisher-listed tracks in
[Toucan Music 2005 to 2020](https://www.toucanmusic.com/mixes/tou2020),
CC BY-NC-SA 4.0. Track/artist attribution is in
[E007 attribution](../E007/ATTRIBUTION.txt) and the source manifest alongside this file.
The mix hash is
`39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103`.
All 22 original ten-second midpoint references and the self-control stream
match E007 byte-for-byte. This is a fixed previously studied mix, not a random
sample of all broadcasts. The publisher supplies order, not independent cue
times; extra mix starts cannot be labelled correct or false from that alone.

Held-out: Cipher, Electrodoodle, Cut and Run, Wallpaper, Funkorama, Carefree,
Hyperfun and Sneaky Snitch, all by Kevin MacLeod. Separate unwatched negatives:
Cruising for Goblins, Go Cart, Jerry Five, Raving Energy (faster), Reformat,
Shiny Tech II, Pamgaea and Blippy Trance, also Kevin MacLeod.
Source URLs and ISRCs were frozen in
[recognition-holdout.json](../../../experiments/recognition-holdout.json).
[Publisher licensing](https://incompetech.com/music/royalty-free/licenses/):
CC BY 4.0; music was excerpted, filtered and sometimes mixed with generated
speech for this experiment. The published `source-manifest.json` records every download
and decoded/reference hash. These eight held-out recordings are disjoint
from development but cover only one additional artist.

## Repeating the experiment

Follow [E009-reproduce.md](../../experiments/E009-reproduce.md) for source
selection, sample-accurate slicing, environment, native builds, every treatment,
scoring and the exact commands. That guide requires no uploaded audio archive.
