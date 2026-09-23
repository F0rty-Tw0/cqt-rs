#!/usr/bin/env bash
# Reproduces the README table "Compared with other libraries".
#
# Needs: cargo (Rust 1.98+) and uv (installs Python 3.12 and the Python
# libraries itself). Linux `taskset` is optional: when present, the
# single-threaded rows are pinned to CPU $PIN (default 4, a P-core on the
# i7-13700H the README numbers come from).
#
#   scripts/compare/run.sh
#
# Prints one markdown table; the last two rows are cqt-rs with its rayon pool.
set -euo pipefail
cd "$(dirname "$0")"

signals=compare-rs/target/signals
binary=compare-rs/target/release/compare-rs
export OMP_NUM_THREADS=1 MKL_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 NUMBA_NUM_THREADS=1
pin=()
if command -v taskset >/dev/null; then pin=(taskset -c "${PIN:-4}"); fi

cargo build -q --release --manifest-path compare-rs/Cargo.toml

echo "| Library | Legacy grid, 30 s | Fingerprint grid, 10 s | Notes |"
echo "| --- | ---: | ---: | --- |"
RAYON_NUM_THREADS=1 "${pin[@]}" "$binary" "$signals"
"${pin[@]}" uv run -q compare_py.py "$signals"

cargo build -q --release --manifest-path compare-rs/Cargo.toml --features parallel
"$binary" "$signals"

echo
rustc --version
cargo tree -q --manifest-path compare-rs/Cargo.toml -e normal --depth 1 --prefix none
