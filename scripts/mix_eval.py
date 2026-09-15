#!/usr/bin/env python3
"""Evaluate the watch-list monitor on a real 90-minute, 22-track DJ mix.

Measures four things with the 22 tracks of the Toucan Music 2020 mix as the
watch list:

  * the full mix: how many of the 22 songs produce a `start` event somewhere
    near where they are actually mixed in, and how many starts land outside
    those bounds ("wrong starts");
  * 22 frozen 10 s excerpts cut out of the mix itself, each surrounded by
    silence, which is the hardest case: the excerpt is a DJ blend of two
    tracks at an unknown pitch and tempo;
  * 22 clean 10 s excerpts cut out of the original releases;
  * 16 negatives that are not in the watch list, both as 10 s excerpts and as
    complete tracks (about an hour of audio), where any `start` is a false
    alarm.

    python3 scripts/mix_eval.py --prepare     # once, downloads and decodes
    python3 scripts/mix_eval.py --run

`--prepare` writes everything under `target/mix/`. It needs `curl` and
`ffmpeg` on the path, about 2 GB of disk for the decoded 44.1 kHz mono WAV
files, and it skips anything it has already produced, so it can be re-run
after a failed download. The mix and its tracks are published by Toucan
Music under CC BY-NC-SA 4.0 (see `experiments/toucan2020.json` for the
per-track sources and `experiments/recognition-holdout.json` for the
negatives); none of the audio is redistributed here.

Caveat on the cue labels in `CUES` below: they come from an automatic
alignment of each track against the mix, not from human annotation, and a DJ
mix has no per-second truth about where one track stops and the next starts.
Scoring "wrong starts" against those labels therefore counts genuine layered
overlaps as wrong. The wrong starts this reports on the mix (t04 at about
2077 s, t21 at about 4005 s and about 4222 s) are real audio: aligning the
peaks of those tracks against the mix there matches 25 to 35 % of them,
against about 5 % anywhere else. The DJ brings them back under a later track.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import time
import wave
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
EXPERIMENTS = ROOT / "experiments"
WORK = ROOT / "target" / "mix"
MEDIA = WORK / "media"
CLIPS = WORK / "clips"
RUNS = WORK / "runs"

RATE = 44100
CLIP_SECONDS = 10.0
CLIP_SAMPLES = int(CLIP_SECONDS * RATE)
SONGS = [f"t{i:02d}" for i in range(1, 23)]
NEGATIVES = [f"h{i:02d}" for i in range(1, 9)] + [f"n{i:02d}" for i in range(1, 9)]
# Silence before and after each item of a programme.
PRE, POST = 7.0, 9.0

# Where each track starts in the mix, in seconds, as the start of its frozen
# 10 s excerpt. These come from an automatic spectrogram alignment of every
# track against the mix, not from human annotation or from this monitor: the
# mix carries no per-second truth about track boundaries, and neighbouring
# tracks are layered over each other for tens of seconds around every cue.
CUES = {
    "t01": 37.0,
    "t02": 218.5,
    "t03": 406.5,
    "t04": 648.5,
    "t05": 835.0,
    "t06": 971.5,
    "t07": 1224.0,
    "t08": 1508.5,
    "t09": 1763.5,
    "t10": 1980.5,
    "t11": 2266.5,
    "t12": 2604.0,
    "t13": 2874.5,
    "t14": 3117.0,
    "t15": 3404.5,
    "t16": 3653.0,
    "t17": 3892.5,
    "t18": 4108.5,
    "t19": 4349.0,
    "t20": 4615.0,
    "t21": 4957.0,
    "t22": 5274.5,
}


def read_wav(path: Path, start: int = 0, count: int | None = None) -> np.ndarray:
    with wave.open(str(path)) as w:
        if start:
            w.setpos(start)
        frames = w.readframes(w.getnframes() - start if count is None else count)
    return np.frombuffer(frames, dtype=np.int16)


def write_wav(path: Path, samples: np.ndarray) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(samples.astype(np.int16).tobytes())


def wav_samples(path: Path) -> int:
    with wave.open(str(path)) as w:
        return w.getnframes()


def download(url: str, dest: Path) -> None:
    if dest.exists():
        return
    print(f"download {dest.name}", flush=True)
    partial = dest.with_suffix(dest.suffix + ".part")
    subprocess.run(["curl", "--fail", "--location", "--retry", "2", "--silent", "--show-error",
                    "--output", str(partial), url], check=True)
    partial.rename(dest)


def decode(src: Path, dest: Path) -> None:
    """Decode to 44.1 kHz mono signed 16-bit WAV, the monitor's input format."""
    if dest.exists():
        return
    print(f"decode {dest.name}", flush=True)
    partial = dest.with_suffix(".part.wav")
    subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-y", "-i", str(src),
                    "-ac", "1", "-ar", str(RATE), "-c:a", "pcm_s16le", str(partial)], check=True)
    partial.rename(dest)


