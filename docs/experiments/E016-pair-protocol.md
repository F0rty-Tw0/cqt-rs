# E016: bounded pair-assisted candidate retrieval

Status: **planned; frozen before implementation and candidate outcomes**.
PR #4 live head and the clean experimental checkout were verified at
`25435ea781315b247a0a4331bd4be95f23309fe8` on 2026-09-14.
Work branch: `codex/e016-pair-retrieval`; publication target is PR #4.

## Hypothesis and isolated change

E015 identifies retrieval as the immediate blocker for all 14 heavy-noise
misses. Two surviving peaks may retrieve a trajectory when no stable triplet
survives. Their weaker discrimination requires explicit collision, support,
and candidate limits. This revisits distinct-anchor support only for a new
pair fallback, not the previously rejected replacement for triplet evidence.

Baseline is the pinned source above with `--modal-fit --window 10
--continuation-seconds 2 --continuation-hypotheses 3`. Candidate adds only
`--pair-fallback`. Triplet extraction, indexing, confidence and ranking stay
primary and unchanged. The fallback is eligible only for songs without a
triplet hypothesis reaching the existing confidence threshold in that window.
It may propose a trajectory, never a recognized play. Fresh two-second
verification, 0.4 start / 0.3 hold alignment, two consecutive observations,
three hypotheses per song, release and trajectory compatibility stay fixed.

No labels, expected IDs, source positions or oracle checks enter recognition.
The runtime interface accepts watched audio and stream audio only. Labels are
used after execution for scoring. Voiceover verification is out of scope.

## Frozen algorithm and bounds

- Build pairs independently from peaks: positive spans from 0.25 through
  2 seconds, at most the first 16 eligible partners per anchor. Index and
  query use the same rule; query context is ten seconds.
- Key by signed frequency-bin difference, with +/-1 query tolerance. Store
  original anchor/bin/span. Span is not a fixed-time equality key: search
  reference spans in [0.7, 1.4] times query span, derive continuous tempo
  from the span ratio, and derive pitch shift from anchor bins (limit +/-24).
  Index sorted span ranges bound lookup without assuming tempo equals one.
- Retain at most 4,000,000 index entries total; if another song would exceed
  this limit, reject that song's pair index, record the rejection, and retain
  its triplet index. No partial song silently disappears. Report index bytes.
- At most 512 retained query peaks; overflow skips pair retrieval for that
  observation. At most 4,096 query pairs, selected in deterministic anchor
  order. Drop an entire lookup range exceeding 4,096 reference occurrences
  (across all three difference probes); do not retain a favorable prefix.
- At most 262,144 matched pair votes per observation. Reaching the limit
  stops collection and is reported. Quantize tempo at 0.02 and reference
  position at the context origin at 0.5 seconds; keep integer pitch bins.
- Rank cells deterministically; refine at most 64 cells using neighboring
  pitch/tempo/position cells. One support item per query anchor frame and
  per reference anchor frame; multiple partners/bins do not multiply support.
  Require at least six distinct anchors in at least three disjoint two-second
  time bins, spanning at least four seconds, with an anchor in the newest
  two-second interval. Fit reference time against query time by least squares,
  then recount support within four frames and one pitch bin and reapply all
  support gates. Reject out-of-range fitted tempo/pitch.
- Propose at most eight pair trajectories globally and at most three per
  song, after deterministic deduplication by existing trajectory tolerances.
  A pair candidate's evidence is its distinct-anchor count. Its admission
  score is exactly the configured start threshold after the support gate;
  this is explicitly a pair admission score, not calibrated triplet confidence
  and never pooled with triplet votes. Log origin `pair_retrieval`, support,
  collisions, discarded work and candidate counts separately. The unchanged
  state machine still requires the current admission score and consecutive
  fresh verification. Defaults allocate no pair index or retained context.

## Inputs and paired measurements

Reuse E014's exact 22 full references and frozen query identities from
`docs/evidence/E011/inputs-and-provenance.zip`; recover via
`scripts/continuation_inputs.py`. Prepare the same E014 programmes/transforms
with `scripts/continuation_eval.py prepare` and `prepare-robustness`. Verify
the archived reference/query hashes and E014 programme hashes before scoring.
Changed input bytes block a paired claim; never relabel or drop a failed case.
These are development recordings and synthetic speech, not a held-out split.

Run both arms on 96 queries (22 exact, clean, frozen mix and random mix; eight
known music negatives), the 25-play programme and one-second phase variant,
all 12 robustness programmes, and the complete mix: 222 main invocations.
Include a pinned-baseline/default semantic check and pair block-size
257/4096/65536 plus file/live PCM parity on the controlled programme. Use
one native thread and at most six concurrent native processes. Record every
command, status, source/patch, lockfile, binary, input, evaluator and log hash.

Primary metric: correct heavy-noise plays and each of the prior 14 misses.
Also report false/wrong starts, exposure, duplicates, misses, premature ends,
start/end notification delay with misses retained, pitch/tempo/source-position
errors, all query/treatment outcomes, and full-mix coverage/segments. Report
per-observation pair survival/support, lookup collisions, dropped ranges,
vote saturation, candidates proposed/verified/accepted and index/query memory
bounds. Pair support recovery is a retrieval diagnostic, not recognition.
Use play-level paired bootstrap intervals as descriptive development evidence.

## Acceptance and execution limits

Accept only as an opt-in development candidate if heavy-noise recall exceeds
8/22 with no lost baseline heavy-noise play, no added false/wrong starts,
no lost baseline identity in any frozen regression treatment/query, no added
controlled duplicate/premature endings, and all invariance, default, parity,
boundedness and repository CI gates pass. All 14 misses remain in the table
even if unrecovered. A lost identity or extra false start rejects this gate.
No default switch, general recognition, human-voiceover or speed claim.

Budget: one frozen candidate configuration, 222 main plus at most 12 parity
or integrity invocations; 900 seconds per normal invocation, 1,800 per full
mix; at most three hours total execution including input recovery/builds.
Unit/synthetic tests may debug correctness before audio evaluation. No
post-outcome threshold/cap tuning; an algorithmic change needs a separately
recorded experiment. Preserve setup failures, timeouts and interrupted logs.
If exact inputs/builds cannot be recovered, complete independent code/tests
and mark the audio gate blocked rather than substituting oracle recognition.

Planned commands: `python3 scripts/pair_eval.py run` and
`python3 scripts/pair_eval.py report` (harness to be implemented), plus the
Rust/Python/CLI gates in `docs/GOAL.md`. Persist compact raw numeric evidence,
results and exact next action in the repository. No merge or release.
