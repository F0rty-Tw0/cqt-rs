#!/usr/bin/env python3
"""
radio_sim.py -- build simulated radio streams for evaluating an audio-fingerprint monitor.

Outputs (all in target/radio/):
  watch_vibe_ace.wav, watch_sweet_waltz.wav   clean full-length watched songs (float WAV, mono 44.1 kHz)
  stream_eval.wav + stream_eval.json          ~12-15 min programme with the watched songs played under
                                              every DJ / broadcast treatment, plus ground truth
  stream_null.wav + stream_null.json          >= 30 min programme that never contains a watched song

Every segment is level-matched (-20 dBFS RMS music, -23 dBFS speech), segments are joined with 0.5 s of
silence (except for "crossfade" plays), and the whole stream goes through an FM broadcast chain
(15 kHz low-pass, 4:1 compressor/limiter, hard clip).  All randomness is seeded.

Run:  python3 scripts/radio_sim.py
"""
from __future__ import annotations

import json
import math
import os
import sys
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional

import numpy as np
import scipy.signal as sps
from scipy.ndimage import uniform_filter1d
import soundfile as sf
import librosa

# ----------------------------------------------------------------------------------------------
# Constants
# ----------------------------------------------------------------------------------------------
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RADIO_DIR = os.path.join(ROOT, "target", "radio")
SCRATCH = os.environ.get("RADIO_SIM_TMP", os.path.join(RADIO_DIR, "_tmp"))

SR = 44100

SOURCES: Dict[str, str] = {
    # watched
    "vibe_ace": "Kevin_MacLeod_-_Vibe_Ace.hq.ogg",
    "sweet_waltz": "147793__setuniman__sweet-waltz-0i-22mi.hq.ogg",
    # other music
    "sugar_plum": "Kevin_MacLeod_-_P_I_Tchaikovsky_Dance_of_the_Sugar_Plum_Fairy.hq.ogg",
    "brahms": "Hungarian_Dance_number_5_-_Allegro_in_F_sharp_minor_(string_orchestra).hq.ogg",
    "fishin": "Karissa_Hobbs_-_Lets_Go_Fishin.hq.ogg",
    "pistachio": "442789__lena-orsa__happy-music-pistachio-ice-cream-ragtime.hq.ogg",
    "choice": "admiralbob77_-_Choice_-_Drum-bass.hq.ogg",
    "trumpet": "sorohanro_-_solo-trumpet-06.hq.ogg",
    # speech (16 kHz mono, resampled on load)
    "libri1": "5703-47212-0000.hq.ogg",
    "libri2": "3436-172162-0000.hq.ogg",
    "libri3": "198-209-0000.hq.ogg",
}
WATCHED = ["vibe_ace", "sweet_waltz"]
OTHER_MUSIC = ["sugar_plum", "brahms", "fishin", "pistachio", "choice", "trumpet"]
SPEECH = ["libri1", "libri2", "libri3"]

MUSIC_RMS_DBFS = -20.0
SPEECH_RMS_DBFS = -23.0
GAP_S = 0.5                 # silence between segments
XFADE_S = 6.0               # crossfade length
WATCH_PLAY_S = 40.0         # length of a watched-song excerpt (in source seconds)
TALKOVER_S = 12.0           # speech mixed over the head of a talkover play
TALKOVER_DUCK_DB = -6.0     # music level under the speech
TALKOVER_SPEECH_REL_DB = -3.0  # speech level relative to the music RMS
EDGE_FADE_S = 0.01          # tiny fades on every excerpt edge to avoid clicks

# FM chain
FM_LPF_HZ = 15000.0
FM_LPF_ORDER = 8
COMP_RMS_WIN_S = 0.050
COMP_ATTACK_S = 0.005
COMP_RELEASE_S = 0.200
COMP_THRESH_DB = -18.0
COMP_RATIO = 4.0
MAKEUP_PEAK_DBFS = -1.0
CLIP = 0.99

# bass cut
HPF_HZ = 120.0
HPF_ORDER = 4

