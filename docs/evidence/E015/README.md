# E015 evidence

[Findings](../../experiments/E015-results.md).
`plays.csv` retains all 110 plays, including every miss; `windows.csv` retains
all interior two-second oracle windows. `audit.json.gz` contains the complete
JSON audit, including native observations and all 32 nominal parity differences
with their serialization uncertainty bounds. `extractions.json.gz` records
all 27 native commands, inputs, output hashes and exits. Compressed files are
ordinary gzip and can be read with Python gzip or `gzip -dc`.

The separately supplied `E015-evidence.zip` contains every raw native P/H export
(compressed text), extraction records, stderr, full audit, protocol, diagnostic
source and tests. It contains no audio or executables. See `archive.json` for
its SHA-256 and size. Every ZIP member is hashed in SHA256SUMS.
