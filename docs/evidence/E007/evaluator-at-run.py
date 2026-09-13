#!/usr/bin/env python3
"""Reproducible real-mix challenge; no threshold tuning or algorithm edits."""

import argparse
import concurrent.futures
import hashlib
import json
import math
import platform
import re
import shutil
import subprocess
import sys
import time
import wave
from pathlib import Path

import numpy as np

RATE = 44100


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def identity(path):
    return dict(path=str(Path(path).resolve()), bytes=Path(path).stat().st_size, sha256=sha(path))


def save(path, data):
    Path(path).write_text(json.dumps(data, indent=2, allow_nan=False) + "\n")


def command(cmd, log, timeout=1200):
    start = time.perf_counter()
    result = subprocess.run([str(x) for x in cmd], capture_output=True, timeout=timeout)
    log.append(dict(command=[str(x) for x in cmd], returncode=result.returncode,
                    elapsed_seconds=time.perf_counter() - start,
                    stderr=result.stderr.decode(errors="replace")[-5000:]))
    if result.returncode:
        raise RuntimeError(f"{cmd}: {result.stderr.decode(errors='replace')[-2000:]}")
    return result.stdout


def ffmpeg(args, log):
    return command(["ffmpeg", "-hide_banner", "-loglevel", "error", "-nostdin", "-y", *args], log)


def wav_info(path):
    with wave.open(str(path)) as f:
        assert f.getnchannels() == 1 and f.getsampwidth() == 2 and f.getframerate() == RATE
        n = f.getnframes()
    return dict(samples=n, seconds=n / RATE, rate=RATE)


def wav_writer(path):
    f = wave.open(str(path), "wb")
    f.setnchannels(1)
    f.setsampwidth(2)
    f.setframerate(RATE)
    return f


def download(item, work):
    name, url = item
    dest = work / f"{name}.mp3"
    headers = work / f"{name}.headers.txt"
    log = []
    resolved = command(["curl", "--fail", "--location", "--silent", "--show-error", "--retry", "2",
                        "--max-time", "300", "--dump-header", headers, "--output", dest,
                        "--write-out", "%{url_effective}", url], log, timeout=1000).decode()
    return name, dict(url=url, resolved_url=resolved, file=identity(dest), command=log[0])


def voice_mix(source, voice, dest):
    # Stream in 30-second blocks; match speech RMS over the exact music
    # samples it overlays. Scale both equally only if needed for headroom.
    with wave.open(str(voice)) as f:
        spoken = np.frombuffer(f.readframes(f.getnframes()), dtype="<i2").astype(np.float64) / 32768
    speech = np.resize(spoken, 12 * RATE)
    records = []
    with wave.open(str(source)) as inp, wav_writer(dest) as out:
        block = 0
        while data := inp.readframes(30 * RATE):
            music = np.frombuffer(data, dtype="<i2").astype(np.float64) / 32768
            n = min(len(music), len(speech))
            mr = float(np.sqrt(np.mean(music[:n] ** 2)))
            sr = float(np.sqrt(np.mean(speech[:n] ** 2)))
            overlay = speech[:n] * mr / max(sr, 1e-12)
            music[:n] += overlay
            scale = min(1.0, 0.95 / max(float(np.max(np.abs(music))), 1e-12))
            out.writeframes(np.rint(music * scale * 32767).astype("<i2").tobytes())
            records.append(dict(start=block * 30, end=block * 30 + n / RATE,
                                music_rms_before=mr, speech_rms_before_scale=float(np.sqrt(np.mean(overlay ** 2))),
                                common_headroom_scale=scale))
            block += 1
    return records


