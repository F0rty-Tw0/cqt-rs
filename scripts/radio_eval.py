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


def run_monitor(stream: Path, extra: list[str], monitor: str | None = None) -> list[dict]:
    if monitor is None:
        subprocess.run(["cargo", "build", "--release", "--quiet", "-p", "cqt-monitor"], cwd=ROOT, check=True)
        monitor = str(ROOT / "target" / "release" / "monitor")
    cmd = [monitor]
    for name, path in WATCH.items():
        cmd += ["--watch", f"{name}={path}"]
    cmd += ["--stream", str(stream), *extra]
    out = subprocess.run(cmd, cwd=ROOT, check=True, capture_output=True, text=True).stdout
    return [json.loads(line) for line in out.splitlines() if line.startswith("{")]


def load_truth(name: str) -> dict:
    return json.load(open(WORK / f"{name}.json"))


def git_commit() -> str:
    try:
        return subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, check=True,
                              capture_output=True, text=True).stdout.strip()
    except (subprocess.CalledProcessError, OSError):
        return "unknown"


def worst_cases(rows: list[dict], false_starts: list[dict], reports: list[dict], truth: dict,
                keep: int = 3) -> list[dict]:
    """The slowest (or missed) plays and every false start, each with its
    ground-truth segment and the report trace around it, so that a case can
    be replayed and plotted without rerunning the whole evaluation."""
    def trace(t0: float, t1: float) -> list[dict]:
        return [r for r in reports if t0 <= r["consumed"] <= t1]

    ranked = sorted(rows, key=lambda r: (r["detected"], -r.get("latency", 0.0)))
    cases = []
    for r in ranked[:keep]:
        end = r["start"] + r["latency"] + 2.0 if r["detected"] else r["end"]
        cases.append(dict(kind="miss" if not r["detected"] else "slow", play=r,
                          segment=next(s for s in truth["segments"] if s["start"] == r["start"]),
                          reports=trace(r["start"] - 2.0, end)))
    for fs in false_starts:
        seg = next((s for s in truth["segments"] if s["start"] <= fs["consumed"] <= s["end"]), None)
        cases.append(dict(kind="false_start", start=fs, segment=seg,
                          reports=trace(fs["consumed"] - 6.0, fs["consumed"] + 2.0)))
    return cases


def negatives(stream: str, extra: list[str], monitor: str | None, threshold: float) -> dict:
    """Runs a stream that contains no play of a watched song and reports the
    evidence, confidence and start events per family of segments (the
    `family` key of the ground truth, or the source for plain fillers)."""
    events = run_monitor(WORK / f"{stream}.wav", extra, monitor)
    truth = load_truth(stream)
    reports = [e for e in events if e["event"] == "report"]
    starts = [e for e in events if e["event"] == "start"]
    done = next(e for e in events if e["event"] == "done")
    info = next(e for e in events if e["event"] == "stream")
    # A report's evidence window ends a fingerprint delay behind the
    # consumed audio and spans window_seconds; a report (or start) counts
    # for a segment when that whole window lies inside the segment, so
    # evidence left over from the previous segment is not charged to it.
    delay, window = info["fingerprint_delay_seconds"], info["window_seconds"]

    def inside(seg: dict, consumed: float) -> bool:
        return seg["start"] <= consumed - delay - window and consumed - delay <= seg["end"]

    families: dict[str, dict] = {}
    for seg in truth["segments"]:
        if seg["source"] == "silence":
            continue
        family = seg.get("family", "other")
        within = [r for r in reports if inside(seg, r["consumed"])]
        fam = families.setdefault(family, dict(segments=0, seconds=0.0, max_evidence=0, max_confidence=0.0,
                                               max_verify_q=0.0, verify_q_at_max=0.0, false_starts=0, worst=None))
        fam["segments"] += 1
        fam["seconds"] += seg["end"] - seg["start"]
        if within:
            top = max(within, key=lambda r: r["evidence"])
            fam["max_verify_q"] = max(fam["max_verify_q"], max(r.get("verify_q", 0.0) for r in within))
            if top["evidence"] > fam["max_evidence"]:
                fam.update(max_evidence=top["evidence"], max_confidence=top["confidence"],
                           verify_q_at_max=top.get("verify_q", 0.0),
                           worst=dict(treatment=seg["treatment"], source=seg["source"], song=top["song"], t=top["consumed"]))
        fam["false_starts"] += sum(inside(seg, st["consumed"]) for st in starts)
    evidence = np.array([r["evidence"] for r in reports]) if reports else np.zeros(1)
    verify_q = np.array([r.get("verify_q", 0.0) for r in reports]) if reports else np.zeros(1)
    return dict(seconds=done["audio_seconds"], max_evidence=int(evidence.max()),
                p999_evidence=float(np.percentile(evidence, 99.9)),
                max_confidence=max((r["confidence"] for r in reports), default=0.0),
                max_verify_q=float(verify_q.max()), p999_verify_q=float(np.percentile(verify_q, 99.9)),
                false_alarms=len(starts), threshold=threshold, realtime_fraction=done["realtime_fraction"],
                families=families)


