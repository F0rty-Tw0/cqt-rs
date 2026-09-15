#!/usr/bin/env python3
"""A simulated DJ set for the watch-list monitor: the hardest case we
care about, every play with several treatments at once.

`scripts/radio_sim.py` covers each DJ treatment on its own. This stream
(`stream_dj.wav` + `.json`) is a continuous mix with no silence: every
item crossfades into the previous one over 6 s, the watched songs are
played with a key-locked BPM change *and* a key change *and* 12 s of DJ
talk-over at the same time, in some plays through the FM chain and an
MP3 round trip as well, and unwatched tracks and dry DJ speech sit in
between. Treatment names spell out the combination:

    dj_tempo_+8_key_-1_talkover          BPM +8 % (key lock), −1 semitone, talk-over
    dj_tempo_-6_key_+1_talkover          BPM −6 %, +1 semitone, talk-over
    dj_fader_+5_key_-2                   pitch fader +5 % then −2 semitones
    dj_tempo_+12_key_+2                  BPM +12 %, +2 semitones
    dj_fader_-7_talkover_fm_mp3          pitch fader −7 %, talk-over, FM chain, MP3
    dj_tempo_+8_key_-1_talkover_fm_mp3   everything at once

Run after radio_sim.py:

    python3 scripts/radio_dj.py
    python3 scripts/radio_eval.py --programme stream_dj --out eval_dj
"""
from __future__ import annotations

import math
import os
import sys
import tempfile
import time
from typing import List

import numpy as np

import radio_sim as rs
from radio_negatives import build_rendered_stream

DJ_SEED = 2024

COMBOS = [
    # (name, tempo (key-locked), fader percent, key semitones, talkover, fm_mp3)
    ("dj_tempo_+8_key_-1_talkover", 1.08, 0, -1, True, False),
    ("dj_tempo_-6_key_+1_talkover", 0.94, 0, +1, True, False),
    ("dj_fader_+5_key_-2", 1.0, +5, -2, False, False),
    ("dj_tempo_+12_key_+2", 1.12, 0, +2, False, False),
    ("dj_fader_-7_talkover_fm_mp3", 1.0, -7, 0, True, True),
    ("dj_tempo_+8_key_-1_talkover_fm_mp3", 1.08, 0, -1, True, True),
]


def render_combo(rng: np.random.Generator, sid: str, combo) -> rs.Rendered:
    name, tempo, fader, key, talkover, fm_mp3 = combo
    start = rs.watched_start(sid, rng, False)
    x = rs.excerpt(sid, start, rs.WATCH_PLAY_S)
    speed = 1.0 + fader / 100.0
    if fader:
        x = rs.pitch_fader(x, fader)
    if tempo != 1.0:
        x = rs.keylock_tempo(x, tempo)
    if key:
        x = rs.key_change(x, key)
    x = rs.level_match(x, rs.MUSIC_RMS_DBFS)
    extra = {}
    if talkover:
        sp_sid = str(rng.choice(rs.SPEECH))
        sp_start = round(float(rng.uniform(0.0, max(0.0, rs.source_duration(sp_sid) - rs.TALKOVER_S))), 3)
        sp = rs.excerpt(sp_sid, sp_start, rs.TALKOVER_S)
        x = rs.talkover_mix(x, sp)
        extra = {"talkover_source": sp_sid, "talkover_start": sp_start,
                 "talkover_duration": min(rs.TALKOVER_S, len(sp) / rs.SR)}
    if fm_mp3:
        x = rs.level_match(rs.mp3_roundtrip(rs.fm_chain(x)), rs.MUSIC_RMS_DBFS)
    item = rs.PlanItem(sid, name, round(start, 3), rs.WATCH_PLAY_S, "music", crossfade=True)
    semitones = 12.0 * math.log2(speed) + key
    return rs.Rendered(item, x.astype(np.float32), speed, speed * tempo, semitones, extra)


def build_dj_set(rng: np.random.Generator) -> List[rs.Rendered]:
    def filler(sid: str, treatment: str = "clean") -> rs.Rendered:
        item = rs.filler_item(rng, sid, treatment, (25.0, 35.0))
        item.crossfade = True
        return rs.render(item)

    def speech() -> rs.Rendered:
        item = rs.speech_item(rng)
        item.crossfade = True
        return rs.render(item)

    out = [filler("choice", "keylock_tempo_+10")]
    plays = [("vibe_ace", c) for c in COMBOS] + [("sweet_waltz", COMBOS[0]), ("sweet_waltz", COMBOS[2]),
                                                  ("sweet_waltz", COMBOS[5])]
    rng.shuffle(plays)
    fillers = ["sugar_plum", "brahms", "fishin", "pistachio", "choice"]
    for i, (sid, combo) in enumerate(plays):
        out.append(render_combo(rng, sid, combo))
        if i % 3 == 2:
            out.append(speech())
        out.append(filler(fillers[i % len(fillers)], str(rng.choice(["clean", "pitch_fader_+6", "key_change_-1"]))))
    return out


def main():
    os.makedirs(rs.SCRATCH_PARENT, exist_ok=True)
    scratch = tempfile.TemporaryDirectory(prefix="radio_dj_", dir=rs.SCRATCH_PARENT)
    rs.SCRATCH = scratch.name
    t0 = time.time()
    rng = np.random.default_rng(DJ_SEED)
    rendered = build_dj_set(rng)
    gt = build_rendered_stream("stream_dj", rendered, DJ_SEED, watched=True)
    rs.print_summary("stream_dj", gt)
    scratch.cleanup()
    rs.SCRATCH = None
    print(f"total {time.time() - t0:.1f} s")


if __name__ == "__main__":
    sys.exit(main())