def prepare(plan, work, evidence):
    log = []
    media = work / "media"
    media.mkdir(exist_ok=True)
    items = [("mix", plan["mix_url"])] + [(t["id"], t["url"]) for t in plan["tracks"]]
    downloads = {}
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(download, item, media) for item in items]
        for future in concurrent.futures.as_completed(futures):
            name, record = future.result()
            downloads[name] = record
            print(f"Downloaded {name}: {record['file']['bytes']} bytes", flush=True)
            save(evidence / "downloads.json", downloads)
    references = {"short": {}, "full": {}}
    clips = []
    for track in plan["tracks"]:
        name = track["id"]
        full, short = media / f"{name}-full.wav", media / f"{name}-10s.wav"
        ffmpeg(["-i", media / f"{name}.mp3", "-ac", "1", "-ar", str(RATE), "-c:a", "pcm_s16le", full], log)
        info = wav_info(full)
        start = (info["samples"] - 10 * RATE) // 2
        if start < 0:
            raise AssertionError(f"{name}: original is shorter than ten seconds")
        with wave.open(str(full)) as source, wav_writer(short) as target:
            source.setpos(start)
            pcm = source.readframes(10 * RATE)
            assert len(pcm) == 20 * RATE
            target.writeframes(pcm)
        references["short"][name] = str(short)
        references["full"][name] = str(full)
        clips.append(dict(**track, source_seconds=info["seconds"], clip_start=start / RATE,
                          clip_seconds=10, full=identity(full), short=identity(short)))
    save(evidence / "references.json", clips)

    voice_text = evidence / "voice.txt"
    voice_text.write_text("You are listening to an experimental music programme. This spoken announcement tests recognition while the music changes pitch and tempo. The broadcast continues with a selection of electronic tracks. Please stay tuned for more music after this announcement. ")
    voice = media / "voice.wav"
    ffmpeg(["-f", "lavfi", "-i", f"flite=textfile={voice_text}:voice=slt", "-ac", "1", "-ar", str(RATE),
            "-c:a", "pcm_s16le", voice], log)

    streams = {}
    for name, filt in [("clean", "volume=0.25"),
                       ("pitch", f"volume=0.25,rubberband=pitch={2 ** (2 / 12):.15f}"),
                       ("tempo", "volume=0.25,rubberband=tempo=0.9"),
                       ("combined_music", f"volume=0.25,rubberband=tempo=0.9:pitch={2 ** (2 / 12):.15f}")]:
        path = media / f"stream-{name}.wav"
        print(f"Rendering {name}", flush=True)
        ffmpeg(["-i", media / "mix.mp3", "-ac", "1", "-ar", str(RATE), "-af", filt,
                "-c:a", "pcm_s16le", path], log)
        streams[name] = str(path)
    combined = media / "stream-combined.wav"
    speech_spans = voice_mix(streams.pop("combined_music"), voice, combined)
    streams["combined"] = str(combined)
    save(evidence / "voiceover.json", dict(voice=identity(voice), spans=speech_spans,
                                         period_seconds=30, duration_seconds=12, target_ratio_db=0))
    clean_seconds = wav_info(streams["clean"])["seconds"]
    assert abs(wav_info(streams["pitch"])["seconds"] - clean_seconds) < 0.1
    for name in ("tempo", "combined"):
        assert abs(wav_info(streams[name])["seconds"] - clean_seconds / 0.9) < 0.2

    # Independent positive control: each known original snippet once.
    control = media / "stream-self.wav"
    truth = []
    with wav_writer(control) as out:
        cursor = 0
        for name, ref in references["short"].items():
            out.writeframes(bytes(5 * RATE * 2))
            cursor += 5
            with wave.open(ref) as f:
                out.writeframes(f.readframes(f.getnframes()))
            truth.append(dict(song=name, start=cursor, end=cursor + 10))
            cursor += 10
            out.writeframes(bytes(10 * RATE * 2))
            cursor += 10
    streams["self"] = str(control)
    save(evidence / "self-truth.json", truth)
    negative = media / "stream-negative.wav"
    with wave.open(str(voice)) as src:
        v = src.readframes(12 * RATE)
    with wav_writer(negative) as out:
        for _ in range(10):
            out.writeframes(v)
            out.writeframes(bytes(30 * RATE * 2 - len(v)))
    streams["negative"] = str(negative)
    # Small attributed listening samples in the artifact; full mixes stay out.
    for name in ("clean", "combined"):
        ffmpeg(["-ss", "60", "-i", streams[name], "-t", "30", "-c:a", "libmp3lame", "-b:a", "128k",
                evidence / f"sample-{name}.mp3"], log)
    save(evidence / "streams.json", {k: dict(**identity(p), **wav_info(p)) for k, p in streams.items()})
    save(evidence / "preparation-commands.json", log)
    return references, streams, truth


def parse_events(path, baseline, seconds):
    events = []
    for line in Path(path).read_text().splitlines():
        if baseline and line.startswith('{"event":"stream",'):
            # Only repair the known PR3 duration-string truncation for parsing.
            expected = f"{seconds:.2f}"
            line, count = re.subn(r'("seconds":)' + re.escape(expected[:2]) + r'(?=,)',
                                  lambda m: m[1] + expected, line)
            assert count == 1, "unexpected baseline JSON defect"
        event = json.loads(line, parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x)))
        events.append(event)
    assert events and events[-1]["event"] == "done"
    return events


def stable_digest(events):
    h = hashlib.sha256()
    for item in events:
        item = item.copy()
        if item["event"] == "index_done":
            item.pop("seconds")
        if item["event"] == "done":
            item.pop("cpu_seconds")
            item.pop("realtime_fraction")
        h.update(json.dumps(item, sort_keys=True).encode() + b"\n")
    return h.hexdigest()


