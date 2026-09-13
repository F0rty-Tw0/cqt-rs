# E011/E012 evidence: ten-second queries, complete references and modal fitting

All **288 main native runs and four matched-source controls completed**.
The standard-library archive recount independently reproduces all main
outcomes. The four adaptive controls were separately recounted from their
raw events. Every query is ten seconds; all source/binary/input/output
identities and complete native commands are retained.

| Correct identities | Independent chunks | Full songs, default | Full songs, modal |
| --- | ---: | ---: | ---: |
| Exact database clips | 22/22 | 20/22 | 22/22 |
| Random clean source clips | 22/22 | 22/22 | 22/22 |
| Frozen mix clips | 14/22 | 15/22 | 16/22 |
| Random mix clips | 15/22 | 16/22 | 17/22 |
| Known negative clips rejected | 8/8 | 8/8 | 8/8 |

The random set has one wrong top identity under its frozen t01 label in
every configuration. A post-outcome independent STFT diagnostic supports
both t01 and t02 near that timestamp, compatible with overlap or cue
ambiguity. No relabeling or exclusion is applied. Modal fitting also adds
a wrong identity on the older frozen t01 query. Its no-added-error gate
fails, so the option remains experimental and disabled by default.

The four remaining random modal no-matches are t04, t10, t13 and t22. All
four approximately corresponding clean-source passages are recognized at
the same settings. This supports investigating fingerprint survival in the
mix; it does not isolate an individual audio effect.

## Source correction

The publisher's t05 download link supplies **Exodus (original mix), Marc
Burt**, while labelled Roots and Shoots by Dave Kent. E011 corrects that
recording using the Internet Archive release embedded by the publisher.
The correct file's archived SHA-1, tags, SHA-256 and decoded PCM identity
are retained. Five consistent independent STFT anchors place it around
791–852 seconds, supporting the old 835-second query.

- [Publisher mix](https://www.toucanmusic.com/mixes/tou2020)
- [Publisher release](https://www.toucanmusic.com/releases/tou285)
- [Archive release](https://archive.org/details/tou285)
- [Attribution and source URLs](attribution.txt)

Both new reference layouts use the same corrected 22-song catalogue
(864 independent chunks or 22 full songs). E010's old corpus/results remain
unchanged, and differences from E010 cannot be assigned to layout alone.
The t22 publisher/MP3 remix-name discrepancy is retained in the tag audit;
the experiments use the frozen audio supported by independent alignment.

## Files and verification

- [E011 summary](summary.json), [E012 summary](modal-summary.json), and
  [independent raw recount](independent-recount.json).
- [Main archive inventory](archives.json): per-file SHA-256, size and member
  counts. Every ZIP also contains its own member SHA256SUMS.
- [Inputs and provenance](inputs-and-provenance.zip): corrected catalogue,
  96 frozen query records, exact sample positions/seeds, source tag audit,
  annotation attempts, initial invalidated manifest and its exact preparer
  source, overlap diagnostic, environment and native build provenance.
- `default-{exact,clean,frozen,mix,negative}.zip`: 192 native runs, each with
  raw stdout/stderr and a result record containing the complete command.
- `modal-{exact,clean,frozen,mix,negative}.zip`: 96 paired candidate runs.
- [Matched-source archive](matched-source-diagnostics.zip),
  [its inventory](diagnostic-archives.json) and
  [summary](matched-source-summary.json): four additional adaptive controls.
- [Python tests](python-tests.txt) and [test provenance](python-tests.json):
  all 18 tests pass, including four new source/scoring/input regressions.

Native executable for every run: source commit
`bace1a92feb7d9e862c21d295fd84df6a6df6626`, SHA-256
`d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`.
It is the same executable used by E010, from
[binary export run 34767444533](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34767444533).
The native source/defaults are unchanged. Each query has a fresh process;
at most four run concurrently with RAYON_NUM_THREADS=1. All 96 default/modal
report pairs have identical times, selected songs, evidence, confidence and
vote counts; point estimates, verification and start/end events may differ.

Run the independent recount directly from a checkout, with no audio or
native executable required:

```sh
python3 scripts/recount_song_index_archives.py
```

To regenerate audio and execute the experiments, follow
[E011 protocol](../../experiments/E011-song-index.md) and
[E012 protocol](../../experiments/E012-full-modal.md). The complete tables
and descriptive uncertainty are in [E011 results](../../experiments/E011-results.md)
and [E012 results](../../experiments/E012-results.md); approximate source
mapping is documented in [the four-case diagnostic](../../experiments/E012-matched-source.md).

The preparation record retains its failed long-template annotation attempt
and the pre-native correction to inherited t01 support. The final manifest
was frozen before any native E011 outcomes. Queries and earlier failures
were not replaced after inspecting recognition results.

These are development measurements on one known mix with algorithmic cue
labels. Each negative set provides 80 seconds of previously known music;
this is not a production false-alarm-rate estimate. Index bytes are the
native index estimate, and concurrent wall times are not speed evidence.
The broad research goal remains incomplete. No merge or release.
