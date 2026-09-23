# /// script
# requires-python = "==3.12.*"
# dependencies = ["numpy", "librosa", "essentia", "nnAudio", "torch"]
#
# [tool.uv]
# exclude-newer = "2026-09-23T00:00:00Z"
#
# [[tool.uv.index]]
# name = "pytorch-cpu"
# url = "https://download.pytorch.org/whl/cpu"
# explicit = true
#
# [tool.uv.sources]
# torch = { index = "pytorch-cpu" }
# ///
"""Times the Python CQTs on the signals compare-rs wrote, on one thread.

Usage: uv run scripts/compare/compare_py.py <signal-dir>
Prints markdown table rows.
"""

import os

# One thread everywhere; must be set before numpy / torch load.
for var in ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "NUMBA_NUM_THREADS"):
    os.environ[var] = "1"

import sys
import time
from importlib.metadata import version
from pathlib import Path

import numpy as np

# Same grids as compare-rs (benches/bench.rs); n_bins matches cqt-rs.
GRIDS = {
    "legacy": dict(sr=22_000, fmin=14.568, fmax=7_902.1, bpo=12, n_bins=109, hop=1_760),
    "fingerprint": dict(sr=44_100, fmin=55.0, fmax=7_040.0, bpo=24, n_bins=169, hop=512),
}


def median_ms(run):
    """Median wall time in ms after one warm-up call; ~2 s, 5 to 51 repeats."""
    start = time.perf_counter()
    run()
    first = time.perf_counter() - start
    repeats = min(max(int(2.0 / first), 5), 51)
    times = []
    for _ in range(repeats):
        start = time.perf_counter()
        run()
        times.append((time.perf_counter() - start) * 1e3)
    return sorted(times)[repeats // 2]


def fmt(value):
    return f"{value:.2f} ms" if value < 10 else f"{value:.1f} ms"


def time_librosa(signal, g):
    import librosa

    return median_ms(
        lambda: librosa.cqt(
            signal, sr=g["sr"], hop_length=g["hop"], fmin=g["fmin"],
            n_bins=g["n_bins"], bins_per_octave=g["bpo"],
        )
    )


def time_essentia(signal, g):
    import essentia.standard as es

    cqt = es.NSGConstantQ(
        inputSize=len(signal), sampleRate=g["sr"], minFrequency=g["fmin"],
        maxFrequency=g["fmax"], binsPerOctave=g["bpo"],
    )
    return median_ms(lambda: cqt(signal))


def time_nnaudio(signal, g):
    import torch
    from nnAudio.features import CQT1992v2

    torch.set_num_threads(1)
    cqt = CQT1992v2(
        sr=g["sr"], hop_length=g["hop"], fmin=g["fmin"], n_bins=g["n_bins"],
        bins_per_octave=g["bpo"], verbose=False,
    )
    x = torch.from_numpy(signal)[None, :]

    def run():
        with torch.no_grad():
            cqt(x)

    return median_ms(run)


LIBRARIES = [
    ("librosa", "`librosa.cqt`", time_librosa, "builds its filters inside the call, `res_type` default"),
    ("essentia", "`NSGConstantQ`", time_essentia, "invertible NSGT over the whole signal, no hop; configured outside the timed call"),
    ("nnAudio", "`CQT1992v2`, CPU", time_nnaudio, f"PyTorch {version('torch')}, `torch.set_num_threads(1)`; kernels built outside the timed call"),
]


def main():
    signal_dir = Path(sys.argv[1])
    signals = {name: np.fromfile(signal_dir / f"{name}.f32", dtype="<f4") for name in GRIDS}
    for package, api, timer, notes in LIBRARIES:
        cells = []
        for name, grid in GRIDS.items():
            try:
                cells.append(fmt(timer(signals[name], grid)))
            except Exception as error:  # report, never carry an old number
                cells.append(f"not reproduced: {type(error).__name__}: {error}".replace("\n", " "))
        try:
            label = f"{package} {version(package)} ({api})"
        except Exception:
            label = f"{package} ({api})"
        print(f"| {label} | {cells[0]} | {cells[1]} | {notes} |", flush=True)


if __name__ == "__main__":
    main()