def cut(src: Path, dest: Path, start: int, count: int = CLIP_SAMPLES) -> None:
    if dest.exists():
        return
    write_wav(dest, read_wav(src, start, count))


def prepare() -> None:
    for d in (MEDIA, CLIPS, RUNS):
        d.mkdir(parents=True, exist_ok=True)
    mix = json.load(open(EXPERIMENTS / "toucan2020.json"))
    holdout = json.load(open(EXPERIMENTS / "recognition-holdout.json"))

    download(mix["mix_url"], MEDIA / "mix.mp3")
    decode(MEDIA / "mix.mp3", MEDIA / "mix-full.wav")
    for song, cue in CUES.items():
        cut(MEDIA / "mix-full.wav", CLIPS / f"frozen-{song}.wav", int(cue * RATE))

    for track in mix["tracks"] + holdout["tracks"]:
        name = track["id"]
        download(track["url"], MEDIA / f"{name}.mp3")
        decode(MEDIA / f"{name}.mp3", MEDIA / f"{name}-full.wav")
        # Ten seconds centred on the recording's midpoint.
        midpoint = (wav_samples(MEDIA / f"{name}-full.wav") - CLIP_SAMPLES) // 2
        cut(MEDIA / f"{name}-full.wav", MEDIA / f"{name}-10s.wav", max(midpoint, 0))
    print(f"prepared {MEDIA}", flush=True)


def programme(name: str, items: list[tuple[str, np.ndarray]]) -> tuple[Path, list]:
    """One WAV with every item separated by silence, plus its ground truth.

    Returns the path and a list of (label, start_seconds, end_seconds).
    """
    path, truth_path = CLIPS / f"{name}.wav", CLIPS / f"{name}.json"
    if path.exists() and truth_path.exists():
        return path, json.load(open(truth_path))
    out: list[np.ndarray] = []
    truth = []
    t = 0.0
    for label, samples in items:
        out.append(np.zeros(int(PRE * RATE), np.int16))
        t += PRE
        out.append(samples)
        truth.append((label, t, t + len(samples) / RATE))
        t += len(samples) / RATE
        out.append(np.zeros(int(POST * RATE), np.int16))
        t += POST
    write_wav(path, np.concatenate(out))
    json.dump(truth, open(truth_path, "w"))
    return path, truth


def build_programmes() -> dict[str, tuple[Path, list]]:
    return {
        "frozen": programme("prog-frozen", [(s, read_wav(CLIPS / f"frozen-{s}.wav")) for s in SONGS]),
        "clean": programme("prog-clean", [(s, read_wav(MEDIA / f"{s}-10s.wav")) for s in SONGS]),
        "neg": programme("prog-neg", [(s, read_wav(MEDIA / f"{s}-10s.wav")) for s in NEGATIVES]),
        "negfull": programme("prog-negfull", [(s, read_wav(MEDIA / f"{s}-full.wav")) for s in NEGATIVES]),
    }


def monitor_binary(given: str | None) -> str:
    if given:
        return given
    binary = ROOT / "target" / "release" / "monitor"
    if not binary.exists():
        subprocess.run(["cargo", "build", "--release", "--quiet", "-p", "cqt-monitor"],
                       cwd=ROOT, check=True)
    return str(binary)


