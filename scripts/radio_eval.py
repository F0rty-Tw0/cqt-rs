#!/usr/bin/env python3
"""Evaluate the watch-list monitor on the simulated radio streams.

Runs `monitor` (the `cqt-monitor` crate) over the null stream, which holds
no watched song, to measure the evidence a false match can reach, then over
the evaluation programme, and compares the detections with the ground
truth written by `scripts/radio_sim.py`.

    python3 scripts/radio_sim.py      # once, builds the streams
    python3 scripts/radio_eval.py [--half N] [--threshold C]

Writes plots/radio_timeline.png, plots/radio_detection.png and
target/radio/eval_summary.json and prints a Markdown table.
"""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / "target" / "radio"
PLOTS = ROOT / "plots"
BINS_PER_OCTAVE = 24
WATCH = {"vibe_ace": WORK / "watch_vibe_ace.wav", "sweet_waltz": WORK / "watch_sweet_waltz.wav"}


def run_monitor(stream: Path, extra: list[str]) -> list[dict]:
    subprocess.run(["cargo", "build", "--release", "--quiet", "-p", "cqt-monitor"], cwd=ROOT, check=True)
    cmd = [str(ROOT / "target" / "release" / "monitor")]
    for name, path in WATCH.items():
        cmd += ["--watch", f"{name}={path}"]
    cmd += ["--stream", str(stream), *extra]
    out = subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True).stdout
    return [json.loads(line) for line in out.splitlines() if line.startswith("{")]


def load_truth(name: str) -> dict:
    return json.load(open(WORK / f"{name}.json"))