def evaluate(binaries, references, streams, truth, evidence):
    runs = []
    cases = [("short", name) for name in ("self", "negative", "clean", "pitch", "tempo", "combined")]
    cases += [("full", name) for name in ("clean", "combined")]
    for i, (mode, name) in enumerate(cases):
        pair = []
        for side in (("baseline", "candidate") if i % 2 == 0 else ("candidate", "baseline")):
            cmd = [str(binaries[side])]
            for song, ref in references[mode].items():
                cmd += ["--watch", f"{song}={ref}"]
            cmd += ["--stream", streams[name]]
            prefix = evidence / f"{mode}-{name}-{side}"
            print(f"Running {mode}/{name}/{side}", flush=True)
            start = time.perf_counter()
            with prefix.with_suffix(".jsonl").open("wb") as stdout, prefix.with_suffix(".stderr.txt").open("wb") as stderr:
                proc = subprocess.run(cmd, stdout=stdout, stderr=stderr, timeout=600)
            elapsed = time.perf_counter() - start
            if proc.returncode:
                raise RuntimeError(f"monitor failed: {prefix} exit {proc.returncode}")
            events = parse_events(prefix.with_suffix(".jsonl"), side == "baseline", wav_info(streams[name])["seconds"])
            starts = [e for e in events if e["event"] == "start"]
            reports = [e for e in events if e["event"] == "report"]
            tracks = {}
            for song in references[mode]:
                hits = [s for s in starts if s["song"] == song]
                if name == "self":
                    interval = next(x for x in truth if x["song"] == song)
                    hits = [s for s in hits if interval["start"] <= s["consumed"] <= interval["end"] + 1]
                candidates = [e for e in reports if e["song"] == song]
                tracks[song] = dict(detected=bool(hits), starts=len(hits), first_start=hits[0] if hits else None,
                                    max_confidence=max((e["confidence"] for e in candidates), default=0),
                                    max_verify_q=max((e.get("verify_q", 0) for e in candidates), default=0))
            row = dict(mode=mode, treatment=name, side=side, command=cmd, returncode=proc.returncode,
                       elapsed_seconds=elapsed, done=events[-1], starts=starts, tracks=tracks,
                       detected=sum(t["detected"] for t in tracks.values()),
                       stdout=identity(prefix.with_suffix(".jsonl")), stderr=identity(prefix.with_suffix(".stderr.txt")),
                       event_digest=stable_digest(events))
            runs.append(row)
            pair.append(row)
            save(evidence / "results.json", runs)
            print(f"RESULT {mode}/{name}/{side}: {row['detected']}/22 tracks, {len(starts)} starts, {elapsed:.3f}s wall", flush=True)
        if pair[0]["event_digest"] != pair[1]["event_digest"]:
            raise AssertionError(f"non-timing events differ: {mode}/{name}")
    return runs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--plan", default="experiments/toucan2020.json")
    parser.add_argument("--work", default="target/real-mix")
    args = parser.parse_args()
    work = Path(args.work).resolve()
    evidence = work / "evidence"
    evidence.mkdir(parents=True, exist_ok=True)
    plan = json.loads(Path(args.plan).read_text())
    save(evidence / "plan.json", plan)
    binaries = {s: Path(getattr(args, s)).resolve(strict=True) for s in ("baseline", "candidate")}
    metadata = dict(status="running", plan=identity(args.plan), evaluator=identity(__file__),
                    binaries={s: identity(p) for s, p in binaries.items()}, platform=platform.platform(),
                    python=sys.version, numpy=np.__version__, source_page=plan["source_page"])
    (evidence / "ATTRIBUTION.txt").write_text(
        "Toucan Music 2005 to 2020, Various Artists, Toucan Music.\n" + plan["source_page"] + "\n" +
        "CC BY-NC-SA 4.0: " + plan["license"] + "\n" +
        "Samples modified for a noncommercial recognition experiment: mono conversion, level reduction;\n"
        "combined sample additionally +2 semitones, 0.9 tempo and synthetic voiceover.\n" +
        "\n".join(f"{t['id']}: {t['artist']} — {t['title']}" for t in plan["tracks"]) + "\n")
    try:
        references, streams, truth = prepare(plan, work, evidence)
        runs = evaluate(binaries, references, streams, truth, evidence)
        metadata.update(status="completed", all_event_parity=True, runs=len(runs))
    except Exception as error:
        metadata.update(status="failed", error=repr(error))
        raise
    finally:
        save(evidence / "metadata.json", metadata)


if __name__ == "__main__":
    main()