def run_monitor(binary: str, flags: list[str], stream: Path, out: Path) -> float:
    cmd = [binary]
    for song in SONGS:
        cmd += ["--watch", f"{song}={MEDIA.relative_to(ROOT)}/{song}-full.wav"]
    cmd += ["--stream", str(stream), *flags]
    started = time.time()
    with open(out, "w") as f:
        result = subprocess.run(cmd, cwd=ROOT, stdout=f, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        raise SystemExit(f"{out}: rc={result.returncode}\n{result.stderr[-2000:]}")
    return time.time() - started


def events(path: Path) -> list[dict]:
    keep = ('"event":"start"', '"event":"end"', '"index_done"')
    return [json.loads(line) for line in open(path) if any(k in line for k in keep)]


def score_programme(path: Path, truth: list, negative: bool = False) -> tuple[int, int, list]:
    starts = [e for e in events(path) if e["event"] == "start"]
    hits = 0
    wrong = 0
    detail = []
    for label, a, b in truth:
        inside = [e for e in starts if a - 1 <= e["t"] <= b + POST]
        songs = {e["song"] for e in inside}
        if negative:
            wrong += len(songs)
            detail.append((label, sorted(songs)))
        else:
            ok = label in songs
            hits += ok
            wrong += len(songs - {label})
            detail.append((label, ok, sorted(songs - {label})))
    return hits, wrong, detail


def score_fullmix(path: Path) -> tuple[set[str], list, int]:
    """A start counts for a song if it falls between the previous cue plus 10 s
    and the next cue, with 30 s of slack on both sides: generous, because the
    cue labels are automatic and tracks are layered around every transition."""
    starts = [e for e in events(path) if e["event"] == "start"]
    bounds = {}
    for i, song in enumerate(SONGS):
        lo = CUES[SONGS[i - 1]] + 10 if i > 0 else 0
        hi = CUES[SONGS[i + 1]] if i + 1 < len(SONGS) else 1e9
        bounds[song] = (lo, hi)
    found: set[str] = set()
    wrong = []
    for e in starts:
        lo, hi = bounds[e["song"]]
        if lo - 30 <= e["t"] <= hi + 30:
            found.add(e["song"])
        else:
            wrong.append((round(e["t"]), e["song"]))
    return found, wrong, len(starts)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--prepare", action="store_true", help="download and decode the corpus")
    ap.add_argument("--run", action="store_true", help="run the monitor and score it")
    ap.add_argument("--bin", help="monitor binary [target/release/monitor]")
    ap.add_argument("--flags", default="", help="extra monitor flags, as one string")
    ap.add_argument("--tag", default="default", help="name of the run directory under target/mix/runs")
    ap.add_argument("--skip-fullmix", action="store_true", help="score the programmes only")
    ap.add_argument("--verbose", action="store_true", help="list the misses and the false starts")
    args = ap.parse_args()
    if not args.prepare and not args.run:
        ap.error("nothing to do: pass --prepare, --run, or both")
    if args.prepare:
        prepare()
    if not args.run:
        return

    binary = monitor_binary(args.bin)
    flags = args.flags.split()
    progs = build_programmes()
    out_dir = RUNS / args.tag
    out_dir.mkdir(parents=True, exist_ok=True)
    jobs = {k: (progs[k][0], out_dir / f"{k}.jsonl") for k in progs}
    if not args.skip_fullmix:
        jobs["fullmix"] = (MEDIA / "mix-full.wav", out_dir / "fullmix.jsonl")
    with ThreadPoolExecutor(6) as pool:
        times = dict(zip(jobs, pool.map(lambda job: run_monitor(binary, flags, *job), jobs.values())))

    res = {}
    for k in ("frozen", "clean"):
        res[k] = score_programme(jobs[k][1], progs[k][1])
    for k in ("neg", "negfull"):
        hits, wrong, detail = score_programme(jobs[k][1], progs[k][1], negative=True)
        res[k] = (0, wrong, detail)
    line = (f"[{args.tag}] flags='{args.flags}'"
            f" | clean {res['clean'][0]}/22 (wrong {res['clean'][1]})"
            f" | frozen {res['frozen'][0]}/22 (wrong {res['frozen'][1]})"
            f" | neg10s false {res['neg'][1]} | negfull false {res['negfull'][1]}")
    if not args.skip_fullmix:
        found, wrong, n = score_fullmix(jobs["fullmix"][1])
        index = [e for e in events(jobs["fullmix"][1]) if e["event"] == "index_done"][0]
        line += (f" | FULLMIX {len(found)}/22 missing={sorted(set(SONGS) - found)}"
                 f" wrong_starts={len(wrong)} starts={n}"
                 f" idx_bytes={index['bytes'] // 1_000_000}MB t={times['fullmix']:.0f}s")
        if args.verbose:
            print("  fullmix wrong:", wrong)
    print(line, flush=True)
    if args.verbose:
        for k in ("frozen", "clean"):
            print(" ", k, "misses:", [x[0] for x in res[k][2] if not x[1]],
                  "wrong:", [(x[0], x[2]) for x in res[k][2] if x[2]])
        for k in ("neg", "negfull"):
            print(" ", k, "false:", [x for x in res[k][2] if x[1]])
    json.dump({"tag": args.tag, "flags": args.flags, "line": line},
              open(out_dir / "summary.json", "w"))


if __name__ == "__main__":
    main()