def evaluate(args) -> dict:
    extra = ["--half", str(args.half), "--threshold", str(args.threshold), "--window", str(args.window)]
    null_events = run_monitor(WORK / "stream_null.wav", extra)
    eval_events = run_monitor(WORK / "stream_eval.wav", extra)
    null_truth = load_truth("stream_null")
    eval_truth = load_truth("stream_eval")

    # --- null stream: what does a false match look like? --------------------
    null_reports = [e for e in null_events if e["event"] == "report"]
    null_evidence = np.array([e["evidence"] for e in null_reports])
    null_conf = np.array([e["confidence"] for e in null_reports])
    null_starts = [e for e in null_events if e["event"] == "start"]
    null_done = next(e for e in null_events if e["event"] == "done")

    # --- eval stream: match detections with the watched plays --------------
    reports = [e for e in eval_events if e["event"] == "report"]
    starts = [e for e in eval_events if e["event"] == "start"]
    ends = {(e["song"], e["start"]): e for e in eval_events if e["event"] == "end"}
    done = next(e for e in eval_events if e["event"] == "done")
    plays = [s for s in eval_truth["segments"] if s.get("watched")]
    rows = []
    used = set()
    for seg in plays:
        # The first detection of this song that starts inside the play
        # (allowing the pipeline delay past the end).
        cands = [s for s in starts if s["song"] == seg["source"] and seg["start"] <= s["consumed"] <= seg["end"] + 3
                 and id(s) not in used]
        det = cands[0] if cands else None
        expected_bins = round(seg["semitones"] * BINS_PER_OCTAVE / 12)
        row = dict(source=seg["source"], treatment=seg["treatment"], start=seg["start"], end=seg["end"],
                   expected_shift=expected_bins, expected_tempo=seg["tempo"], detected=det is not None)
        if det is not None:
            used.add(id(det))
            row.update(latency=det["consumed"] - seg["start"], confidence=det["confidence"],
                       shift=det["shift"], tempo=det["tempo"])
            # Position error: the song position the monitor reports at the
            # detection frame versus the truth.
            expected_position = seg["excerpt_start"] + (det["t"] - seg["start"]) * seg["tempo"]
            row["position_error"] = det["position"] - expected_position
            inside = [r for r in reports if seg["start"] <= r["consumed"] <= seg["end"] and r["song"] == seg["source"]]
            row["max_confidence"] = max((r["confidence"] for r in inside), default=det["confidence"])
            row["extra_starts"] = len(cands) - 1
            end = ends.get((det["song"], det["consumed"]))
            row["reported_end"] = end["consumed"] if end else None
        rows.append(row)
    false_starts = [s for s in starts if id(s) not in used]
    # A false start inside a play of the *other* song or in filler music.
    summary = dict(
        half=args.half, threshold=args.threshold, window=args.window,
        null=dict(seconds=null_done["audio_seconds"], reports=len(null_reports), max_evidence=int(null_evidence.max()),
                  p999_evidence=float(np.percentile(null_evidence, 99.9)), median_evidence=float(np.median(null_evidence)),
                  max_confidence=float(null_conf.max()), false_alarms=len(null_starts),
                  realtime_fraction=null_done["realtime_fraction"]),
        eval=dict(seconds=done["audio_seconds"], plays=len(plays), detected=sum(r["detected"] for r in rows),
                  false_starts=len(false_starts), realtime_fraction=done["realtime_fraction"],
                  index=[e for e in eval_events if e["event"] in ("index", "index_done")]),
        plays=rows,
    )
    json.dump(summary, open(WORK / "eval_summary.json", "w"), indent=2)

    # --- table --------------------------------------------------------------
    print(f"\nnull stream: {null_done['audio_seconds'] / 60:.1f} min, evidence max {null_evidence.max()}, "
          f"99.9 % {np.percentile(null_evidence, 99.9):.0f}, median {np.median(null_evidence):.0f}; "
          f"max confidence {null_conf.max():.1f}; false alarms at {args.threshold}: {len(null_starts)}; "
          f"CPU {100 * null_done['realtime_fraction']:.2f} % of one core")
    print(f"eval stream: {done['audio_seconds'] / 60:.1f} min, {sum(r['detected'] for r in rows)}/{len(plays)} plays detected, "
          f"{len(false_starts)} false starts, CPU {100 * done['realtime_fraction']:.2f} %")
    print("\n| Song | Treatment | Detected after | Confidence at detection / max | Shift expected / detected | Tempo expected / detected | Position error |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: |")
    for r in rows:
        if r["detected"]:
            print(f"| {r['source']} | {r['treatment']} | {r['latency']:.1f} s | {r['confidence']:.0f} / {r['max_confidence']:.0f} | "
                  f"{r['expected_shift']:+d} / {r['shift']:+d} | ×{r['expected_tempo']:.3f} / ×{r['tempo']:.3f} | {r['position_error']:+.1f} s |")
        else:
            print(f"| {r['source']} | {r['treatment']} | missed | — | {r['expected_shift']:+d} / — | ×{r['expected_tempo']:.3f} / — | — |")

    # --- figure: timeline ---------------------------------------------------
    fig, axes = plt.subplots(2, 1, figsize=(16, 7.5), sharex=True, constrained_layout=True,
                             gridspec_kw=dict(height_ratios=[3, 1.2]))
    ax = axes[0]
    colours = {"vibe_ace": "tab:blue", "sweet_waltz": "tab:orange"}
    for song, colour in colours.items():
        t = [r["consumed"] for r in reports]
        c = [r["confidence"] if r["song"] == song else 0.0 for r in reports]
        ax.plot(t, c, color=colour, lw=1.0, label=f"confidence for {song}")
    for seg in eval_truth["segments"]:
        if seg["source"] == "silence":
            continue
        if seg.get("watched"):
            ax.axvspan(seg["start"], seg["end"], color=colours[seg["source"]], alpha=0.15)
            ax.text((seg["start"] + seg["end"]) / 2, 103, seg["treatment"], ha="center", va="bottom", fontsize=6.5,
                    rotation=90, color=colours[seg["source"]])
        else:
            ax.axvspan(seg["start"], seg["end"], color="grey", alpha=0.08)
    ax.axhline(args.threshold, color="k", ls="--", lw=1, label=f"threshold {args.threshold}")
    ax.axhline(null_conf.max(), color="r", ls=":", lw=1, label=f"highest confidence on {null_done['audio_seconds'] / 60:.0f} min without the songs")
    for s in starts:
        ax.plot(s["consumed"], s["confidence"], "v", color=colours[s["song"]], ms=7, mec="k")
    ax.set_ylim(0, 140)
    ax.set_ylabel("confidence")
    ax.set_title("Watch-list monitor on a simulated radio programme (shaded: watched plays with their DJ treatment, "
                 "grey: other music and speech, triangles: detections)", fontsize=10)
    ax.legend(loc="upper left", fontsize=7, ncol=4)
    ax = axes[1]
    for song, colour in colours.items():
        t = [r["consumed"] for r in reports]
        e = [r["evidence"] if r["song"] == song else 0 for r in reports]
        ax.plot(t, np.maximum(e, 0.5), color=colour, lw=0.9)
    ax.set_yscale("log")
    ax.set_ylabel("evidence (votes)")
    ax.set_xlabel("stream time (s)")
    ax.axhline(null_evidence.max(), color="r", ls=":", lw=1)
    ax.grid(alpha=0.3, which="both")
    fig.savefig(PLOTS / "radio_timeline.png", dpi=90)
    plt.close(fig)

    # --- figure: null distribution and latencies -----------------------------
    fig, axes = plt.subplots(1, 2, figsize=(14, 5), constrained_layout=True)
    ax = axes[0]
    ax.hist(null_conf, bins=np.arange(0, 101, 2), color="grey", label=f"{null_done['audio_seconds'] / 60:.0f} min without the watched songs")
    det_conf = [r["confidence"] for r in rows if r["detected"]]
    max_conf = [r["max_confidence"] for r in rows if r["detected"]]
    ax.hist(det_conf, bins=np.arange(0, 101, 2), color="tab:green", alpha=0.8, label="watched plays: confidence at detection")
    ax.hist(max_conf, bins=np.arange(0, 101, 2), color="tab:blue", alpha=0.5, label="watched plays: highest confidence during the play")
    ax.axvline(args.threshold, color="k", ls="--", label=f"threshold {args.threshold}")
    ax.set_yscale("log")
    ax.set_xlabel("confidence")
    ax.set_ylabel("reports (4 per second)")
    ax.set_title("Confidence without the songs versus with them", fontsize=10)
    ax.legend(fontsize=7)
    ax = axes[1]
    labels = [f"{r['source']}: {r['treatment']}" for r in rows]
    lat = [r.get("latency", 0.0) for r in rows]
    y = np.arange(len(rows))
    ax.barh(y, lat, color=[colours[r["source"]] for r in rows])
    for yi, r in zip(y, rows):
        ax.text(r.get("latency", 0) + 0.1, yi, f"{r['latency']:.1f} s, conf {r['confidence']:.0f}" if r["detected"] else "missed",
                va="center", fontsize=7)
    ax.set_yticks(y)
    ax.set_yticklabels(labels, fontsize=7)
    ax.invert_yaxis()
    ax.set_xlabel("seconds from the start of the play to the detection (includes 2.4 s of pipeline delay)")
    ax.set_title("Time to detection per play", fontsize=10)
    ax.set_xlim(0, max(lat) + 4)
    fig.savefig(PLOTS / "radio_detection.png", dpi=90)
    plt.close(fig)
    return summary


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--half", type=float, default=100.0)
    parser.add_argument("--threshold", type=float, default=70.0)
    parser.add_argument("--window", type=float, default=5.0)
    evaluate(parser.parse_args())


if __name__ == "__main__":
    main()
