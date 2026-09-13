# E014 resource-only scheduling amendment

Recorded while primary query/full-mix execution is running and before the
programme/robustness outcomes. Native source, inputs, thresholds, candidate
budgets, scoring and the frozen query labels are unchanged.

After all three full-mix processes finish, reuse their two process slots for
controlled/phase and robustness programmes, at two workers. Four short-query
workers may remain active, so total concurrent native processes stays at most
six, each with one Rayon thread. This raises the combined short/robustness
worker ceiling from four to six during the overlap, without raising total
concurrency. Wait for the queries before the final parity stage reads a shared
cached clean-query result. No speed claim; sample-time notifications and
supported-edge metrics do not depend on this wall-time scheduling choice.

Command: `python3 scripts/continuation_remaining.py`. The original stage
commands remain available for sequential reproduction. Preserve the 45-minute
native budget and every failed case. This amendment does not permit tuning
based on outcomes.
