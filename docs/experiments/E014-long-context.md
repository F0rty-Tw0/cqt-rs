# E014: long-context retrieval with fresh continuation checks

Status: planned; frozen before implementation or E014 native outcomes.

## Hypothesis and scope

Ten seconds of continuous fingerprint evidence can retain candidate identities
that disjoint two-second retrieval loses. Verify each proposed source trajectory
on the next disjoint two-second peak interval. A rolling retrieval score is used
once per decision; overlapping windows are never summed. Previously proposed
trajectories remain eligible for direct continuation verification even when
their hash evidence expires. Old votes alone cannot renew a play.

The user targets 100% matching under pitch, BPM, EQ and noise changes, with good
voiceover matching. This is a bounded development experiment toward that goal,
not a promise under arbitrary masking or a recording-disjoint accuracy claim.
Preserve all E010–E013 results and defaults. No merge or release.

## Comparators and candidate

Source baseline: PR #4 `36098740e78556e41e3e4bbd477508f191331d21`.
Pinned original PR #3 baseline remains unchanged. Use one E014 executable for
the paired modes, after proving the legacy path has unchanged semantic output
against the exported baseline on a clean query and controlled programme.

| Arm | Options | Purpose |
| --- | --- | --- |
| legacy | `--modal-fit` | Existing five-second retrieval, recorded E012 comparator |
| long | `--modal-fit --window 10 --report 2` | Ten-second rolling retrieval, legacy tracker |
| continuation | `--modal-fit --window 10 --continuation-seconds 2 --continuation-hypotheses 3` | Primary candidate |
| single | `--modal-fit --window 10 --continuation-seconds 2 --continuation-hypotheses 1` | Candidate-budget ablation on controlled programme and full mix |

Keep CQT, peak picking, triplets, corrected full-song catalogue, per-song index
cap, lookup tolerances and confidence function unchanged. The principal change
is separating retrieval lifetime from continuation decisions; the single arm
isolates the contribution of retaining competing source positions.

Candidate settings: up to three distinct retrieval trajectories per song;
retain up to three pending trajectories plus one active play per song. Start
only after two consecutive complete observations with alignment >=0.4 and
current long-context confidence >=70 (`100*n/(n+40)`). At least one of the two
checks verifies a previously proposed trajectory on newly arriving peaks.
Confidence is not a probability; votes and peak checks are correlated.
Agreement: pitch within two bins, tempo within 0.05, predicted position within
0.5 seconds. An active trajectory renews on fresh alignment >=0.3, independent
of its old confidence. Allow the existing three-second dropout interval,
rounded to observation cadence. Confirm a changed trajectory before replacement.
Partial final intervals cannot start, confirm or renew a play. Supported edges,
logical event times and input-consumed notification times remain separate.

## Frozen inputs and evaluation

Recover the exact corrected 22 full references and all 96 E011 queries from
`docs/evidence/E011/inputs-and-provenance.zip`; verify source and PCM hashes.
Do not recreate random cuts or relabel t01/t02. Missing or changed input bytes
block that paired comparison. E013 raw archives absent from this checkout are
unavailable evidence, not silently reconstructed historical results.

1. Rust behavioral tests: stale retrieval cannot hold or restart a play;
   overlapping rolling evidence cannot be summed; a previously proposed
   trajectory must pass on fresh peaks; competing repeated positions, overlap,
   release, dropout, EOF, nonintegral cadence, bounded state, hash conservation,
   block-size invariance and rebasing. Preserve required repository CI gates.
2. Run all 96 unchanged queries in long and continuation modes. Compare both
   to the archived E012 legacy modal results without conflating the arms.
   Report identities, every miss/wrong start, duplicate starts, source-position
   error for clean controls, and notification delays with misses retained.
3. Recreate E013's 584-second, 25-play programme using its checked-in preparer;
   run legacy, long, continuation and single. Include a one-second global
   phase shift, block sizes 257/4096/65536, and file/live PCM parity. Score
   per-play recall, all false/duplicate starts, premature endings and signed
   supported-edge errors separately from notification delays.
4. Entire existing 5436.510771-second DJ mix: long, continuation and single.
   Report all segments and per-song coverage at frozen/random query locations.
   Algorithmic labels do not establish exact audible boundaries.
5. Robustness programmes: the same 22 frozen random clean clips, seven seconds
   of silence before and nine after each. Fixed treatments: gain-only 0.15;
   pitch -2/+2/+0.5 semitones; key-locked tempo 0.88/1.12; bass and treble EQ
   -12 dB at 200/3000 Hz separately; seeded white noise at 10/0 dB music SNR;
   continuous synthetic speech at 0 dB speech/music RMS; and combined +2
   semitones, tempo 0.88, bass -12 dB, 10 dB noise and 0 dB speech. Use ffmpeg
   rubberband and the existing frozen E007 voice text with flite/slt. Normalize
   added noise/speech to the transformed clip RMS; apply one common gain if
   needed for peak headroom, and retain transformation commands/hashes. Run
   long and continuation, report each treatment separately. These are known
   recordings and synthetic speech, not real human voiceover generalization.

## Acceptance, evidence and budget

Reject a default switch if any previously correct clean/mix query is lost,
wrong/negative starts increase, or continuous-play misses/duplicates increase.
Also report every robustness regression and all source-position ambiguities.
Passing a finite treatment cell is 22/22 on that cell, not universal 100%.
No threshold tuning after reading outcomes. A failed implementation gate may
be fixed and rerun with both revisions recorded; a failed quality gate stays
visible and the candidate stays opt-in.

Retain protocol/evaluator/source/binary/input hashes, exact commands, exits,
raw JSONL/stderr, toolchain, lockfile, configuration and one Rayon thread per
process. Checkpoint each run and verify inputs/configuration/output identity
before reusing it. Independently recount the final score table from raw events.
No speed claim; fresh-process indexing and concurrent wall time are not a
benchmark. Only 80 seconds of frozen negative music per query arm.

Budget: <=192 primary short-query runs, <=12 programme/parity runs, three
full-mix runs, 24 robustness-programme runs, two baseline parity runs; up to
four short/robustness workers and two full-mix workers, <=45 minutes native
execution and <=15 minutes CI per implementation revision. If dependencies or
data cannot be recovered, retain the completed work and precise blocked gate.

Commands will be implemented in `scripts/continuation_eval.py` with `prepare`,
`run` and `report` subcommands. The protocol is committed before source edits.
