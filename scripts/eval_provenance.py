"""Content identities for reproducible monitor evaluations (standard library only)."""

from __future__ import annotations

import hashlib
import os
import platform
import subprocess
import sys
from pathlib import Path


def file_identity(path: Path) -> dict:
    path = path.resolve(strict=True)
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return dict(path=str(path), bytes=path.stat().st_size, sha256=digest.hexdigest())


def checkout_identity(root: Path) -> dict:
    def git(*args: str) -> str:
        return subprocess.run(["git", *args], cwd=root, check=True,
                              capture_output=True, text=True).stdout.strip()

    try:
        return dict(commit=git("rev-parse", "HEAD"),
                    dirty=bool(git("status", "--porcelain", "--untracked-files=normal")))
    except (OSError, subprocess.CalledProcessError):
        return dict(commit=None, dirty=None)


def manifest(root: Path, monitor: Path, inputs: dict[str, Path], *, built_here: bool) -> dict:
    """The checkout is evaluator provenance, never proof of an external binary's origin.

    SHA-256 identifies the actual executable and evaluated inputs. `built_here`
    records whether this invocation built the executable from this checkout;
    it does not identify the source of a binary supplied via --monitor.
    """
    scripts = ("radio_eval.py", "eval_provenance.py")
    return dict(
        schema_version=1,
        evaluator=checkout_identity(root),
        evaluator_files={name: file_identity(root / "scripts" / name) for name in scripts},
        monitor=dict(**file_identity(monitor), built_from_evaluator_checkout=built_here),
        inputs={name: file_identity(path) for name, path in sorted(inputs.items())},
        runtime=dict(python=sys.version, platform=platform.platform(),
                     logical_cpus=os.cpu_count(),
                     environment={name: os.environ[name] for name in
                                  ("RUSTFLAGS", "RAYON_NUM_THREADS", "CARGO_BUILD_TARGET")
                                  if name in os.environ}),
        timing=dict(realtime_fraction="elapsed wall seconds / audio seconds; not CPU utilization",
                    includes_index_build=False),
    )


def validate_comparison(before: dict, after: dict, *, allow_unverified: bool = False) -> None:
    """Reject comparisons of different audio/truth/watch lists, even at identical timestamps.

    Detector parameters and binary identities may differ: those are the experiment.
    Legacy summaries require an explicit opt-in because their inputs are unknown.
    """
    left, right = before.get("provenance"), after.get("provenance")
    if not left or not right:
        if allow_unverified:
            return
        raise ValueError("summary lacks input provenance; rerun the evaluation or use --allow-unverified for legacy data")
    if left.get("schema_version") != 1 or right.get("schema_version") != 1:
        raise ValueError("unsupported evaluation provenance schema")
    a = {name: info["sha256"] for name, info in left["inputs"].items()}
    b = {name: info["sha256"] for name, info in right["inputs"].items()}
    if not a or not b:
        raise ValueError("evaluation provenance has no inputs")
    changed = sorted(name for name in a.keys() | b.keys() if a.get(name) != b.get(name))
    if changed:
        raise ValueError("evaluation inputs differ: " + ", ".join(changed))