# MP3: libsndfile maps compression_level -> LAME quality; 0.65 with CONSTANT bitrate mode gives 128 kbps CBR
MP3_COMPRESSION_LEVEL = 0.65

EVAL_SEED = 1234
NULL_SEED = 4321
NULL_MIN_S = 30.5 * 60.0

TREATMENTS = [
    "clean",
    "pitch_fader_+6",
    "pitch_fader_-8",
    "keylock_tempo_+10",
    "keylock_tempo_-8",
    "key_change_+2",
    "key_change_-1",
    "pitch_fader_+4_talkover",
    "crossfade",
    "bass_cut_fm_mp3",
    "fm_mp3",
    "fader_+6_fm_mp3",
]

# ----------------------------------------------------------------------------------------------
# Plan / result records
# ----------------------------------------------------------------------------------------------


@dataclass
class PlanItem:
    source: str
    treatment: str
    excerpt_start: float          # seconds into the source
    duration: float               # seconds of source material used
    kind: str                     # "music" | "speech"
    talkover_source: Optional[str] = None
    talkover_start: float = 0.0


@dataclass
class Rendered:
    item: PlanItem
    audio: np.ndarray
    speed: float
    tempo: float
    semitones: float
    extra: dict = field(default_factory=dict)


# ----------------------------------------------------------------------------------------------
# Loading and basic utilities
# ----------------------------------------------------------------------------------------------
_CACHE: Dict[str, np.ndarray] = {}


def load_source(sid: str) -> np.ndarray:
    if sid not in _CACHE:
        path = os.path.join(RADIO_DIR, SOURCES[sid])
        y, _ = librosa.load(path, sr=SR, mono=True)
        _CACHE[sid] = y.astype(np.float32)
    return _CACHE[sid]


def source_duration(sid: str) -> float:
    return len(load_source(sid)) / SR


def db_to_lin(db: float) -> float:
    return 10.0 ** (db / 20.0)


def rms(x: np.ndarray) -> float:
    return float(np.sqrt(np.mean(np.square(x, dtype=np.float64)))) if len(x) else 0.0


def level_match(x: np.ndarray, target_dbfs: float) -> np.ndarray:
    r = rms(x)
    if r < 1e-9:
        return x
    return (x * (db_to_lin(target_dbfs) / r)).astype(np.float32)


