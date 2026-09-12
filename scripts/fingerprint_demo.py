#!/usr/bin/env python3
"""Validate the transform on real music and prove fingerprint invariances.

Downloads a 30 s excerpt of "Vibe Ace" by Kevin MacLeod (CC BY 3.0, via the
librosa example-data repository), creates pitch-shifted, tempo-changed,
sped-up, clipped and noisy versions, runs `examples/cqt_dump.rs` on every
version, fingerprints the spectrograms with pitch- and tempo-invariant peak
triplets, and matches every version against the original. It also compares
our spectrogram with librosa's CQT of the same excerpt.

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
SOURCE = (
    "https://raw.githubusercontent.com/librosa/data/main/audio/"
    "Kevin_MacLeod_-_Vibe_Ace.hq.ogg"
)
SOURCE_SHA256 = "73d6443ef90a7c022f164e5aa90e56c2291585930b39b1656d0765abbc1f1779"
ATTRIBUTION = (
    "Vibe Ace by Kevin MacLeod, CC BY 3.0 "
    "(https://freemusicarchive.org/music/Kevin_MacLeod/Jazz_Sampler/Vibe_Ace)"
)

SR = 44_100
HOP = 512
MIN_FREQ = 55.0
MAX_FREQ = 7_040.0
BINS_PER_OCTAVE = 24
EXCERPT = (0.5, 30.5)  # seconds
CONTROL = (31.0, 61.0)  # the other, non-overlapping half of the song


def download() -> Path:
    WORK.mkdir(parents=True, exist_ok=True)
    path = WORK / "vibe_ace.ogg"
    if not path.exists() or hashlib.sha256(path.read_bytes()).hexdigest() != SOURCE_SHA256:
        print(f"downloading {SOURCE}")
        urllib.request.urlretrieve(SOURCE, path)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        if digest != SOURCE_SHA256:
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


def peaks(db: np.ndarray, time_radius: int = 12, bin_radius: int = 6, min_db: float = -50.0):
    """Local maxima of the dB spectrogram, as (frame, bin) pairs."""
    footprint = np.ones((2 * time_radius + 1, 2 * bin_radius + 1), dtype=bool)
    local_max = ndimage.maximum_filter(db, footprint=footprint, mode="nearest") == db
    strong = db >= db.max() + min_db
    frames, bins = np.nonzero(local_max & strong)
    return np.stack([frames, bins], axis=1)


def triplet_hashes(points: np.ndarray, zone_frames: int = 120, fan_out: int = 6):
    """Pitch- and tempo-invariant hashes from peak triplets.

    For peaks p1 < p2 < p3 in time the key is the two bin differences and the
    quantized ratio (t2 - t1) / (t3 - t1); bin differences survive pitch
    shifts and the ratio survives tempo changes.
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
                key = (int(b2 - b1), int(b3 - b2), int(round(ratio * 24)))
                hashes[key].append((int(t1), int(b1)))
    return hashes


def match(query: dict, reference: dict, tolerance: int = 1):
    """Returns matched (t_query, t_ref, b_query, b_ref) tuples.

    Keys are looked up within ±`tolerance` on the quantized ratio, which
    absorbs frame rounding after a tempo change; bin differences must match
    exactly.
    """
    pairs = []
    for (d1, d2, r), q_entries in query.items():
        for dr in range(-tolerance, tolerance + 1):
            r_entries = reference.get((d1, d2, r + dr))
            if not r_entries or len(r_entries) > 8:
                continue
            for tq, bq in q_entries:
                for tr, br in r_entries:
                    pairs.append((tq, tr, bq, br))
    return np.array(pairs, dtype=float).reshape(-1, 4)


def estimate(pairs: np.ndarray):
    """Robustly estimates the pitch shift (query − reference, in bins) and
    the tempo factor (reference duration / query duration) from matched
    pairs, and counts the pairs that agree with both estimates."""
    if len(pairs) < 10:
        return dict(matches=len(pairs), consistent=0, bins=np.nan, tempo=np.nan)
    pairs = pairs[(pairs[:, 0] > 0) & (pairs[:, 1] > 0)]
    d_bins = pairs[:, 2] - pairs[:, 3]
    tempo = pairs[:, 1] / pairs[:, 0]  # t_ref / t_query: > 1 when the query is faster
    est_bins = int(np.round(np.median(d_bins)))
    est_tempo = float(np.median(tempo))
    consistent = int(np.sum((np.abs(d_bins - est_bins) <= 1) & (np.abs(tempo - est_tempo) <= 0.03)))
    return dict(matches=len(pairs), consistent=consistent, bins=est_bins, tempo=est_tempo)


