# E011: independent chunks versus continuous song references

Status: execution verified; blanket preference for full-song indexing rejected.
All 192 native runs and raw-result audit completed. See [results](E011-results.md).
Protocol recorded before native evaluation on 2026-09-13.

## Question and source correction

Does the existing matcher recognize ten-second queries more reliably when
each complete recording is one reference with continuous timestamps,
compared with independently fingerprinted ten-second chunks?

The E010 input audit found that the publisher's download URL for t05,
Roots and Shoots by Dave Kent, supplies an MP3 tagged Exodus (original mix)
by Marc Burt. The publisher's release page lists Roots and Shoots as 5:50;
the pinned wrong download is 8:33. Obtain the recording from the Internet
Archive release embedded by the publisher, audit tags and hashes, and
independently align it before freezing the new mix queries. Preserve E010
inputs/results unchanged. Both E011 database layouts use the same corrected
22-song catalogue, so their paired comparison changes reference layout only.
Do not attribute a difference from E010 to layout alone.

Sources: <https://www.toucanmusic.com/mixes/tou2020>,
<https://www.toucanmusic.com/releases/tou285>,
<https://archive.org/metadata/tou285>.

## Frozen comparison and queries

Use the same binary as E010, source
`bace1a92feb7d9e862c21d295fd84df6a6df6626`, SHA-256
`d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`.
No native implementation or threshold change; modal fit remains disabled.
The work starts at PR #4 commit `e7a63f9e721f040e65f781752afab4eeea85a0ad`.
PR #3 remains the separate fixed research baseline.

Compare all non-overlapping ten-second chunks, including unpadded tails,
against all 22 complete originals. All queries search the entire database.
Continuous references also change the per-song repeated-hash cap scope and
fingerprint boundary context. This comparison does not isolate those
mechanisms individually or establish a native chunk-aggregation fix.

Freeze the following query sets before any native E011 outcomes are opened:

- Exact: one complete chunk per song, choosing the middle complete chunk.
- Random clean: one sample-uniform ten-second cut per song, drawn from the
  complete source's legal start positions. Do not discard silence or misses.
- Frozen mix: preserve all 22 E010 query bytes and timestamps. Reassess t05's
  label using only the corrected recording's independent STFT alignment.
- Random mix: one sample-uniform ten-second cut per song from the union of
  independently supported anchor intervals. A whole query must fit an
  interval; overlapping intervals must not get extra sampling weight.
  If a cue is unsupported, retain that annotation gap explicitly.
- Negative music: the eight already pinned E009 nonmatching midpoint clips,
  ten seconds each. These are known development negatives, not a fresh split.

Use separate deterministic Python Random streams seeded by the literal
strings `E011:20260913:clean:<song>` and `E011:20260913:mix:<song>`.
Record actual sample positions, selection intervals, audio hashes and
annotation provenance. Mix labels are algorithmic, not human-verified;
random cuts from this known mix are not recording-disjoint evaluation.

The corrected t05 long-template annotation has one strong correspondence
near 791 seconds but lacks two consistent accepted anchors. Before native
evaluation, use E010's existing fallback: 20-second templates every ten
source seconds within publisher-order mix interval [700,1000], retaining
the same similarity and multi-anchor consistency requirements. Save both
attempts; do not weaken the annotation threshold.

## Scoring and acceptance

Each query uses a fresh native process and ten seconds of mono 44.1 kHz
PCM. Score accepted `start` events, not report candidates. Choose the
strongest accepted start, then earliest consumed time and lexical reference
ID; map chunks to parent songs and global positions as in E010. Do not sum
chunk scores. Retain every wrong-parent and repeated-parent accepted start.
For negatives, any accepted start is a false acceptance.

Report correct/wrong/no-match and paired gains/losses separately for every
query set. Target 22/22 for each clean control; every miss requires a saved
diagnostic and remains in the denominator. Exploratory preference for full
references requires more correct supported mix queries, no lost correct
clean or mix queries, and no added wrong identities or negative starts.
If this rule fails, reject a blanket recommendation and retain all losses.
Inspect candidate evidence and alignment on failures; label explanations
suspected unless separately reproduced. Do not weaken thresholds to pass.

## Execution and budget

Up to 192 native runs (96 queries times two layouts), at most four processes
concurrently, each with RAYON_NUM_THREADS=1 and a 600-second timeout. Alternate
layout order per query. Allow 90 minutes including preparation and bounded
diagnostics, then checkpoint unfinished gates. Capture stdout/stderr before
atomic file writes. Persist source/binary/evaluator identities before runs
and one result after every completed run. Concurrent wall times are not a
speed benchmark; index bytes retain the native estimate's scope.

Commands will be retained with the frozen manifest and native run metadata.
Verify all input/output hashes, exact query durations, complete reference
sets, scorer regressions and independent raw-result recount before publishing.
Update PR #4 with the result and exact next action. No merge or release.

## Reproduction

Prepare E010's pinned originals, chunks, binary and independent annotation
features using its [reproduction instructions](E010-chunk-queries.md).
Keep its old source manifest intact; this experiment writes a separate
`target/song-index` corpus and corrects t05 there. Install Python 3.12 with
NumPy, SciPy and Matplotlib, and FFmpeg/FFprobe. The recorded run uses the
same environment and binary as E010.

```sh
python3 scripts/song_index_prepare.py
python3 scripts/song_index_eval.py --label default-a --workers 4
python3 scripts/song_index_report.py --label default-a
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

Preparation refuses to overwrite a frozen manifest and evaluation refuses
to overwrite a run directory. Native calls pass only `--watch id=path` for
every reference and `--stream query.wav`; expected query labels are never
passed to the detector. Preserve the frozen manifest's sample positions
and hashes when comparing machines or rerunning this experiment.

The initial preparation attempt failed its long-template t05 annotation
gate, then the bounded short-template fallback succeeded. Before any
native E011 execution, preparation also corrected an overly strict support
test accidentally applied to inherited t01: its old query lies between
consistent anchors. The original manifest is retained as
`manifest-pre-native-v1.json`; the final manifest preserves E010 support
for existing labels and uses the corrected source to establish t05. The
stricter within-anchor rule applies to the newly sampled random mix cuts.

## Post-outcome cue diagnostic

The frozen random t01 query starts at 59.594195 seconds. Both native layouts
accept t02, while the query's predeclared expected identity is t01. Preserve
this as a wrong top identity under the frozen single-label protocol. Check
possible overlapping audio using the separate STFT aligner: both original
recordings, 20-second templates every ten seconds, mix interval [0,120],
the existing tempo/pitch grid and unchanged consistency/similarity rules.
This is a post-outcome annotation diagnostic; it must not silently relabel
the query, drop it, or be described as a fresh independent test.
