#!/usr/bin/env python3
"""Five alternating process pairs for the real verify_probe executables."""

import argparse
import hashlib
import json
import math
import platform
import statistics
import subprocess
from pathlib import Path


def identity(path):
    return {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--output", default="target/verifier-proof")
    args = parser.parse_args()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    binaries = {side: Path(getattr(args, side)).resolve(strict=True) for side in ("baseline", "candidate")}
    report = {"scope": "exploratory synthetic verifier wall time; not end-to-end or representative-host proof",
              "platform": platform.platform(), "binaries": {s: identity(p) for s, p in binaries.items()},
              "runner": identity(Path(__file__)), "runs": [], "summary": {}, "status": "running"}
    expected_names = {f"{size}_{kind}" for size in ("small", "medium", "large", "whole")
                      for kind in ("match", "wrong_shift")}
    try:
        for repetition in range(5):
            order = ("baseline", "candidate") if repetition % 2 == 0 else ("candidate", "baseline")
            for side in order:
                command = [str(binaries[side])]
                process = subprocess.run(command, capture_output=True, timeout=60)
                prefix = output / f"{repetition}-{side}"
                prefix.with_suffix(".jsonl").write_bytes(process.stdout)
                prefix.with_suffix(".stderr.txt").write_bytes(process.stderr)
                run = {"repetition": repetition, "side": side, "command": command,
                       "returncode": process.returncode,
                       "stdout": identity(prefix.with_suffix(".jsonl")),
                       "stderr": identity(prefix.with_suffix(".stderr.txt"))}
                report["runs"].append(run)
                if process.returncode:
                    raise RuntimeError(process.stderr.decode())
                rows = [json.loads(line) for line in process.stdout.splitlines()]
                if len(rows) != 8 or {r["case"] for r in rows} != expected_names:
                    raise AssertionError("probe case set changed")
                for row in rows:
                    if not math.isfinite(row["ns_per_call"]) or row["ns_per_call"] <= 0:
                        raise AssertionError("invalid timing")
                run["cases"] = {row["case"]: row for row in rows}
        for name in sorted(expected_names):
            grouped = {side: [run["cases"][name] for run in report["runs"] if run["side"] == side]
                       for side in binaries}
            counts = grouped["baseline"][0]["counts"]
            if any(row["counts"] != counts for rows in grouped.values() for row in rows):
                raise AssertionError(f"{name}: verification counts changed")
            values = {s: [r["ns_per_call"] for r in rows] for s, rows in grouped.items()}
            medians = {s: statistics.median(v) for s, v in values.items()}
            paired = [100 * (a - b) / a for a, b in zip(values["baseline"], values["candidate"])]
            result = {"ns_per_call": values, "median_ns": medians, "counts": counts,
                      "paired_reduction_percent": paired,
                      "reduction_percent": 100 * (medians["baseline"] - medians["candidate"]) / medians["baseline"]}
            report["summary"][name] = result
            print(f"{name}: baseline={medians['baseline']:.1f} ns candidate={medians['candidate']:.1f} ns "
                  f"reduction={result['reduction_percent']:.2f}% paired_range=[{min(paired):.2f}, {max(paired):.2f}]%")
        report["timing_rule_met"] = all(r["reduction_percent"] >= (10 if name.startswith("large_") else -5)
                                        for name, r in report["summary"].items())
        report["status"] = "measured"
        print(f"Predeclared exploratory timing rule met: {report['timing_rule_met']}")
    except Exception as error:
        report["status"] = "failed"
        report["error"] = repr(error)
        raise
    finally:
        (output / "timing.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
