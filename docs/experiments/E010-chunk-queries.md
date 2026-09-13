# E010: ten-second DJ-mix queries against complete chunk coverage

Status: execution verified; full 22-labelled-query acceptance blocked by
t05's annotation gap. Protocol recorded before native evaluation on
2026-09-13. The user requested this experiment on PR #4.

[Measured results](E010-results.md): all 44 native runs complete; 2/21 →
13/21 supported identities, no wrong-song accepted starts or lost correct
identities. The following protocol preserves the predeclared comparison.

## Frozen question and comparison

Does indexing every non-overlapping ten-second section of all 22 original
Toucan songs recover more of 22 short clips from the actual DJ mix than
indexing only one midpoint section per song?

Use the existing 22 originals and mix, with E009 source/PCM hashes. There
are 858 complete ten-second sections and 22 unpadded shorter tails (880
reference records, mapped explicitly to 22 parent songs). Do not substitute
clips from the clean originals for recordings from the mix.

The source and executable for both reference configurations are PR #4 head
`bace1a92feb7d9e862c21d295fd84df6a6df6626`; native default thresholds and
modal fit disabled. This isolates reference coverage/segmentation, not a
matcher implementation change. Preserve PR #3's comparison pin; this
experiment does not replace its numerical or performance baseline.

The existing CLI assigns separate internal IDs to reference chunks. Map
accepted detections back to parent song IDs, and translate position by
adding the chunk's original start offset. The bucket-frequency cap remains
per indexed chunk: this is part of the chunk-database treatment and differs
from a single full-track index. Do not claim that chunk and full-track
indexing have identical semantics.

Each query contains exactly 441000 mono PCM samples from the mix. Run a
fresh monitor process for each query, searching all references, with
RAYON_NUM_THREADS=1. Neither expected identity nor track order is passed
to the evaluated matcher. Use identical query bytes on both sides.

For each query, choose the accepted start with greatest evidence; break
ties by earliest consumed time and lexicographic chunk ID. Map that start
to the parent song. No accepted start means no-match; a wrong parent song
means wrong-match. Retain every accepted start, competing parent identity,
and repeated start. Report correct/wrong/no-match out of 22, paired gains
and losses, and a 22-row result table. Do not sum evidence over unrelated
chunks. Diagnostic report lines without a start are not accepted matches.

Exploratory success: strictly more correct identities with no lost correct
baseline identity and no additional wrong top predictions. Always report
all outcomes even when this rule fails. No production-reliability, rare
false-alarm, end-to-end speed or unseen-recording claim follows from this
single known mix. Timings are descriptive single runs, not speed evidence.

## Annotation gate

The publisher lists order only. The Mixcloud API returned an empty sections
list on 2026-09-13. Before native matching, derive a provisional cue map
using a separate STFT-based audio alignment method and the known originals.
Freeze its algorithm, all inferred spans and actual query sample positions
before running either reference treatment. Record ambiguous annotations
explicitly. These are algorithmic annotations, not human-verified cue times
or a new recording-disjoint test set. If a song cannot be supported by the
annotation method, preserve that missing gate; do not invent its timestamp.

## Execution and evidence

Budget: up to 60 minutes for preparation, alignment and 44 native queries;
600 seconds per native query; checkpoint identities before native runs and
results after each run. Use the existing source/binary export rather than
changing native code. Preserve executable, archive, evaluator, source,
query and raw-output hashes, commands, exit codes, host/runtime, FFmpeg and
Rust build metadata. Refuse to overwrite prior result sets.

Initial commands:

```sh
python3 scripts/chunk_query_prepare.py
python3 scripts/chunk_query_align.py
python3 scripts/chunk_query_align_short.py
python3 scripts/chunk_query_freeze.py
python3 scripts/chunk_query_eval.py --label default-a
```

Binary artifact: GitHub Actions run 34767444533, artifact 10320783776,
archive SHA-256 `d06acaccaf717e83f858f3e5a1568dad3f101d771b4ab43bb4bf32163220991a`.
The downloaded ZIP matched its GitHub digest and the candidate runs locally.
No new recognition result is claimed at this planning checkpoint.

## Annotation checkpoint before native query evaluation

The initial global spectral-cosine pass produced misleading stationary
pattern matches. It was rejected before either query treatment. The final
aligner subtracts each template band's temporal mean and normalizes by
the corresponding mix-window temporal variance. It uses 60-second source
templates every 30 seconds, tempo 0.88 to 1.12 in 0.02 steps and integer
pitch shifts -2 through +2. Group anchors at a consistent source/mix offset
(within five seconds); select the group with the most anchors, breaking
ties by summed similarity then earliest mix start. Require at least two
anchors of similarity >=0.45. The query is centered in that group's span.

t01 requires 20-second templates every ten seconds in mix interval [0,180]
because a full-minute template failed. Five consistent anchors support its
selected span. The shorter annotation search for t05 in [700,1000] still
failed to establish a correspondence. Those intervals come from publisher
order and neighboring songs' supported anchors, not native query results.

There are **21 algorithmically supported query labels and one unverified
label (t05)**. Keep all 22 query files and all 44 native outcomes, but exclude
t05 from the supported accuracy denominator. Its provisional query is the
midpoint of the gap between the t04 anchor span's end and t06 span's start.
Do not count its outcome as a verified correct identification or miss.
The remaining labels are algorithmic, not independent human annotation.

Four decoded original caches were found truncated on a subsequent hash
read (t01/t02/t09/t19). All downloaded originals, midpoint files and all 880
chunk hashes remained correct. Restore originals only to the exact frozen
hashes, invalidate/recompute affected annotation features, and record the
incident. t19 required capturing FFmpeg's WAV output and finalizing its
RIFF/data lengths in Python; its restored hash matches the frozen original.
Native outputs likewise use captured stdout before durable file writes.

Every final cached feature array was subsequently recomputed from its
hash-verified original and compared with `numpy.array_equal`: all 23 arrays
(22 sources and mix) matched exactly. The short-template driver also
reproduced both retained JSON anchor sets exactly. Thus repaired source
caches did not leave changed features in the frozen query annotations.

For a fresh reproduction, install NumPy 2.3.5 and SciPy 1.17.0, use FFmpeg
6.1.1-3ubuntu5, and obtain the candidate from the linked binary artifact.
Extract into `target/chunk-query/binaries` and make `candidate` executable.
Require candidate SHA-256
`d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`.
The preparation script requires all original and midpoint hashes to match
the existing source manifest. Preserve old runs rather than overwriting
them. Audio is downloaded from the previously pinned publisher URLs;
source attribution remains in `docs/evidence/E007/ATTRIBUTION.txt`.

Run the commands above in a fresh checkout/work directory, then:

```sh
python3 scripts/chunk_query_report.py --label default-a
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```

The exact frozen query positions and hashes in `docs/evidence/E010/queries.json`
are the reproduction target. Do not choose new queries after inspecting
native matching results. Full audio and native binaries stay out of Git;
the source URLs/hashes, native build artifact, scripts, metadata and compact
raw-output archive make the experiment reviewable and repeatable.

Native database/self-query smoke passed with 880 indexed records,
5,069,409 stored hashes, 3,834 dropped occurrences, and 119,553,164 reported
index bytes. That clean-source control is not one of the 22 mix queries.
