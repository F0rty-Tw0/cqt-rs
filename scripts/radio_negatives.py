#!/usr/bin/env python3
"""Held-out null material and hard negatives for the watch-list monitor.

`scripts/radio_sim.py` builds the calibration null stream (seed 4321) on
which `half` is tuned. Evaluating the false-alarm rate on the same audio
would be optimistic, so this script builds two more streams that contain
no play of a watched song:

  stream_null2.wav + .json   >= 30 min drawn like the calibration null
                             stream but with another seed: the held-out
                             null set
  stream_hard.wav + .json    hard negatives, each labelled by family:
                               same_artist   the other Kevin MacLeod track
                                             (sugar_plum) under every
                                             treatment
                               reversed      watched excerpts played
                                             backwards (same spectra,
                                             same key, no play)
                               loop_Ns       a 0.5, 1 or 2 s sample of a
                                             watched song looped for 30 s,
                                             alone or 6 dB under another
                                             track (a sampled loop is not
                                             a play of the song)
                               out_of_range  a watched song at 0.6x and
                                             1.5x speed, outside the
                                             0.7-1.4 tempo range the
                                             matcher is asked to cover

Both streams go through the same FM chain. Run after radio_sim.py:

    python3 scripts/radio_negatives.py
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
import time
from typing import List

import numpy as np
import soundfile as sf

import radio_sim as rs

NULL2_SEED = 7777
HARD_SEED = 99


def loop_audio(sid: str, start: float, loop_s: float, total_s: float, rng) -> np.ndarray:
    piece = rs.excerpt(sid, start, loop_s)
    reps = int(np.ceil(total_s * rs.SR / len(piece)))
    return np.tile(piece, reps)[: int(total_s * rs.SR)]


def build_hard_plan(rng: np.random.Generator) -> List[rs.Rendered]:
    """Rendered segments in programme order; families in the treatment name."""
    out: List[rs.Rendered] = []

    def add(source: str, family: str, audio: np.ndarray, excerpt_start: float, duration: float, **extra):
        item = rs.PlanItem(source, family, round(excerpt_start, 3), round(duration, 3), "music")
        audio = rs.level_match(audio.astype(np.float32), rs.MUSIC_RMS_DBFS)
        r = rs.Rendered(item, audio, 1.0, 1.0, 0.0, dict(family=family.split("/")[0], **extra))
        out.append(r)

    # Same artist, every treatment (including talk-over and crossfade
    # neighbours, which the assembler handles from the treatment name).
    for t in rs.TREATMENTS:
        if t == "crossfade":
            continue
        item = rs.filler_item(rng, "sugar_plum", t, (30.0, 40.0))
        r = rs.render(item)
        r.item.treatment = "same_artist/" + t
        r.extra.update(family="same_artist")
        out.append(r)
        if rng.random() < 0.4:
            out.append(rs.render(rs.speech_item(rng)))

    # Reversed watched excerpts, clean and pitch-fadered.
    for sid in rs.WATCHED:
        for t in ("clean", "pitch_fader_+6", "key_change_-1"):
            start = rs.watched_start(sid, rng, False)
            x = rs.excerpt(sid, start, rs.WATCH_PLAY_S)[::-1].copy()
            if t == "pitch_fader_+6":
                x = rs.pitch_fader(x, +6)
            elif t == "key_change_-1":
                x = rs.key_change(x, -1)
            add(sid, "reversed/" + t, x, start, rs.WATCH_PLAY_S)
        out.append(rs.render(rs.speech_item(rng)))

    # Loops of a watched song: alone, and 6 dB under another track.
    for sid in rs.WATCHED:
        for loop_s in (0.5, 1.0, 2.0):
            start = rs.watched_start(sid, rng, False)
            loop = loop_audio(sid, start, loop_s, 30.0, rng)
            add(sid, f"loop_{loop_s:g}s/alone", loop, start, loop_s, loop_seconds=loop_s)
            bed_sid = str(rng.choice([s for s in rs.OTHER_MUSIC if s != "trumpet"]))
            bed_item = rs.filler_item(rng, bed_sid, "clean", (30.0, 30.0))
            bed = rs.render(bed_item).audio
            n = min(len(bed), len(loop))
            mix = bed[:n] + rs.db_to_lin(-6.0) * rs.level_match(loop[:n], rs.MUSIC_RMS_DBFS)
            add(sid, f"loop_{loop_s:g}s/under_{bed_sid}", mix, start, loop_s, loop_seconds=loop_s, bed=bed_sid)
        out.append(rs.render(rs.speech_item(rng)))

    # Out of the tempo range: 0.6x and 1.5x speed (pitch fader).
    for sid in rs.WATCHED:
        for pct in (-40, +50):
            start = rs.watched_start(sid, rng, False)
            x = rs.pitch_fader(rs.excerpt(sid, start, rs.WATCH_PLAY_S), pct)
            add(sid, f"out_of_range/pitch_fader_{pct:+d}", x, start, rs.WATCH_PLAY_S, speed=1 + pct / 100)
    out.append(rs.render(rs.filler_item(rng, "trumpet", "clean", (5.0, 6.0))))
    return out


def build_rendered_stream(name: str, rendered: List[rs.Rendered], seed: int):
    t0 = time.time()
    stream, segments = rs.assemble(rendered)
    for seg in segments:
        seg["watched"] = False  # nothing here is a play of a watched song
    stream = rs.fm_chain(stream)
    wav = os.path.join(rs.RADIO_DIR, f"{name}.wav")
    sf.write(wav, stream, rs.SR, subtype="PCM_16")
    gt = {"sample_rate": rs.SR, "duration": len(stream) / rs.SR, "seed": seed,
          "watched_sources": rs.WATCHED, "segments": segments}
    with open(os.path.join(rs.RADIO_DIR, f"{name}.json"), "w") as f:
        json.dump(gt, f, indent=1)
    print(f"  [{name}] {len(stream) / rs.SR / 60:.1f} min, {len(segments)} segments, {time.time() - t0:.1f} s -> {wav}")
    return gt


def main():
    os.makedirs(rs.SCRATCH_PARENT, exist_ok=True)
    scratch = tempfile.TemporaryDirectory(prefix="radio_neg_", dir=rs.SCRATCH_PARENT)
    rs.SCRATCH = scratch.name
    which = sys.argv[1:] or ["null2", "hard"]

    if "hard" in which:
        rng = np.random.default_rng(HARD_SEED)
        gt = build_rendered_stream("stream_hard", build_hard_plan(rng), HARD_SEED)
        rs.print_summary("stream_hard", gt)
    if "null2" in which:
        rng = np.random.default_rng(NULL2_SEED)
        plan = rs.build_null_plan(rng, rs.NULL_MIN_S)
        gt, _, _ = rs.build_stream("stream_null2", plan, NULL2_SEED)
        assert not any(s["watched"] for s in gt["segments"])
        rs.print_summary("stream_null2", gt)
    scratch.cleanup()
    rs.SCRATCH = None


if __name__ == "__main__":
    sys.exit(main())