def main() -> None:
    ogg = download()
    y, sr = librosa.load(ogg, sr=SR, mono=True)
    excerpt = y[int(EXCERPT[0] * sr) : int(EXCERPT[1] * sr)]
    control = y[int(CONTROL[0] * sr) : int(CONTROL[1] * sr)]
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
        "control (other 30 s of the song)": (control, 0, 1.0),
    }

    spectrograms = {}
    for name, (signal, _, _) in variants.items():
        wav = WORK / (name.split(" (")[0].replace(" ", "_").replace("+", "plus").replace("-", "minus").replace("%", "pct").replace(",", "") + ".wav")
        sf.write(wav, signal, sr, subtype="FLOAT")
        spectrograms[name] = cqt_dump(wav)
        print(f"{name:34} {spectrograms[name].shape}")

    # --- accuracy against librosa's CQT --------------------------------------
    # librosa's magnitudes scale with each filter's length (`scale=False`) or
    # its square root (`scale=True`); ours are calibrated so that a sinusoid
    # of amplitude A reads as A. Divide out the lengths and the factor two
    # between L1-normalized and amplitude-calibrated kernels to compare.
    ours = spectrograms["original"]
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

    # --- fingerprints ---------------------------------------------------------
    db = {name: to_db(m) for name, m in spectrograms.items()}
    pk = {name: peaks(d) for name, d in db.items()}
    hashes = {name: triplet_hashes(p) for name, p in pk.items()}
    ref_hashes = hashes["original"]

    rows = []
    for name, (_, applied_bins, applied_rate) in variants.items():
        pairs = match(hashes[name], ref_hashes)
        est = estimate(pairs)
        total = sum(len(v) for v in hashes[name].values())
        rows.append((name, total, est, applied_bins, applied_rate, pairs))

    print("\n| Version | Hashes | Consistent matches | Score | Applied shift (bins) | Detected | Applied tempo | Detected |")
    print("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for name, total, est, applied_bins, applied_rate, _ in rows:
        det_bins = "—" if np.isnan(est["bins"]) else f"{est['bins']:+d}"
        det_tempo = "—" if np.isnan(est["tempo"]) else f"×{est['tempo']:.3f}"
        score = 100.0 * est["consistent"] / max(total, 1)
        print(f"| {name} | {total} | {est['consistent']} | {score:.1f} % | {applied_bins:+d} | {det_bins} | ×{applied_rate:.2f} | {det_tempo} |")

    # --- figure: pitch shift and tempo change proofs --------------------------
    fig, axes = plt.subplots(2, 2, figsize=(14, 8.5), constrained_layout=True)
    for row, (name, colour) in enumerate([("pitch +2 semitones", "tab:red"), ("tempo +12 %", "tab:blue")]):
        _, _, est, applied_bins, applied_rate, pairs = next(r for r in rows if r[0] == name)
        ax = axes[row, 0]
        d = db[name]
        ax.imshow(d.T - d.max(), origin="lower", aspect="auto", cmap="magma", vmin=-80, vmax=0,
                  extent=[0, len(d) * HOP / sr, 0, d.shape[1]])
        p0 = pk["original"]
        p1 = pk[name]
        ax.scatter(p0[:, 0] * HOP / sr, p0[:, 1] + 0.5, s=6, c="white", label="peaks of the original")
        ax.scatter(p1[:, 0] * HOP / sr, p1[:, 1] + 0.5, s=6, c=colour, label=f"peaks of «{name}»")
        ax.set_title(f"«{name}»: CQT with peak constellations")
        ax.set_xlabel("time (s)")
        ax.set_ylabel("bin (24 per octave)")
        ax.legend(loc="upper right", fontsize=8)

        ax = axes[row, 1]
        if len(pairs):
            valid = (pairs[:, 0] > 0) & (pairs[:, 1] > 0)
            if row == 0:
                ax.hist(pairs[:, 2] - pairs[:, 3], bins=np.arange(-12.5, 13.5, 1), color=colour)
                ax.axvline(applied_bins, color="k", ls="--", label=f"applied: {applied_bins:+d} bins")
                ax.set_xlabel("bin offset of matched peaks (query − reference)")
                ax.set_title(f"detected shift {est['bins']:+d} bins from {est['consistent']} consistent matches")
            else:
                ax.hist(pairs[valid, 1] / pairs[valid, 0], bins=np.linspace(0.8, 1.3, 51), color=colour)
                ax.axvline(applied_rate, color="k", ls="--", label=f"applied: ×{applied_rate:.2f}")
                ax.set_xlabel("tempo factor of matched peaks (reference time / query time)")
                ax.set_title(f"detected tempo ×{est['tempo']:.3f} from {est['consistent']} consistent matches")
            ax.legend()
            ax.set_ylabel("matched triplet hashes")
    fig.suptitle("Fingerprint invariance on real music (" + ATTRIBUTION + ")", fontsize=10)
    fig.savefig(PLOTS / "song_pitch_tempo_proof.png", dpi=80)
    plt.close(fig)

    # --- figure: match scores for every version -------------------------------
    fig, ax = plt.subplots(figsize=(10, 4.8), constrained_layout=True)
    names = [r[0] for r in rows]
    scores = [100.0 * r[2]["consistent"] / max(r[1], 1) for r in rows]
    bars = ax.barh(names, scores, color=["tab:green" if "control" not in n else "tab:gray" for n in names])
    for bar, s in zip(bars, scores):
        ax.text(bar.get_width() + 0.5, bar.get_y() + bar.get_height() / 2, f"{s:.1f} %", va="center")
    ax.invert_yaxis()
    ax.set_xlabel("triplet hashes matched consistently with the original (%)")
    ax.set_title("Fingerprint match against the original 30 s excerpt")
    ax.set_xlim(0, max(scores) * 1.2)
    fig.savefig(PLOTS / "song_match_scores.png", dpi=100)
    plt.close(fig)

    json.dump(
        {r[0]: dict(hashes=r[1], **{k: (None if isinstance(v, float) and np.isnan(v) else v) for k, v in r[2].items()},
                    applied_bins=r[3], applied_rate=r[4]) for r in rows},
        open(WORK / "summary.json", "w"), indent=2, default=float,
    )


if __name__ == "__main__":
    main()
