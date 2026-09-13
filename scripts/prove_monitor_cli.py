#!/usr/bin/env python3
"""Run real monitor binaries on edge inputs and save replayable JSON proof.

    python3 scripts/prove_monitor_cli.py --candidate target/release/monitor
    python3 scripts/prove_monitor_cli.py --baseline OLD --candidate NEW --output PROOF_DIR

With --baseline, require the specific empty-stream JSON bug in the baseline
and equivalent non-timing events after the fix. Build failures are not proof.
Only the standard library is needed; no downloaded music or mocked detector.
"""

import argparse
import hashlib
import json
import math
import platform
import re
import subprocess
import sys
import wave
from pathlib import Path

RATE = 44100


def identity(path):
    return dict(path=str(path.resolve()), sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def strict_json(line):
    def reject(value):
        raise ValueError(f"non-finite JSON constant: {value}")
    return json.loads(line, parse_constant=reject)


def write_wav(path, pcm):
    with wave.open(str(path), "wb") as output:
        output.setnchannels(1)
        output.setsampwidth(2)
        output.setframerate(RATE)
        output.writeframes(pcm)


def run(binary, watch, source, pcm, prefix):
    command = [str(binary), "--watch", f"fixture={watch}", "--stream", str(source)]
    result = subprocess.run(command, input=pcm, capture_output=True, timeout=30)
    prefix.with_suffix(".stdout.jsonl").write_bytes(result.stdout)
    prefix.with_suffix(".stderr.txt").write_bytes(result.stderr)
    record = dict(command=command, returncode=result.returncode,
                  stdout=identity(prefix.with_suffix(".stdout.jsonl")),
                  stderr=identity(prefix.with_suffix(".stderr.txt")))
    return result, record


def parse_output(result, *, baseline_empty=False):
    if result.returncode != 0:
        raise AssertionError(f"monitor failed with exit {result.returncode}: {result.stderr.decode()}")
    events = []
    reproduced = False
    for line in result.stdout.decode().splitlines():
        try:
            event = strict_json(line)
        except (ValueError, json.JSONDecodeError):
            if not baseline_empty:
                raise
            # Accept only the predicted defect, with the rest of the event
            # valid. An unrelated crash or malformed output cannot pass.
            sanitized, count = re.subn(r'("realtime_fraction":)(?:inf|NaN)(?=,|})', r'\1null', line)
            if count != 1:
                raise AssertionError(f"unexpected baseline JSON defect: {line}")
            event = strict_json(sanitized)
            if event.get("event") != "done" or event.get("audio_seconds") != 0:
                raise AssertionError(f"unexpected baseline failure location: {line}")
            reproduced = True
        if not isinstance(event, dict) or "event" not in event:
            raise AssertionError(f"invalid event: {event}")
        events.append(event)
    done = [event for event in events if event["event"] == "done"]
    if len(done) != 1 or not events or events[-1] != done[0]:
        raise AssertionError("expected exactly one final done event")
    if baseline_empty and not reproduced:
        raise AssertionError("baseline did not reproduce the expected empty-stream JSON defect")
    return events


def stable_events(events):
    # Exclude measured runtime only, retaining frame/peak/hash/match counts
    # and every detection field for a before/after behavior comparison.
    out = []
    for event in events:
        event = dict(event)
        if event["event"] == "index_done":
            event.pop("seconds")
        if event["event"] == "done":
            event.pop("cpu_seconds")
            event.pop("realtime_fraction")
        out.append(event)
    return out


def prove(args, output, report):
    candidate = Path(args.candidate).resolve(strict=True)
    baseline = Path(args.baseline).resolve(strict=True) if args.baseline else None
    report["candidate"] = identity(candidate)
    report["baseline"] = identity(baseline) if baseline else None
    # Silence isolates the CLI framing contract; recognition quality is not
    # being tested. Include sub-block and sub-display-precision durations.
    watch = output / "reference.wav"
    write_wav(watch, bytes(RATE * 2))
    report["reference"] = identity(watch)
    for name, samples, live in [
        ("empty_stdin", 0, True), ("empty_wav", 0, False),
        ("one_sample_stdin", 1, True), ("one_sample_wav", 1, False),
        ("partial_block_stdin", 2053, True), ("nonempty_wav", RATE, False),
    ]:
        pcm = bytes(samples * 2)
        fixture = output / f"{name}.{'pcm' if live else 'wav'}"
        if live:
            fixture.write_bytes(pcm)
        else:
            write_wav(fixture, pcm)
        source = "-" if live else fixture
        case = dict(name=name, samples=samples, input=identity(fixture), status="running")
        report["cases"].append(case)
        if baseline:
            result, case["baseline"] = run(baseline, watch, source, pcm if live else b"",
                                           output / f"{name}-baseline")
            before = parse_output(result, baseline_empty=samples == 0)
            case["baseline"]["expected_json_failure"] = samples == 0
        result, case["candidate"] = run(candidate, watch, source, pcm if live else b"",
                                        output / f"{name}-candidate")
        after = parse_output(result)
        fraction = after[-1]["realtime_fraction"]
        if samples == 0:
            if fraction is not None:
                raise AssertionError(f"{name}: empty duration must have a null fraction")
        elif not isinstance(fraction, (int, float)) or not math.isfinite(fraction) or fraction < 0:
            raise AssertionError(f"{name}: positive duration must have a finite numeric fraction")
        if after[-1]["audio_seconds"] != round(samples / RATE, 2):
            raise AssertionError(f"{name}: wrong audio duration")
        if baseline and stable_events(before) != stable_events(after):
            raise AssertionError(f"{name}: events changed beyond runtime fields")
        case["status"] = "passed"
        print(f"PASS {name}: baseline={'expected JSON failure' if baseline and samples == 0 else 'valid' if baseline else 'not run'}, candidate valid")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--baseline")
    parser.add_argument("--output", default="target/monitor-cli-proof")
    args = parser.parse_args()
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    report = dict(status="running", cases=[], python=sys.version, platform=platform.platform(),
                  verifier=identity(Path(__file__)),
                  scope="CLI JSON and event compatibility on deterministic edge inputs; not recognition or performance")
    try:
        prove(args, output, report)
        report["status"] = "passed"
    except Exception as error:
        report["status"] = "failed"
        report["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        (output / "proof.json").write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")


if __name__ == "__main__":
    main()
