#!/usr/bin/env python3
"""Validate the transform on real music and prove fingerprint invariances.

Downloads a 30 s excerpt of "Vibe Ace" by Kevin MacLeod (CC BY 3.0, via the
librosa example-data repository), creates pitch-shifted, tempo-changed,
sped-up, clipped and noisy versions, cuts every version into 10 s chunks,
runs `examples/cqt_dump.rs` on each chunk, fingerprints the spectrograms
with pitch- and tempo-invariant peak triplets, and matches every chunk
against the fingerprint of the full original, reporting the recovered pitch
shift, tempo factor and position, plus how the evidence grows with the
length of the query. It also compares our spectrogram with librosa's CQT of
the same excerpt.

    pip install numpy scipy soundfile librosa matplotlib
    python3 scripts/fingerprint_demo.py

Figures are written to plots/ and a summary table to stdout.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import urllib.request
from collections import defaultdict
from pathlib import Path

import numpy as np
import soundfile as sf
import librosa
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from scipy import ndimage  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
WORK = ROOT / "target" / "fingerprint_demo"
PLOTS = ROOT / "plots"
BASE_URL = "https://raw.githubusercontent.com/librosa/data/main/audio/"
SOURCE = (
    "Kevin_MacLeod_-_Vibe_Ace.hq.ogg",
    "73d6443ef90a7c022f164e5aa90e56c2291585930b39b1656d0765abbc1f1779",
)
# An unrelated piece by the same producer as the negative control.
CONTROL_SOURCE = (
    "Kevin_MacLeod_-_P_I_Tchaikovsky_Dance_of_the_Sugar_Plum_Fairy.hq.ogg",
    "f062221a56a227cdb7c067cf2e6ac0e250a50012f7693ca0c8e31f05f83e49b1",
)
ATTRIBUTION = (
    "Vibe Ace by Kevin MacLeod, CC BY 3.0 "
    "(https://freemusicarchive.org/music/Kevin_MacLeod/Jazz_Sampler/Vibe_Ace)"
)
CONTROL_ATTRIBUTION = (
    "Dance of the Sugar Plum Fairy by Kevin MacLeod, CC BY 3.0 "
    "(https://freemusicarchive.org/music/Kevin_MacLeod/Classical_Sampler/Dance_of_the_Sugar_Plum_Fairy)"
)

SR = 44_100
HOP = 256
MIN_FREQ = 55.0
MAX_FREQ = 7_040.0
BINS_PER_OCTAVE = 24
EXCERPT = (0.5, 30.5)  # seconds
SAME_SONG = (31.0, 61.0)  # the other, non-overlapping half of the song
CONTROL = (5.0, 35.0)  # window of the unrelated control piece
CHUNK_SECONDS = 10.0
# Fewer consistent matches than this is treated as no identification.
MIN_CONSISTENT = 30


def download(name: str, sha256: str) -> Path:
    WORK.mkdir(parents=True, exist_ok=True)
    path = WORK / name
    if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != sha256:
        print(f"downloading {BASE_URL + name}")
        urllib.request.urlretrieve(BASE_URL + name, path)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != sha256:
            sys.exit(f"checksum mismatch: {digest}")
    return path


def cqt_dump(wav: Path) -> np.ndarray:
    npy = wav.with_suffix(".npy")
    subprocess.run(
        [
            "cargo", "run", "--release", "--quiet", "--example", "cqt_dump", "--",
            str(wav), str(npy), str(HOP), str(MIN_FREQ), str(MAX_FREQ), str(BINS_PER_OCTAVE),
        ],
        cwd=ROOT,
        check=True,
    )
    return np.load(npy)


def to_db(magnitudes: np.ndarray, top_db: float = 80.0) -> np.ndarray:
    db = 20.0 * np.log10(np.maximum(magnitudes, 1e-5))
    return np.maximum(db, db.max() - top_db)


def peaks(db: np.ndarray, time_radius: int = 24, bin_radius: int = 9, prominence_db: float = 15.0,
          floor_db: float = -70.0):
    """Local maxima of the dB spectrogram that stand `prominence_db` above the
    mean of their neighbourhood, as (frame, bin) pairs.

    A prominence threshold adapts to the local level: it keeps peaks in
    quiet passages, ignores the raised floor of noisy audio and drops the
    dense side lobes of clipped audio, where a fixed threshold does the
    opposite. `floor_db` below the global maximum is a safety net."""
    size = (2 * time_radius + 1, 2 * bin_radius + 1)
    local_max = ndimage.maximum_filter(db, size=size, mode="nearest") == db
    local_mean = ndimage.uniform_filter(db, size=size, mode="nearest")
    strong = (db >= local_mean + prominence_db) & (db >= db.max() + floor_db)
    frames, bins = np.nonzero(local_max & strong)
    return np.stack([frames, bins], axis=1)


def peak_survival(query_peaks: np.ndarray, reference_peaks: np.ndarray, shift: int, tempo: float,
                  offset: float, time_tolerance: int = 3, bin_tolerance: int = 1) -> float:
    """Fraction of the query's peaks with a reference peak within tolerance
    after mapping through the detected shift, tempo and offset. A triplet
    hash survives only when all three of its peaks do, so the match score is
    bounded by roughly the cube of this number."""
    if len(query_peaks) == 0 or np.isnan(offset):
        return np.nan
    reference = {(int(t), int(b)) for t, b in reference_peaks}
    hits = 0
    for t, b in query_peaks:
        t_mapped = int(round(tempo * t + offset))
        b_mapped = int(b) - shift
        if any((t_mapped + dt, b_mapped + db) in reference
               for dt in range(-time_tolerance, time_tolerance + 1)
               for db in range(-bin_tolerance, bin_tolerance + 1)):
            hits += 1
    return hits / len(query_peaks)


def triplet_hashes(points: np.ndarray, zone_frames: int = 320, fan_out: int = 6, ratio_steps: int = 32):
    """Pitch- and tempo-invariant hashes from peak triplets.

    For peaks p1 < p2 < p3 in time the key is the two bin differences and the
    ratio (t2 - t1) / (t3 - t1) quantized to `ratio_steps`; bin differences
    survive pitch shifts and the ratio survives tempo changes.
    """
    order = np.lexsort((points[:, 1], points[:, 0]))
    points = points[order]
    times = points[:, 0]
    hashes = defaultdict(list)
    for i, (t1, b1) in enumerate(points):
        j_end = np.searchsorted(times, t1 + zone_frames, side="right")
        candidates = points[i + 1 : min(j_end, i + 1 + 3 * fan_out)]
        for a in range(len(candidates)):
            t2, b2 = candidates[a]
            if t2 == t1:
                continue
            for c in range(a + 1, min(len(candidates), a + 1 + fan_out)):
                t3, b3 = candidates[c]
                if t3 == t2:
                    continue
                ratio = (t2 - t1) / (t3 - t1)
                key = (int(b2 - b1), int(b3 - b2), int(round(ratio * ratio_steps)))
                hashes[key].append((int(t1), int(b1), int(t3 - t1)))
    return hashes


def match(query: dict, reference: dict, ratio_tolerance: int = 1, bin_tolerance: int = 1):
    """Returns matched (t_query, t_ref, b_query, b_ref, span_query, span_ref)
    tuples, where the spans are the triplets' first-to-last peak distances.

    Keys are looked up within ±`ratio_tolerance` on the quantized ratio,
    which absorbs frame rounding after a tempo change, and within
    ±`bin_tolerance` on each bin difference, which absorbs peaks that move by
    one bin under distortion.
    """
    pairs = []
    offsets = range(-bin_tolerance, bin_tolerance + 1)
    for (d1, d2, r), q_entries in query.items():
        for e1 in offsets:
            for e2 in offsets:
                for dr in range(-ratio_tolerance, ratio_tolerance + 1):
                    r_entries = reference.get((d1 + e1, d2 + e2, r + dr))
                    if not r_entries or len(r_entries) > 8:
                        continue
                    for tq, bq, sq in q_entries:
                        for tr, br, sr_ in r_entries:
                            pairs.append((tq, tr, bq, br, sq, sr_))
    return np.array(pairs, dtype=float).reshape(-1, 6)


def estimate(pairs: np.ndarray, offset_tolerance_frames: float = SR / HOP):
    """Estimates the pitch shift (query − reference, in bins), the tempo
    factor (reference duration / query duration) and the position of the
    query inside the reference (in reference frames) by voting.

    The tempo of each match is the ratio of the two triplets' time spans,
    which does not depend on where the query starts. The densest cell of a
    (bin offset, tempo) histogram wins, the offset follows from
    `t_ref = tempo * t_query + offset`, and the matches within ±1 bin, ±3 %
    tempo and ±1 s offset of the estimate count as consistent."""
    empty = dict(matches=len(pairs), consistent=0, bins=np.nan, tempo=np.nan, offset=np.nan)
    if len(pairs) < 10:
        return empty
    d_bins = pairs[:, 2] - pairs[:, 3]
    tempo = pairs[:, 5] / pairs[:, 4]
    keep = (np.abs(d_bins) <= 24) & (tempo > 0.7) & (tempo < 1.4)
    pairs, d_bins, tempo = pairs[keep], d_bins[keep], tempo[keep]
    if len(d_bins) < 10:
        return empty
    votes, bin_edges, tempo_edges = np.histogram2d(
        d_bins, tempo, bins=[np.arange(-24.5, 25.5, 1.0), np.arange(0.7, 1.4001, 0.01)]
    )
    i, j = np.unravel_index(np.argmax(votes), votes.shape)
    est_bins = int(round(bin_edges[i] + 0.5))
    near = (np.abs(d_bins - est_bins) <= 1) & (np.abs(tempo - (tempo_edges[j] + 0.005)) <= 0.03)
    est_tempo = float(np.median(tempo[near]))
    offsets = pairs[:, 1] - est_tempo * pairs[:, 0]
    # Vote for the offset among the pairs that agree on shift and tempo.
    agree = (np.abs(d_bins - est_bins) <= 1) & (np.abs(tempo - est_tempo) <= 0.03)
    if agree.sum() < 5:
        return empty
    hist, edges = np.histogram(offsets[agree], bins=np.arange(offsets[agree].min() - 1,
                                                              offsets[agree].max() + offset_tolerance_frames,
                                                              offset_tolerance_frames / 2))
    centre = edges[np.argmax(hist)] + offset_tolerance_frames / 4
    close = agree & (np.abs(offsets - centre) <= offset_tolerance_frames)
    est_offset = float(np.median(offsets[close])) if close.any() else np.nan
    consistent = agree & (np.abs(offsets - est_offset) <= offset_tolerance_frames)
    return dict(matches=len(pairs), consistent=int(consistent.sum()), bins=est_bins,
                tempo=est_tempo, offset=est_offset)


def chunks(signal: np.ndarray, sr: int, seconds: float = CHUNK_SECONDS, minimum: float = 4.0):
    """Consecutive chunks of `seconds`; a trailing chunk shorter than
    `minimum` seconds is dropped."""
    size = int(seconds * sr)
    out = []
    for start in range(0, len(signal), size):
        piece = signal[start : start + size]
        if len(piece) >= minimum * sr:
            out.append((start / sr, piece))
    return out


def slug(name: str) -> str:
    return (name.split(" (")[0].replace(" ", "_").replace("+", "plus").replace("-", "minus")
            .replace("%", "pct").replace(",", ""))


def main() -> None:
    y, sr = librosa.load(download(*SOURCE), sr=SR, mono=True)
    excerpt = y[int(EXCERPT[0] * sr) : int(EXCERPT[1] * sr)]
    same_song = y[int(SAME_SONG[0] * sr) : int(SAME_SONG[1] * sr)]
    other, _ = librosa.load(download(*CONTROL_SOURCE), sr=SR, mono=True)
    control = other[int(CONTROL[0] * sr) : int(CONTROL[1] * sr)]
    rng = np.random.default_rng(7)

    semitone_bins = BINS_PER_OCTAVE // 12
    variants = {
        "original": (excerpt, 0, 1.0),
        "pitch +2 semitones": (
            librosa.effects.pitch_shift(excerpt, sr=sr, n_steps=2, bins_per_octave=12),
            2 * semitone_bins,
            1.0,
        ),
        "pitch -1 semitone": (
            librosa.effects.pitch_shift(excerpt, sr=sr, n_steps=-1, bins_per_octave=12),
            -semitone_bins,
            1.0,
        ),
        "tempo +12 %": (librosa.effects.time_stretch(excerpt, rate=1.12), 0, 1.12),
        "tempo -8 %": (librosa.effects.time_stretch(excerpt, rate=0.92), 0, 0.92),
        "speed +6 % (resampled)": (
            librosa.resample(excerpt, orig_sr=sr, target_sr=int(sr / 1.06)),
            int(round(np.log2(1.06) * BINS_PER_OCTAVE)),
            1.06,
        ),
        "hard clip at 0.2": (np.clip(excerpt, -0.2, 0.2), 0, 1.0),
        "white noise, 10 dB SNR": (
            excerpt + rng.normal(0, np.sqrt(np.mean(excerpt**2) / 10), excerpt.shape).astype(np.float32),
            0,
            1.0,
        ),
        "same song, other 30 s": (same_song, 0, 1.0),
        "control (unrelated piece)": (control, 0, 1.0),
    }

    # Reference: the full original excerpt.
    reference_wav = WORK / "original.wav"
    sf.write(reference_wav, excerpt, sr, subtype="FLOAT")
    reference_mag = cqt_dump(reference_wav)
    reference_db = to_db(reference_mag)
    reference_peaks = peaks(reference_db)
    reference_hashes = triplet_hashes(reference_peaks)
    print(f"reference: {reference_mag.shape}, {len(reference_peaks)} peaks, "
          f"{sum(len(v) for v in reference_hashes.values())} hashes")

    # --- accuracy against librosa's CQT --------------------------------------
    # librosa's magnitudes scale with each filter's length (`scale=False`) or
    # its square root (`scale=True`); ours are calibrated so that a sinusoid
    # of amplitude A reads as A. Divide out the lengths and the factor two
    # between L1-normalized and amplitude-calibrated kernels to compare.
    ours = reference_mag
    librosa_freqs = librosa.cqt_frequencies(n_bins=ours.shape[1], fmin=MIN_FREQ, bins_per_octave=BINS_PER_OCTAVE)
    lengths, _ = librosa.filters.wavelet_lengths(freqs=librosa_freqs, sr=sr, window="hann", filter_scale=1, gamma=0)
    reference = np.abs(
        librosa.cqt(
            excerpt, sr=sr, hop_length=HOP, fmin=MIN_FREQ, n_bins=ours.shape[1],
            bins_per_octave=BINS_PER_OCTAVE, scale=False,
        )
    ).T * (2.0 / lengths[None, :])
    n = min(len(ours), len(reference))
    strong = ours[:n] > 0.01
    ratio = np.median((ours[:n] / np.maximum(reference[:n], 1e-12))[strong])
    ours_db = to_db(ours[:n])
    ref_db = to_db(reference[:n])
    ours_db -= ours_db.max()
    ref_db -= ref_db.max()
    corr = np.corrcoef(ours_db.ravel(), ref_db.ravel())[0, 1]
    mad = np.mean(np.abs(ours_db - ref_db))
    mad_strong = np.mean(np.abs(ours_db - ref_db)[ours_db > -40])
    print(
        f"\nlibrosa comparison: linear gain ratio {ratio:.4f}, correlation of dB spectrograms "
        f"{corr:.4f}, mean |diff| {mad:.2f} dB ({mad_strong:.2f} dB above -40 dB)"
    )

    freqs = MIN_FREQ * 2 ** (np.arange(ours.shape[1]) / BINS_PER_OCTAVE)
    extent = [0, n * HOP / sr, 0, ours.shape[1]]
    fig, axes = plt.subplots(1, 3, figsize=(16, 4.6), constrained_layout=True)
    for ax, data, title in [
        (axes[0], ours_db, "cqt-rs (this library)"),
        (axes[1], ref_db, "librosa.cqt, same grid"),
        (axes[2], ours_db - ref_db, f"difference: mean |diff| = {mad:.2f} dB, r = {corr:.4f}"),
    ]:
        im = ax.imshow(data.T, origin="lower", aspect="auto", extent=extent,
                       cmap="magma" if "difference" not in title else "coolwarm",
                       vmin=-80 if "difference" not in title else -3,
                       vmax=0 if "difference" not in title else 3)
        ax.set_title(title)
        ax.set_xlabel("time (s)")
        ticks = np.arange(0, ours.shape[1], BINS_PER_OCTAVE)
        ax.set_yticks(ticks)
        ax.set_yticklabels([f"{freqs[t]:.0f}" for t in ticks])
        fig.colorbar(im, ax=ax, shrink=0.9, label="dB")
    axes[0].set_ylabel("centre frequency (Hz)")
    fig.suptitle(f"{ATTRIBUTION}, {EXCERPT[0]:.0f}–{EXCERPT[1]:.0f} s, hop {HOP}", fontsize=10)
    fig.savefig(PLOTS / "song_cqt_vs_librosa.png", dpi=80)
    plt.close(fig)

    # --- chunked queries --------------------------------------------------------
    results = {}
    for name, (signal, applied_bins, applied_rate) in variants.items():
        rows = []
        for k, (start_s, piece) in enumerate(chunks(signal, sr)):
            wav = WORK / f"{slug(name)}_chunk{k + 1}.wav"
            sf.write(wav, piece, sr, subtype="FLOAT")
            mag = cqt_dump(wav)
            db = to_db(mag)
            pk = peaks(db)
            hs = triplet_hashes(pk)
            pairs = match(hs, reference_hashes)
            est = estimate(pairs)
            total = sum(len(v) for v in hs.values())
            expected_offset = start_s * applied_rate
            if est["consistent"] < MIN_CONSISTENT:
                est.update(bins=np.nan, tempo=np.nan, offset=np.nan)
            survival = peak_survival(pk, reference_peaks, est["bins"], est["tempo"], est["offset"])
            rows.append(dict(
                chunk=k + 1, start=start_s, seconds=len(piece) / sr, hashes=total, pairs=pairs,
                peaks=pk, db=db, score=100.0 * est["consistent"] / max(total, 1),
                survival=survival, expected_offset=expected_offset,
                located=est["offset"] * HOP / sr if not np.isnan(est["offset"]) else np.nan, **est,
            ))
        results[name] = (rows, applied_bins, applied_rate)
        summary = ", ".join(f"{r['score']:.1f} %" for r in rows)
        print(f"{name:34} {summary}")

    def fmt(value, spec):
        if value is None or (isinstance(value, float) and np.isnan(value)):
            return "—"
        return format(int(value) if spec == "+d" else value, spec)

    print("\n| Version | Chunk 1 | Chunk 2 | Chunk 3 | Mean | Peaks kept | Shift applied / detected | Tempo applied / detected | Located at (expected) |")
    print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |")
    for name, (rows, applied_bins, applied_rate) in results.items():
        scores = [f"{r['score']:.1f} %" for r in rows] + ["—"] * (3 - len(rows))
        mean = np.mean([r["score"] for r in rows])
        shifts = "/".join(fmt(r["bins"], "+d") for r in rows)
        tempos = "/".join(fmt(r["tempo"], ".3f") for r in rows)
        located = ", ".join(f"{fmt(r['located'], '.1f')} ({r['expected_offset']:.1f})" for r in rows)
        kept = "/".join(fmt(100 * r["survival"], ".0f") for r in rows) + " %"
        print(f"| {name} | {' | '.join(scores)} | {mean:.1f} % | {kept} | {applied_bins:+d} / {shifts} | "
              f"×{applied_rate:.2f} / {tempos} | {located} s |")

    # --- figure: pitch shift and tempo change proofs on the middle chunk -----
    fig, axes = plt.subplots(2, 2, figsize=(14, 8.5), constrained_layout=True)
    ref_extent = [0, len(reference_db) * HOP / sr, 0, reference_db.shape[1]]
    for row, (name, colour) in enumerate([("pitch +2 semitones", "tab:red"), ("tempo +12 %", "tab:blue")]):
        rows, applied_bins, applied_rate = results[name]
        r = rows[1]
        ax = axes[row, 0]
        ax.imshow(reference_db.T - reference_db.max(), origin="lower", aspect="auto", cmap="magma",
                  vmin=-80, vmax=0, extent=ref_extent)
        ax.scatter(reference_peaks[:, 0] * HOP / sr, reference_peaks[:, 1] + 0.5, s=6, c="white",
                   label="peaks of the original (reference)")
        # Map the chunk's peaks into the reference through the detected
        # tempo, offset and pitch shift.
        t_mapped = (r["tempo"] * r["peaks"][:, 0] + r["offset"]) * HOP / sr
        b_mapped = r["peaks"][:, 1] - r["bins"] + 0.5
        ax.scatter(t_mapped, b_mapped, s=6, c=colour,
                   label=f"peaks of chunk 2 of «{name}», mapped through the detected "
                         f"shift {r['bins']:+d}, tempo ×{r['tempo']:.3f}, offset {r['located']:.1f} s")
        ax.axvspan(r["located"], r["located"] + r["seconds"] * r["tempo"], color=colour, alpha=0.12)
        ax.set_title(f"10 s chunk of «{name}» located in the original at {r['located']:.1f} s "
                     f"(expected {r['expected_offset']:.1f} s)")
        ax.set_xlabel("time in the original (s)")
        ax.set_ylabel("bin (24 per octave)")
        ax.legend(loc="upper right", fontsize=7)

        ax = axes[row, 1]
        pairs = r["pairs"]
        if row == 0:
            ax.hist(pairs[:, 2] - pairs[:, 3], bins=np.arange(-12.5, 13.5, 1), color=colour)
            ax.axvline(applied_bins, color="k", ls="--", label=f"applied: {applied_bins:+d} bins")
            ax.set_xlabel("bin offset of matched peaks (query − reference)")
            ax.set_title(f"detected shift {r['bins']:+d} bins from {r['consistent']} consistent matches")
        else:
            ax.hist(pairs[:, 5] / pairs[:, 4], bins=np.linspace(0.8, 1.3, 51), color=colour)
            ax.axvline(applied_rate, color="k", ls="--", label=f"applied: ×{applied_rate:.2f}")
            ax.set_xlabel("tempo factor of matched triplets (reference span / query span)")
            ax.set_title(f"detected tempo ×{r['tempo']:.3f} from {r['consistent']} consistent matches")
        ax.legend()
        ax.set_ylabel("matched triplet hashes")
    fig.suptitle("Fingerprint invariance on real music, 10 s queries (" + ATTRIBUTION + ")", fontsize=10)
    fig.savefig(PLOTS / "song_pitch_tempo_proof.png", dpi=80)
    plt.close(fig)

    # --- figure: match scores of every chunk ------------------------------------
    fig, ax = plt.subplots(figsize=(11, 6), constrained_layout=True)
    names = list(results)
    y = np.arange(len(names))
    height = 0.26
    chunk_colours = ["#2a9d8f", "#457b9d", "#8d5a97"]
    for k in range(3):
        vals = [results[nm][0][k]["score"] if len(results[nm][0]) > k else 0.0 for nm in names]
        bars = ax.barh(y + (k - 1) * height, vals, height=height, color=chunk_colours[k],
                       label=f"chunk {k + 1} ({k * 10}–{(k + 1) * 10} s of the version)")
        for bar, v in zip(bars, vals):
            if v > 0:
                ax.text(bar.get_width() + 0.5, bar.get_y() + bar.get_height() / 2, f"{v:.1f}", va="center", fontsize=7)
    ax.set_yticks(y)
    ax.set_yticklabels(names)
    ax.invert_yaxis()
    ax.set_xlabel("triplet hashes of the 10 s chunk matched consistently with the full original (%)")
    ax.set_title("Fingerprint match of 10 s chunks against the original 30 s excerpt "
                 "(control: " + CONTROL_ATTRIBUTION.split(",")[0] + ")", fontsize=10)
    ax.legend(loc="lower right", fontsize=8)
    fig.savefig(PLOTS / "song_match_scores.png", dpi=100)
    plt.close(fig)

    # --- figure: how many seconds of audio a match needs -------------------------
    # A real-time monitor sees the query grow; match the first 1..10 s of the
    # middle chunk of every version and count the consistent hashes.
    lengths = list(range(1, int(CHUNK_SECONDS) + 1))
    growth = {}
    for name, (rows, applied_bins, applied_rate) in results.items():
        r = rows[min(1, len(rows) - 1)]
        counts = []
        for seconds in lengths:
            db = r["db"][: int(seconds * sr / HOP)]
            hs = triplet_hashes(peaks(db))
            counts.append(estimate(match(hs, reference_hashes))["consistent"])
        growth[name] = counts
    fig, ax = plt.subplots(figsize=(10, 6), constrained_layout=True)
    for name, counts in growth.items():
        is_control = name.startswith("control")
        ax.plot(lengths, np.maximum(counts, 0.5), marker="o", ms=4, lw=2.2 if is_control else 1.4,
                ls="--" if is_control else "-", color="k" if is_control else None, label=name)
    ax.set_yscale("log")
    ax.set_xlabel("seconds of the query (start of the middle 10 s chunk)")
    ax.set_ylabel("consistent triplet matches with the original")
    ax.set_xticks(lengths)
    ax.grid(True, which="both", alpha=0.3)
    ax.set_title("Evidence accumulated against the original as the query grows "
                 "(dashed: unrelated piece)", fontsize=10)
    ax.legend(fontsize=8, loc="lower right", ncol=2)
    fig.savefig(PLOTS / "song_detection_time.png", dpi=100)
    plt.close(fig)
    print("\nconsistent matches after 1..10 s of the middle chunk:")
    for name, counts in growth.items():
        print(f"{name:34} " + " ".join(f"{c:5d}" for c in counts))

    json.dump(
        {name: dict(applied_bins=applied_bins, applied_rate=applied_rate,
                    chunks=[{k: v for k, v in r.items() if k not in ("pairs", "peaks", "db")} for r in rows],
                    growth=growth[name])
         for name, (rows, applied_bins, applied_rate) in results.items()},
        open(WORK / "summary.json", "w"), indent=2, default=float,
    )


if __name__ == "__main__":
    main()