def edge_fade(x: np.ndarray, fade_s: float = EDGE_FADE_S) -> np.ndarray:
    n = min(int(fade_s * SR), len(x) // 2)
    if n <= 0:
        return x
    x = x.copy()
    ramp = np.linspace(0.0, 1.0, n, dtype=np.float32)
    x[:n] *= ramp
    x[-n:] *= ramp[::-1]
    return x


def excerpt(sid: str, start_s: float, dur_s: float) -> np.ndarray:
    y = load_source(sid)
    a = int(round(start_s * SR))
    b = min(len(y), a + int(round(dur_s * SR)))
    return edge_fade(y[a:b])


def fix_length(y: np.ndarray, n: int) -> np.ndarray:
    if len(y) >= n:
        return y[:n]
    return np.concatenate([y, np.zeros(n - len(y), dtype=y.dtype)])


# ----------------------------------------------------------------------------------------------
# Treatments (each takes float32 mono audio at SR and returns the same)
# ----------------------------------------------------------------------------------------------


def pitch_fader(x: np.ndarray, pct: float) -> np.ndarray:
    """Classic DJ pitch fader: play `pct` percent faster (and higher).  Resample SR -> SR/f and
    relabel the result as SR."""
    f = 1.0 + pct / 100.0
    y = librosa.resample(x, orig_sr=SR, target_sr=SR / f, res_type="soxr_hq")
    return y.astype(np.float32)


def keylock_tempo(x: np.ndarray, rate: float) -> np.ndarray:
    """Tempo change with unchanged pitch (phase vocoder)."""
    return librosa.effects.time_stretch(x, rate=rate).astype(np.float32)


def key_change(x: np.ndarray, n_steps: float) -> np.ndarray:
    """Pitch shift in semitones, tempo unchanged."""
    return librosa.effects.pitch_shift(x, sr=SR, n_steps=n_steps).astype(np.float32)


def highpass(x: np.ndarray, hz: float = HPF_HZ, order: int = HPF_ORDER) -> np.ndarray:
    sos = sps.butter(order, hz, btype="highpass", fs=SR, output="sos")
    return sps.sosfiltfilt(sos, x).astype(np.float32)


def one_pole_smooth(x: np.ndarray, tau_s: float) -> np.ndarray:
    a = math.exp(-1.0 / (tau_s * SR))
    return sps.lfilter([1.0 - a], [1.0, -a], x)


def broadcast_compressor(x: np.ndarray) -> np.ndarray:
    """Simple broadcast-style compressor/limiter, vectorized.

    Envelope: 50 ms RMS -> attack/release one-pole smoothing (fast 'attack' pass and slow 'release'
    pass combined with max, so rises are tracked quickly and falls decay slowly).  Gain reduction
    above COMP_THRESH_DB with COMP_RATIO, make-up gain so the peak sits at MAKEUP_PEAK_DBFS, then
    hard clip at +-CLIP.
    """
    x64 = x.astype(np.float64)
    win = max(1, int(COMP_RMS_WIN_S * SR))
    env = np.sqrt(np.maximum(uniform_filter1d(x64 * x64, win, mode="nearest"), 0.0))
    env_att = one_pole_smooth(env, COMP_ATTACK_S)
    env_rel = one_pole_smooth(env, COMP_RELEASE_S)
    env = np.maximum(env_att, env_rel)
    env_db = 20.0 * np.log10(env + 1e-9)
    over = np.maximum(env_db - COMP_THRESH_DB, 0.0)
    gain_db = -over * (1.0 - 1.0 / COMP_RATIO)
    y = x64 * (10.0 ** (gain_db / 20.0))
    peak = float(np.max(np.abs(y))) if len(y) else 0.0
    if peak > 1e-9:
        y *= db_to_lin(MAKEUP_PEAK_DBFS) / peak
    return np.clip(y, -CLIP, CLIP).astype(np.float32)


def fm_chain(x: np.ndarray) -> np.ndarray:
    """FM broadcast chain: 15 kHz zero-phase low-pass, then compressor/limiter."""
    sos = sps.butter(FM_LPF_ORDER, FM_LPF_HZ, btype="lowpass", fs=SR, output="sos")
    y = sps.sosfiltfilt(sos, x.astype(np.float64))
    return broadcast_compressor(y)


_MP3_DELAY: Optional[int] = None


def _mp3_write_read(x: np.ndarray, path: str) -> np.ndarray:
    sf.write(path, x, SR, format="MP3", bitrate_mode="CONSTANT", compression_level=MP3_COMPRESSION_LEVEL)
    y, sr_read = sf.read(path, dtype="float32")
    assert sr_read == SR
    if y.ndim > 1:
        y = y[:, 0]
    return y.astype(np.float32)


def mp3_delay() -> int:
    """Measure the codec's constant delay once, by cross-correlating a short noise burst."""
    global _MP3_DELAY
    if _MP3_DELAY is None:
        rng = np.random.default_rng(0)
        n = SR * 2
        test = np.zeros(n, dtype=np.float32)
        burst = rng.standard_normal(SR // 2).astype(np.float32)
        sos = sps.butter(4, 6000.0, btype="lowpass", fs=SR, output="sos")
        test[SR // 2: SR] = 0.3 * sps.sosfilt(sos, burst)
        y = _mp3_write_read(test, os.path.join(SCRATCH, "_delay_probe.mp3"))
        ref = test[SR // 2: SR]
        seg = fix_length(y, n)[: n]
        c = sps.correlate(seg, ref, mode="valid")
        _MP3_DELAY = int(np.argmax(c)) - SR // 2
        print(f"[mp3] codec delay measured: {_MP3_DELAY} samples")
    return _MP3_DELAY


def mp3_roundtrip(x: np.ndarray) -> np.ndarray:
    """128 kbps MP3 encode/decode, length preserved and codec delay compensated."""
    d = mp3_delay()
    y = _mp3_write_read(x, os.path.join(SCRATCH, "_roundtrip.mp3"))
    if d > 0:
        y = y[d:]
    elif d < 0:
        y = np.concatenate([np.zeros(-d, dtype=np.float32), y])
    return fix_length(y, len(x))


def talkover_mix(music: np.ndarray, speech: np.ndarray) -> np.ndarray:
    """Mix `speech` over the head of `music` (already level-matched), ducking the music under it."""
    n_sp = min(len(speech), len(music))
    speech = edge_fade(speech[:n_sp], 0.05)
    ramp_n = int(0.2 * SR)
    duck = db_to_lin(TALKOVER_DUCK_DB)
    env = np.ones(len(music), dtype=np.float32)
    env[:n_sp] = duck
    # smooth ramps into / out of the duck
    env[:ramp_n] = np.linspace(1.0, duck, ramp_n)
    out_end = min(len(music), n_sp + ramp_n)
    env[n_sp:out_end] = np.linspace(duck, 1.0, out_end - n_sp)
    speech_target = MUSIC_RMS_DBFS + TALKOVER_SPEECH_REL_DB
    speech = level_match(speech, speech_target)
    y = music * env
    y[:n_sp] += speech
    return y.astype(np.float32)


# ----------------------------------------------------------------------------------------------
# Rendering one plan item
# ----------------------------------------------------------------------------------------------


def treatment_params(t: str):
    """(speed, tempo, semitones) for the ground truth."""
    if t == "pitch_fader_+6" or t == "fader_+6_fm_mp3":
        return 1.06, 1.06, 12.0 * math.log2(1.06)
    if t == "pitch_fader_-8":
        return 0.92, 0.92, 12.0 * math.log2(0.92)
    if t == "pitch_fader_+4_talkover":
        return 1.04, 1.04, 12.0 * math.log2(1.04)
    if t == "keylock_tempo_+10":
        return 1.0, 1.10, 0.0
    if t == "keylock_tempo_-8":
        return 1.0, 0.92, 0.0
    if t == "key_change_+2":
        return 1.0, 1.0, 2.0
    if t == "key_change_-1":
        return 1.0, 1.0, -1.0
    return 1.0, 1.0, 0.0


def render(item: PlanItem) -> Rendered:
    x = excerpt(item.source, item.excerpt_start, item.duration)
    t = item.treatment
    extra = {}
    if item.kind == "speech":
        y = level_match(x, SPEECH_RMS_DBFS)
        return Rendered(item, y, 1.0, 1.0, 0.0)

    if t in ("clean", "crossfade"):
        y = x
    elif t == "pitch_fader_+6":
        y = pitch_fader(x, +6)
    elif t == "pitch_fader_-8":
        y = pitch_fader(x, -8)
    elif t == "keylock_tempo_+10":
        y = keylock_tempo(x, 1.10)
    elif t == "keylock_tempo_-8":
        y = keylock_tempo(x, 0.92)
    elif t == "key_change_+2":
        y = key_change(x, 2)
    elif t == "key_change_-1":
        y = key_change(x, -1)
    elif t == "pitch_fader_+4_talkover":
        y = level_match(pitch_fader(x, +4), MUSIC_RMS_DBFS)
        sp = excerpt(item.talkover_source, item.talkover_start, TALKOVER_S)
        y = talkover_mix(y, sp)
        extra = {"talkover_source": item.talkover_source, "talkover_start": item.talkover_start,
                 "talkover_duration": min(TALKOVER_S, len(sp) / SR)}
    elif t == "bass_cut_fm_mp3":
        y = mp3_roundtrip(fm_chain(highpass(x)))
    elif t == "fm_mp3":
        y = mp3_roundtrip(fm_chain(x))
    elif t == "fader_+6_fm_mp3":
        y = mp3_roundtrip(fm_chain(pitch_fader(x, +6)))
    else:
        raise ValueError(f"unknown treatment {t}")

    if t != "pitch_fader_+4_talkover":     # talkover already level-matched before the mix
        y = level_match(y, MUSIC_RMS_DBFS)
    speed, tempo, semis = treatment_params(t)
    return Rendered(item, y.astype(np.float32), speed, tempo, semis, extra)


# ----------------------------------------------------------------------------------------------
# Plans
# ----------------------------------------------------------------------------------------------


def watched_start(sid: str, rng: np.random.Generator, from_zero: bool) -> float:
    if from_zero:
        return 0.0
    max_start = max(0.0, source_duration(sid) - WATCH_PLAY_S)
    return float(round(rng.uniform(0.25 * max_start, max_start), 3))


def music_item(sid: str, treatment: str, start: float, dur: float) -> PlanItem:
    dur = min(dur, source_duration(sid) - start)
    return PlanItem(sid, treatment, round(start, 3), round(dur, 3), "music")


def speech_item(rng: np.random.Generator, sid: Optional[str] = None) -> PlanItem:
    sid = sid or str(rng.choice(SPEECH))
    total = source_duration(sid)
    dur = float(rng.uniform(6.0, 12.0))
    start = float(rng.uniform(0.0, max(0.0, total - dur)))
    return PlanItem(sid, "clean", round(start, 3), round(dur, 3), "speech")


def filler_item(rng: np.random.Generator, sid: str, treatment: str = "clean",
                dur_range=(30.0, 60.0), start: Optional[float] = None) -> PlanItem:
    total = source_duration(sid)
    dur = min(float(rng.uniform(*dur_range)), total)
    if start is None:
        start = float(rng.uniform(0.0, max(0.0, total - dur)))
    item = music_item(sid, treatment, start, dur)
    if treatment == "pitch_fader_+4_talkover":
        item.talkover_source = str(rng.choice(SPEECH))
        item.talkover_start = round(float(rng.uniform(0.0, max(0.0, source_duration(item.talkover_source) - TALKOVER_S))), 3)
    return item


def watched_item(rng: np.random.Generator, sid: str, treatment: str, from_zero: bool) -> PlanItem:
    item = music_item(sid, treatment, watched_start(sid, rng, from_zero), WATCH_PLAY_S)
    if treatment == "pitch_fader_+4_talkover":
        item.talkover_source = str(rng.choice(SPEECH))
        item.talkover_start = round(float(rng.uniform(0.0, max(0.0, source_duration(item.talkover_source) - TALKOVER_S))), 3)
    return item


def build_eval_plan(rng: np.random.Generator) -> List[PlanItem]:
    """~14 min programme: every treatment once for vibe_ace, four for sweet_waltz, non-target
    fillers and DJ speech in between.  A "crossfade" play is always sandwiched between two songs."""
    W = lambda sid, t, z: watched_item(rng, sid, t, z)
    F = lambda sid, t="clean", d=(30.0, 38.0): filler_item(rng, sid, t, d)
    S = lambda sid=None: speech_item(rng, sid)
    return [
        F("sugar_plum"),
        S("libri1"),
        W("vibe_ace", "clean", True),
        W("sweet_waltz", "pitch_fader_+6", False),
        S("libri2"),
        W("vibe_ace", "pitch_fader_+6", False),
        F("brahms", "keylock_tempo_+10"),
        W("vibe_ace", "pitch_fader_-8", True),
        S("libri3"),
        W("vibe_ace", "keylock_tempo_+10", False),
        W("sweet_waltz", "key_change_-1", True),
        F("fishin", "clean", (30.0, 40.0)),
        W("vibe_ace", "keylock_tempo_-8", True),
        W("vibe_ace", "key_change_+2", False),
        S("libri1"),
        W("vibe_ace", "key_change_-1", True),
        W("vibe_ace", "pitch_fader_+4_talkover", False),
        F("pistachio", "clean", (30.0, 35.0)),
        W("vibe_ace", "crossfade", True),
        F("choice", "fm_mp3", (25.0, 25.0)),
        S("libri2"),
        W("vibe_ace", "bass_cut_fm_mp3", False),
        W("sweet_waltz", "fm_mp3", False),
        W("vibe_ace", "fm_mp3", True),
        F("trumpet", "clean", (5.0, 6.0)),
        W("vibe_ace", "fader_+6_fm_mp3", False),
        S("libri3"),
        W("sweet_waltz", "fader_+6_fm_mp3", True),
        F("sugar_plum", "keylock_tempo_-8"),
    ]


def build_null_plan(rng: np.random.Generator, min_seconds: float) -> List[PlanItem]:
    """>= min_seconds of non-target songs and speech, treatments drawn at random."""
    plan: List[PlanItem] = []
    est = 0.0
    songs = [s for s in OTHER_MUSIC if s != "trumpet"]
    prev_kind = None
    force_music = False           # the item after a crossfade must be a song
    while est < min_seconds:
        want_speech = (not force_music) and prev_kind == "music" and rng.random() < 0.35
        if want_speech:
            item = speech_item(rng)
        else:
            if (not force_music) and rng.random() < 0.08:
                item = filler_item(rng, "trumpet", "clean", (5.0, 6.0))   # jingle
            else:
                sid = str(rng.choice(songs))
                t = str(rng.choice(TREATMENTS))
                prev_ok = prev_kind == "music" and plan[-1].duration >= XFADE_S + 1.0
                if t == "crossfade" and not prev_ok:
                    t = "clean"
                item = filler_item(rng, sid, t)
        force_music = item.treatment == "crossfade"
        plan.append(item)
        prev_kind = item.kind
        est += item.duration + GAP_S
    if plan[-1].treatment == "crossfade":       # a crossfade needs a following song
        plan.append(filler_item(rng, str(rng.choice(songs)), "clean"))
    return plan


# ----------------------------------------------------------------------------------------------
# Assembly
# ----------------------------------------------------------------------------------------------


def assemble(rendered: List[Rendered]):
    """Concatenate rendered segments with GAP_S of silence, or an equal-power XFADE_S crossfade on
    both sides of a "crossfade" play.  Returns (stream, segments) with sample-exact times."""
    gap = int(GAP_S * SR)
    xf = int(XFADE_S * SR)
    total = sum(len(r.audio) for r in rendered) + gap * len(rendered) + SR
    out = np.zeros(total, dtype=np.float32)
    theta = np.linspace(0.0, math.pi / 2.0, xf, dtype=np.float32)
    fade_in, fade_out = np.sin(theta), np.cos(theta)

    segments = []
    prev: Optional[Rendered] = None
    prev_end = 0
    for r in rendered:
        n = len(r.audio)
        overlap = prev is not None and (
            (r.item.treatment == "crossfade" and prev.item.kind == "music")
            or (prev.item.treatment == "crossfade" and r.item.kind == "music"))
        if prev is None:
            start = 0
        elif overlap and n >= xf and len(prev.audio) >= xf:
            start = prev_end - xf
            out[start:prev_end] *= fade_out
            a = r.audio.copy()
            a[:xf] *= fade_in
            r = Rendered(r.item, a, r.speed, r.tempo, r.semitones, r.extra)
            # the previous play also fades out here; keep its record as-is (it ends at prev_end)
        else:
            start = prev_end + gap
            segments.append(dict(start=prev_end / SR, end=start / SR, source="silence", watched=False,
                                 treatment="silence", excerpt_start=0.0, speed=1.0, tempo=1.0, semitones=0.0))
        end = start + n
        out[start:end] += r.audio
        seg = dict(start=start / SR, end=end / SR, source=r.item.source,
                   watched=r.item.source in WATCHED, treatment=r.item.treatment,
                   excerpt_start=float(r.item.excerpt_start), excerpt_duration=float(r.item.duration),
                   speed=r.speed, tempo=r.tempo, semitones=r.semitones)
        seg.update(r.extra)
        segments.append(seg)
        prev, prev_end = r, end
    return out[:prev_end], segments


def build_stream(name: str, plan: List[PlanItem], seed: int):
    t0 = time.time()
    rendered = []
    for i, item in enumerate(plan):
        rendered.append(render(item))
        print(f"  [{name}] rendered {i + 1:3d}/{len(plan)}  {item.source:<12s} {item.treatment:<24s} "
              f"{len(rendered[-1].audio) / SR:6.1f}s", flush=True)
    stream, segments = assemble(rendered)
    print(f"  [{name}] assembled {len(stream) / SR:.1f} s, applying FM chain...", flush=True)
    stream = fm_chain(stream)
    wav = os.path.join(RADIO_DIR, f"{name}.wav")
    sf.write(wav, stream, SR, subtype="PCM_16")
    gt = {"sample_rate": SR, "duration": len(stream) / SR, "seed": seed,
          "watched_sources": WATCHED, "segments": segments}
    js = os.path.join(RADIO_DIR, f"{name}.json")
    with open(js, "w") as f:
        json.dump(gt, f, indent=1)
    print(f"  [{name}] done in {time.time() - t0:.1f} s -> {wav}, {js}")
    return gt, wav, js


def print_summary(name: str, gt: dict):
    segs = gt["segments"]
    print(f"\n=== {name}: {gt['duration'] / 60:.2f} min, {len(segs)} segments "
          f"({sum(s['source'] != 'silence' for s in segs)} non-silence) ===")
    print(f"{'idx':>4} {'start':>9} {'end':>9} {'source':<12} {'treatment':<24} {'W':>1}")
    for i, s in enumerate(segs):
        if s["source"] == "silence":
            continue
        print(f"{i:4d} {s['start']:9.3f} {s['end']:9.3f} {s['source']:<12} {s['treatment']:<24} "
              f"{'*' if s['watched'] else ''}")
    counts = {}
    for s in segs:
        if s["watched"]:
            counts[s["source"]] = counts.get(s["source"], 0) + 1
    print(f"watched plays: {counts if counts else 'none'}")


# ----------------------------------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------------------------------


def main():
    t_all = time.time()
    os.makedirs(SCRATCH, exist_ok=True)

    print("Loading sources...")
    for sid in SOURCES:
        print(f"  {sid:<12s} {source_duration(sid):6.1f} s")

    # 1. clean watched references (float WAV, untreated)
    for sid in WATCHED:
        path = os.path.join(RADIO_DIR, f"watch_{sid}.wav")
        sf.write(path, load_source(sid), SR, subtype="FLOAT")
        print(f"wrote {path}")

    # 2. evaluation stream
    rng = np.random.default_rng(EVAL_SEED)
    eval_plan = build_eval_plan(rng)
    gt_eval, _, _ = build_stream("stream_eval", eval_plan, EVAL_SEED)

    # 3. null stream
    rng = np.random.default_rng(NULL_SEED)
    null_plan = build_null_plan(rng, NULL_MIN_S)
    gt_null, _, _ = build_stream("stream_null", null_plan, NULL_SEED)

    # cleanup scratch mp3s
    for fn in os.listdir(SCRATCH):
        os.remove(os.path.join(SCRATCH, fn))
    os.rmdir(SCRATCH)

    print_summary("stream_eval", gt_eval)
    print_summary("stream_null", gt_null)

    # sanity: no watched source in the null stream, every treatment covered for vibe_ace
    assert not any(s["watched"] for s in gt_null["segments"])
    va = {s["treatment"] for s in gt_eval["segments"] if s["source"] == "vibe_ace"}
    missing = set(TREATMENTS) - va
    assert not missing, f"vibe_ace misses treatments: {missing}"
    print(f"\ntotal run time {time.time() - t_all:.1f} s")


if __name__ == "__main__":
    sys.exit(main())