def print_negatives(name: str, neg: dict) -> None:
    print(f"\n{name}: {neg['seconds'] / 60:.1f} min, evidence max {neg['max_evidence']}, 99.9 % {neg['p999_evidence']:.0f}, "
          f"max confidence {neg['max_confidence']:.1f}, verify_q max {neg['max_verify_q']:.2f}, 99.9 % {neg['p999_verify_q']:.2f}, "
          f"false alarms at {neg['threshold']}: {neg['false_alarms']}")
    print("| Family | Segments | Minutes | Max evidence | Max confidence | verify_q at max / max | False starts | Worst segment |")
    print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |")
    for family, fam in sorted(neg["families"].items()):
        worst = fam["worst"]
        worst_text = f"{worst['source']} {worst['treatment']} as {worst['song']}" if worst else "—"
        print(f"| {family} | {fam['segments']} | {fam['seconds'] / 60:.1f} | {fam['max_evidence']} | "
              f"{fam['max_confidence']:.1f} | {fam['verify_q_at_max']:.2f} / {fam['max_verify_q']:.2f} | {fam['false_starts']} | {worst_text} |")


def evaluate(args) -> dict:
    extra = ["--half", str(args.half), "--threshold", str(args.threshold), "--window", str(args.window)]
    extra += [a for spec in args.arg for a in spec.split()]
    null_events = run_monitor(WORK / "stream_null.wav", extra, args.monitor)
    eval_events = run_monitor(WORK / "stream_eval.wav", extra, args.monitor)
    null_truth = load_truth("stream_null")
    eval_truth = load_truth("stream_eval")

    # --- null stream: what does a false match look like? --------------------
    null_reports = [e for e in null_events if e["event"] == "report"]
    null_evidence = np.array([e["evidence"] for e in null_reports])
    null_conf = np.array([e["confidence"] for e in null_reports])
    null_verify = np.array([e.get("verify_q", 0.0) for e in null_reports])
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
            row["verify_q"] = det.get("verify_q", 0.0)
            row["verify_r"] = det.get("verify_r", 0.0)
            above = [r.get("verify_q", 0.0) for r in inside if r["confidence"] >= args.threshold]
            row["verify_q_min_above"] = min(above, default=0.0)
            row["verify_q_median_above"] = float(np.median(above)) if above else 0.0
            row["extra_starts"] = len(cands) - 1
            end = ends.get((det["song"], det["t"]))
            row["reported_end"] = end["consumed"] if end else None
        rows.append(row)
    false_starts = [s for s in starts if id(s) not in used]
    # A false start inside a play of the *other* song or in filler music.
    summary = dict(
        half=args.half, threshold=args.threshold, window=args.window, args=extra,
        commit=git_commit(),
        label=args.label or git_commit()[:7],
        monitor=args.monitor or "target/release/monitor",
        stream=next(e for e in eval_events if e["event"] == "stream"),
        null=dict(seconds=null_done["audio_seconds"], reports=len(null_reports), max_evidence=int(null_evidence.max()),
                  p999_evidence=float(np.percentile(null_evidence, 99.9)), median_evidence=float(np.median(null_evidence)),
                  max_confidence=float(null_conf.max()), false_alarms=len(null_starts),
                  max_verify_q=float(null_verify.max()), p999_verify_q=float(np.percentile(null_verify, 99.9)),
                  realtime_fraction=null_done["realtime_fraction"],
                  hash_delay_median_seconds=null_done.get("hash_delay_median_seconds"),
                  hash_delay_max_seconds=null_done.get("hash_delay_max_seconds")),
        eval=dict(seconds=done["audio_seconds"], plays=len(plays), detected=sum(r["detected"] for r in rows),
                  false_starts=len(false_starts), realtime_fraction=done["realtime_fraction"],
                  hash_delay_median_seconds=done.get("hash_delay_median_seconds"),
                  index=[e for e in eval_events if e["event"] in ("index", "index_done")]),
        plays=rows,
        worst=worst_cases(rows, false_starts, reports, eval_truth),
    )
    if args.negatives:
        for name in ("stream_null2", "stream_hard"):
            if (WORK / f"{name}.wav").exists():
                summary[name] = negatives(name, extra, args.monitor, args.threshold)
    json.dump(summary, open(WORK / f"{args.out}.json", "w"), indent=2)

    # --- table --------------------------------------------------------------
    print(f"\nnull stream: {null_done['audio_seconds'] / 60:.1f} min, evidence max {null_evidence.max()}, "
          f"99.9 % {np.percentile(null_evidence, 99.9):.0f}, median {np.median(null_evidence):.0f}; "
          f"max confidence {null_conf.max():.1f}; verify_q max {null_verify.max():.2f}, 99.9 % {np.percentile(null_verify, 99.9):.2f}; "
          f"false alarms at {args.threshold}: {len(null_starts)}; "
          f"CPU {100 * null_done['realtime_fraction']:.2f} % of one core")
    print(f"eval stream: {done['audio_seconds'] / 60:.1f} min, {sum(r['detected'] for r in rows)}/{len(plays)} plays detected, "
          f"{len(false_starts)} false starts, CPU {100 * done['realtime_fraction']:.2f} %"
          + (f"; hash delay median {done['hash_delay_median_seconds']:.2f} s, max {done['hash_delay_max_seconds']:.2f} s"
             if "hash_delay_median_seconds" in done else ""))
    print("\n| Song | Treatment | Detected after | Confidence at detection / max | Shift expected / detected | Tempo expected / detected | Position error | verify_q at detection / min / median while above |")
    print("| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |")
    for r in rows:
        if r["detected"]:
            print(f"| {r['source']} | {r['treatment']} | {r['latency']:.1f} s | {r['confidence']:.0f} / {r['max_confidence']:.0f} | "
                  f"{r['expected_shift']:+d} / {r['shift']:+d} | ×{r['expected_tempo']:.3f} / ×{r['tempo']:.3f} | {r['position_error']:+.1f} s | "
                  f"{r['verify_q']:.2f} / {r['verify_q_min_above']:.2f} / {r['verify_q_median_above']:.2f} |")
        else:
            print(f"| {r['source']} | {r['treatment']} | missed | — | {r['expected_shift']:+d} / — | ×{r['expected_tempo']:.3f} / — | — | — |")

    for name in ("stream_null2", "stream_hard"):
        if name in summary:
            print_negatives(name, summary[name])

    if args.no_plots:
        return summary

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
    ax.set_xlabel("seconds from the start of the play to the detection (pipeline delay included)")
    ax.set_title("Time to detection per play", fontsize=10)
    ax.set_xlim(0, max(lat) + 4)
    fig.savefig(PLOTS / "radio_detection.png", dpi=90)
    plt.close(fig)
    return summary


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--half", type=float, default=40.0)
    parser.add_argument("--threshold", type=float, default=70.0)
    parser.add_argument("--window", type=float, default=5.0)
    parser.add_argument("--monitor", help="monitor binary to run instead of building the current tree")
    parser.add_argument("--out", default="eval_summary", help="summary name under target/radio (default eval_summary)")
    parser.add_argument("--no-plots", action="store_true", help="skip the figures (for comparison runs)")
    parser.add_argument("--label", help="name of this run in comparisons (default: the short commit of the tree)")
    parser.add_argument("--negatives", action="store_true",
                        help="also run the held-out null stream and the hard negatives (scripts/radio_negatives.py)")
    parser.add_argument("--arg", action="append", default=[],
                        help="extra monitor arguments, e.g. --arg '--fan-out 4' (repeatable)")
    evaluate(parser.parse_args())


if __name__ == "__main__":
    main()
