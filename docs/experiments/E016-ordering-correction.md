# E016 implementation correction before the complete paired run

The frozen protocol remains unchanged. Initial source `7f4cf65` appended pair
proposals after weak triplets. The unchanged state machine takes the first
compatible current candidate, so a weak triplet could hide the pair admission
score for the same trajectory. Source inspection found this integration defect
before heavy-noise, robustness, negative or full-mix outcomes were available.

The first run was stopped with 18 completed query records and 12 interrupted
active records (30 attempted invocations). All available raw files, including
their original identities and executed harness, are retained under
`target/e016/initial-ordering/`. Completed exact/clean/frozen/random query
outcomes remain visible but are excluded from the final paired configuration.

Correction: inside the opt-in pair path, stably order current proposals by
their existing admission/confidence score before passing them to the unchanged
state machine. Qualifying triplets remain primary; pairs are eligible only
for songs lacking such a triplet. Do not sum scores or change any support,
collision, candidate, trajectory, verification, cadence or decision threshold.

The regression first runs with a no-op ordering adapter on the initial source
and must fail to start despite two aligned intervals. The corrected adapter
must start only after two passing intervals, reject a failed fresh check, and
retain a strong triplet's precedence. Preserve the failing log and adapter
patch as well as the passing result. This is a correctness repair of protocol
implementation, not a second tuned retrieval configuration.

Resource amendment, recorded before resuming: allow 260 total main/parity/
interrupted attempts (229 final invocations plus the 30 initial attempts),
within the original three-hour limit and six-process concurrency. Rerun the
entire final paired matrix using the corrected source, with the same frozen
audio, labels and acceptance rule. No result-driven threshold tuning.
