#!/usr/bin/env python3
"""Compare two evaluation summaries written by `scripts/radio_eval.py`.

    python3 scripts/radio_eval.py --monitor OLD_BINARY --label OLD --out eval_summary_before --no-plots
    python3 scripts/radio_eval.py --label NEW
    python3 scripts/radio_compare.py eval_summary_before eval_summary

Writes plots/radio_compare.png: time to detection per play for both runs,
and the null-stream and resource figures side by side.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / "target" / "radio"
PLOTS = ROOT / "plots"


def load(name: str) -> dict:
    return json.load(open(WORK / f"{name}.json"))


def main() -> None:
    before_name, after_name = (sys.argv[1:3] + ["eval_summary_before", "eval_summary"])[:2]
    before, after = load(before_name), load(after_name)
    plays_b = {(p["source"], p["start"]): p for p in before["plays"]}
    plays_a = {(p["source"], p["start"]): p for p in after["plays"]}
    keys = sorted(plays_a, key=lambda k: k[1])
    labels = [f"{plays_a[k]['source']}\n{plays_a[k]['treatment']}" for k in keys]
    lat_b = [plays_b[k]["latency"] if plays_b[k]["detected"] else np.nan for k in keys]
    lat_a = [plays_a[k]["latency"] if plays_a[k]["detected"] else np.nan for k in keys]

    fig, axes = plt.subplots(1, 2, figsize=(16, 6.5), constrained_layout=True,
                             gridspec_kw=dict(width_ratios=[2.5, 1.3]))
    label_b = before.get("label", before["commit"][:7])
    label_a = after.get("label", after["commit"][:7])
    ax = axes[0]
    x = np.arange(len(keys))
    w = 0.38
    ax.bar(x - w / 2, lat_b, w, color="lightgrey", edgecolor="k", label=f"before ({label_b})")
    ax.bar(x + w / 2, lat_a, w, color="tab:blue", edgecolor="k", label=f"after ({label_a})")
    for i, (b, a) in enumerate(zip(lat_b, lat_a)):
        if np.isfinite(a) and np.isfinite(b):
            ax.text(i + w / 2, a + 0.15, f"{a - b:+.1f}", ha="center", fontsize=7, color="tab:blue")
    ax.axhline(5, color="r", ls=":", lw=1, label="5 s target")
    ax.set_xticks(x)
    ax.set_xticklabels(labels, rotation=90, fontsize=7)
    ax.set_ylabel("time to detection from the first sample of the play [s]")
    ax.set_title(f"Programme: {sum(p['detected'] for p in after['plays'])}/{len(after['plays'])} plays detected "
                 f"(median {np.nanmedian(lat_b):.1f} s → {np.nanmedian(lat_a):.1f} s)")
    ax.legend(fontsize=8)
    ax.grid(axis="y", alpha=0.3)

    ax = axes[1]
    rows = [
        ("null max evidence", before["null"]["max_evidence"], after["null"]["max_evidence"], ""),
        ("null max confidence", before["null"]["max_confidence"], after["null"]["max_confidence"], ""),
        ("null false alarms", before["null"]["false_alarms"], after["null"]["false_alarms"], ""),
        ("CPU, null stream", 100 * before["null"]["realtime_fraction"], 100 * after["null"]["realtime_fraction"], "%"),
        ("CPU, programme", 100 * before["eval"]["realtime_fraction"], 100 * after["eval"]["realtime_fraction"], "%"),
        ("hash delay, median", before["stream"]["fingerprint_delay_seconds"] if before["eval"].get("hash_delay_median_seconds") is None
         else before["eval"]["hash_delay_median_seconds"], after["eval"]["hash_delay_median_seconds"], "s"),
        ("index bytes / song-second", before["eval"]["index"][-1]["bytes"] / sum(e["seconds"] for e in before["eval"]["index"][:-1]),
         after["eval"]["index"][-1]["bytes"] / sum(e["seconds"] for e in after["eval"]["index"][:-1]), "B"),
    ]
    ax.axis("off")
    cell = [[name, f"{b:.2f}{u}" if isinstance(b, float) else f"{b}{u}", f"{a:.2f}{u}" if isinstance(a, float) else f"{a}{u}"]
            for name, b, a, u in rows]
    table = ax.table(cellText=cell, colLabels=["", f"before ({label_b})", f"after ({label_a})"], loc="center",
                     cellLoc="right", colLoc="right", colWidths=[0.5, 0.27, 0.27])
    table.auto_set_font_size(False)
    table.set_fontsize(8.5)
    table.scale(1.0, 1.7)
    ax.set_title("Null stream (31 min) and resources", fontsize=10)
    fig.savefig(PLOTS / "radio_compare.png", dpi=90)
    print(f"wrote {PLOTS / 'radio_compare.png'}")
    for name, b, a, u in rows:
        print(f"  {name:28s} {b:>10.2f}{u:2s} -> {a:>10.2f}{u}" if isinstance(b, float) else f"  {name:28s} {b:>10}{u:2s} -> {a:>10}{u}")


if __name__ == "__main__":
    main()
