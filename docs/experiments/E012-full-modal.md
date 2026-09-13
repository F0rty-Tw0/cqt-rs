# E012: modal timing estimates on the corrected complete-song catalogue

Status: execution verified; predeclared blanket acceptance rejected because
modal fitting adds a wrong identity on frozen t01. All 96 main candidate
runs and four matched-source diagnostics completed. See [results](E012-results.md).
The following protocol was recorded before E012 native outcomes, as a
follow-up to E011's reproduced exact-source failures, 2026-09-13.

## Hypothesis and baseline

E011's unchanged default full-song layout misses exact queries t18 and t22,
although both report the expected song with high evidence. Their largest
reported correct-candidate alignment while confidence exceeds 70 is 0.259
and 0.032, respectively, below the fixed 0.4 start gate. Both exact queries
are recognized by E011's independent-chunk layout. The failures occur on
unmodified source audio, so a DJ-mix alteration cannot explain these controls.

The existing default matcher averages point estimates across the winning
neighbourhood. The opt-in `--modal-fit` introduced in E009 uses its most
occupied cell for shift, tempo and position while retaining neighbourhood
evidence. Hypothesis: averaging competing alignments contributes to the
observed verification failures, and modal estimates recover identities at
unchanged gates. This is a configuration experiment using existing code,
not a new native implementation or proof that every failure has this cause.

Baseline: E011 default full-song runs, pinned native source
`bace1a92feb7d9e862c21d295fd84df6a6df6626`, binary SHA-256
`d3494a5c0ff0a5e5d78548d73877810f3653e666fde59761d83ee5fade1e82cc`.
Candidate: exactly the same binary, references, query bytes and arguments,
with `--modal-fit` appended. The 22-song catalogue includes corrected t05.
Keep confidence 70 and alignment 0.4 fixed. Do not combine this comparison
with a change to chunks, audio, thresholds, hashing or peak picking.

## Inputs, acceptance and budget

Use all 96 queries frozen for E011: 22 exact chunks, 22 random clean cuts,
22 prior mix cuts, 22 random mix cuts and eight known music negatives.
These known development recordings and exposed E011 queries are not a
held-out final test. Do not select only the failures for candidate reporting.

Scoring is identical to E011, including every accepted wrong-parent start,
repeated parent start and no-match. Pair each candidate with its exact E011
full-song baseline. Target 22/22 on both clean groups. Exploratory acceptance
requires recovery of the two triggering exact misses, more correct mix
queries, no lost correct query in any group, and no added wrong-parent starts
on any query (including negatives). Show every loss if the rule fails;
the option stays experimental and disabled by default.

Run only after E011's full matrix has completed. Budget: 96 additional
fresh native processes, at most four concurrently, RAYON_NUM_THREADS=1,
600-second per-query timeout and 30-minute phase budget. The combined
E011/E012 task remains within its 90-minute research budget. Persist exact
commands, script/binary/input/output identities and partial results before
publication. Native elapsed times are not a performance claim.

Recalculate all claims from raw output and validate the existing 18 Python
regressions and exact final-commit CI gates. A positive result would support
using this configuration on this development corpus, not making it the
default, claiming 100% general recognition, or completing the research goal.

Exact command: `python3 scripts/song_index_modal.py --label modal-a --workers 4`.

Audit also compares every report's time, selected song, evidence, confidence
and vote count with the paired default run. Those fields should remain
identical; only point estimates, verification and resulting start/end events
may change. Descriptive paired track-bootstrap intervals follow E010's
10,000-resample method (seed 20260913), conditional on this known mix and its
algorithmic labels; they do not cover cue error or new-recording accuracy.

## Bounded matched-passage diagnostic

The default full-song random queries t04, t10, t13 and t22 have maximum
reported confidence only 36.5, 38.5, 47.4 and 48.7. If these remain no-match
under modal fitting, run four clean-source controls after the main phase:
derive each source start from the highest-similarity covering frozen STFT
anchor (`source_start + mix_delta * tempo`), then cut ten untransformed
source seconds. Use the same full database, binary, modal option and gates.
Freeze the four derived positions before their native runs. This checks
whether approximately corresponding clean passages supply usable evidence;
it does not isolate individual DJ effects, exact tempo-adjusted content, or
provide new held-out accuracy. Budget four additional concurrent queries,
each with a 600-second timeout. Keep these adaptive diagnostics separate
from the 288 predeclared main runs. Command:
`python3 scripts/song_index_matched_controls.py`.
