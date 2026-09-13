# Reproduce the short-reference, voiceover and EQ experiment

The experiment used public Creative Commons recordings, fixed ten-second
references and three pinned native monitor builds. The candidate changes only
the alignment estimate through `--modal-fit`; confidence and verification gates
remain at their defaults. [Results](E009-results.md) distinguish controlled
snippet detection from coverage of the original DJ mix.

## 1. How we obtained the recordings

The starting mix was **Toucan Music 2005 to 2020**, from the publisher's
[mix page](https://www.toucanmusic.com/mixes/tou2020). The original E007 selection
used seed `20260913` and the ordered catalog in
[toucan2020.json](../../experiments/toucan2020.json); that file freezes the
chosen mix URL and all 22 publisher-listed original track URLs. E009 reused
this already known mix as regression data. It was not another random draw,
and its results do not represent all radio streams.

Download the mix from the manifest's `mix_url`; follow redirects. Required
SHA-256: `39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103`.
Download each listed original from its own URL. We used the publisher's track
list to establish identities; the detector did not generate the ground-truth
track list. The publisher supplies track order without independent cue times.

For additional recordings, we froze eight named Kevin MacLeod tracks from
Incompetech: Cipher, Electrodoodle, Cut and Run, Wallpaper, Funkorama, Carefree,
Hyperfun and Sneaky Snitch. They were selected before viewing their detector
outputs. Eight separate music negatives were sampled with Python
`random.Random(20260913)` from the publisher's genre-7 catalog, sorted by title,
excluding the held-out titles and titles containing ` - `. The exact selected
files, version-specific names, ISRCs and source pages are fixed in
[recognition-holdout.json](../../experiments/recognition-holdout.json); use those
URLs rather than resampling a catalog that may change. For example, Cipher's
actual download is `Cipher2.mp3`, and Shiny Tech II is `Shiny%20Tech2.mp3`.

Toucan material is CC BY-NC-SA 4.0, with
[track/artist attribution](../evidence/E007/ATTRIBUTION.txt). The additional
Kevin MacLeod recordings use CC BY 4.0 under the
[publisher's licence](https://incompetech.com/music/royalty-free/licenses/).
Both require attribution; the Toucan material also has noncommercial/share-alike
conditions. Full source audio is not included in the repository.

The published [source manifest](../evidence/E009/source-manifest.json) contains
all 38 source hashes, decoded/reference hashes, source durations and exact cut
positions. Treat a changed download as different data, not a matching rerun.

## 2. Decode and take ten seconds from each original

Decode with FFmpeg to **44,100 Hz, mono, signed 16-bit PCM**. If the decoded
original has `N` samples, the reference starts at `(N - 441000) // 2` and contains
exactly `441000` samples. We slice PCM by sample index using Python's `wave`
module, avoiding compressed-audio seeking ambiguity. The originals stay clean;
transformations are applied to the query stream only.

For example, t01's reference starts at 187.65451247165532 seconds. Its WAV hash
is `dcf840e5ecd8fd237ca564cbcb0cd0a971d32d0fcec73915c28a0907eed33666`.
All 22 development references match the earlier E007 references byte-for-byte.
Only ten seconds per song are indexed, so a mix can omit the indexed passage.

The downloader/decoder/slicer is
[`prepare_recognition_sources.py`](../../scripts/prepare_recognition_sources.py).
Run it in a fresh directory; an existing cache is reused. Verify every source,
decoded original and reference against the published manifest before accepting
the comparison. MP3 decode and filter versions can affect PCM bytes.

## 3. Environment and pinned builds

Observed execution environment: Linux x86-64, glibc 2.39, Python 3.12.14,
NumPy 2.3.5 and FFmpeg 6.1.1-3ubuntu5 with `rubberband` and `flite` enabled.
Binaries were built on Ubuntu 22.04 using Rust 1.98.1 and
[`Cargo.lock.txt`](../evidence/E007/Cargo.lock.txt). All native invocations set
`RAYON_NUM_THREADS=1`. Single elapsed timings do not establish a speedup.

The existing [binary export workflow](../../.github/workflows/recognition-lab.yml)
shows the actual build commands. The measured candidate's
[build artifact](https://github.com/F0rty-Tw0/cqt-rs/actions/runs/34759390428)
also contains the binaries and source identities, with limited retention.
To rebuild independently, run this from the repository root with all commits
available and the Rust toolchain installed:

```bash
mkdir -p target/lab/binaries target/lab/final-binaries
lab_build=$(mktemp -d)
for side in pr3 pr7 candidate; do
  case "$side" in
    pr3) source_sha=7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a ;;
    pr7) source_sha=cdf428e9d6a82127cf36aedd8ddea27f82de81ec ;;
    candidate) source_sha=69c09c4be5fb391a9e3d93ff4465cc4864d0ddf4 ;;
  esac
  git worktree add --detach "$lab_build/$side" "$source_sha"
  cp docs/evidence/E007/Cargo.lock.txt "$lab_build/$side/Cargo.lock"
  cargo +1.98.1 build --release --locked -p cqt-monitor \
    --manifest-path "$lab_build/$side/Cargo.toml" \
    --target-dir "$lab_build/build-$side"
  if [ "$side" = candidate ]; then
    cp "$lab_build/build-$side/release/monitor" target/lab/final-binaries/candidate
    git -C "$lab_build/$side" rev-parse HEAD > target/lab/final-binaries/candidate-commit.txt
  else
    cp "$lab_build/build-$side/release/monitor" "target/lab/binaries/$side"
  fi
done
```

Record your toolchain, lockfile, source/dirty state and executable hashes.
Our hashes are in the [validation record](../evidence/E009/README.md); a new
host may produce different executable bytes even from the same source.

## 4. What we did to the audio

Each controlled positive stream concatenates the known snippets in ID order.
Every snippet has five seconds of silence before it and ten seconds after it.
The evaluator records exact sample positions as truth before detection. There
are 22 development and eight held-out songs, each with twenty treatments.

| Treatment | Exact operation |
| --- | --- |
| Self control | Original PCM, unchanged |
| Clean | Gain 0.15 for shared headroom |
| Pitch | Gain 0.15, `rubberband=pitch=1.122462048309373` (+2 semitones) |
| Tempo | Gain 0.15, `rubberband=tempo=0.9` (90% BPM; duration divided by 0.9) |
| Pitch + tempo | Gain 0.15, `rubberband=tempo=0.9:pitch=1.122462048309373` |
| Voice | Clean music plus generated speech at equal local RMS |
| Combined | Pitch + tempo treatment plus the same speech |
| Clipping | Clip original amplitudes at 25% of that clip's peak, then rescale for headroom |

The other twelve treatments are separately applied EQ filters, with common
gain 0.15 and FFmpeg f64 precision:

- Bass shelf at 200 Hz and treble shelf at 3000 Hz, each at -12, -6, +6 and
  +12 dB; Q=0.707, two poles.
- Bell cuts at 1000 Hz, -12 dB, Q=0.7 and Q=5.
- Two-pole high-pass at 200 Hz and low-pass at 3000 Hz.

For example: `volume=0.15,bass=f=200:t=q:w=0.707:g=-12:p=2:r=f64`.
The complete literal filter strings are in `recognition_lab.py`. The prepared
positive EQ outputs were checked for absence of PCM clipping. EQ settings are
tested individually, not combined with pitch, tempo and speech in this sweep.

Speech is synthesized with FFmpeg `flite`, voice `slt`, using the fixed
[`voice.txt`](../evidence/E007/voice.txt). `real_mix_eval.voice_mix` takes its
first twelve seconds. For each thirty-second music block, it scales speech to
the RMS of the music over the overlaid interval, adds it, and applies a shared
headroom scale `min(1, 0.95 / peak)` before conversion to PCM. Speech therefore
covers each entire controlled 10-second (or slowed 11.11-second) snippet.

The actual 90.61-minute DJ mix uses four treatments: clean, pitch, tempo and
combined. Those use gain **0.25**, and voice occurs for the first twelve seconds
of each thirty-second block. Thus the controlled speech cases and mix speech
case have different speech exposure. Slowed mix duration is about 100.68 minutes.

Negative controls watch all thirty positive references while playing eight
different recordings: their full originals (24.93 minutes total), plus eight
one-minute midpoint excerpts under self, clean and all twelve EQ treatments.
These excerpts overlap the full songs; do not count the variants as independent
additional recordings or sum them into independent exposure.

## 5. Commands to generate and run the cases

Use a fresh checkout/work directory for a rerun. The commands create new audio,
truth manifests and outputs locally; no large evidence download is needed.

```bash
python3 -m pip install numpy==2.3.5
mkdir -p target/lab
python3 scripts/prepare_recognition_sources.py
python3 - <<'PY'
import json
from pathlib import Path
expected=json.loads(Path('docs/evidence/E009/source-manifest.json').read_text())
actual=json.loads(Path('target/lab/sources.json').read_text())
for song, record in expected.items():
    for kind in ['file', 'original', 'reference']:
        assert actual[song][kind]['sha256']==record[kind]['sha256'], (song, kind)
print('All 38 source, decoded and reference hashes agree.')
PY
python3 scripts/recognition_lab.py --prepare
python3 scripts/recognition_lab.py --split development --candidate target/lab/final-binaries/candidate --label rerun
python3 scripts/recognition_lab.py --split heldout --candidate target/lab/final-binaries/candidate --label rerun
python3 scripts/recognition_lab.py --split negative --candidate target/lab/final-binaries/candidate --label rerun
python3 scripts/recognition_lab.py --peaks
python3 scripts/recognition_mix_regression.py --prepare
python3 scripts/recognition_mix_regression.py --candidate target/lab/final-binaries/candidate --label rerun
```

The mix preparer has an optional reuse path from the original execution; when
that file is absent it downloads the pinned mix and requires the original hash.
`target/lab/evidence/rerun/{development,heldout,negative,mix}` receives metadata,
commands, stdout/stderr and checkpointed `results.json`. Use a different label
for another run: the harness refuses to overwrite an existing result set.

There are 60 development runs, 60 held-out runs, 30 negative runs and 12 mix
runs: **162 selected cases**. Native runs alternate baseline/candidate ordering
by treatment and use the same reference sets and default thresholds. A direct
candidate invocation is:

```bash
RAYON_NUM_THREADS=1 target/lab/final-binaries/candidate --modal-fit \
  --watch song=target/lab/media/t01-10s.wav --stream query.wav
```

## 6. How we scored and what should reproduce

For controlled streams, a correct start must name the song and occur during
its known sample window or within one second after its end. A wrong identity
or out-of-window start is false. Additional starts in the valid window are
duplicates; a song without a valid start is a miss. An early false start must
not conceal a later correct one. Start delay uses consumed audio minus the
known start. Position error compares the reported song position with the
known reference position at the reported audio frame. Delay summaries include
detected plays only, so different recall can change their denominators.

For the real mix we count unique identities in the publisher's list, retaining
all starts. Without independently labelled cue times this is coverage, not
verified per-play precision; extra starts can be fragmentation or errors.

PR #3 and PR #7 should have identical non-timing events. The known PR #3 stream
duration JSON defect is repaired only while parsing that field; original logs
stay untouched. Elapsed timing fields are excluded from event parity. Input,
binary and evaluator hashes are recorded before work; each completed case is
checkpointed. Check exit codes, terminal `done` events, durations and hashes.

Expected mix coverage (PR #3/PR #7 → modal): **14→15 clean, 12→12 pitch,
12→13 slowed, 5→8 combined**, out of 22. Controlled modal EQ detects all 22
development and all eight held-out recordings in each setting. Held-out EQ
was already 8/8 on the baseline; held-out voice and combined remain 6/8 and
5/8. Zero false starts occurred in the controlled/negative grid, with limited
negative exposure. No previously recovered identity was lost.

The independent local review also compared 390 native fingerprint cases:
reference peak survival within four frames/one bin, and exact hash-key multiset
overlap. Those are feature-survival diagnostics, not recognition by themselves.

The original completed run needed eight exact-command replays after raw logs
were found truncated; their non-timing digests agreed with the original records.
Nine auxiliary audio caches and two fingerprint dumps were restored only when
their original hashes matched. Detector inputs retained their hashes. The
[failure record](../evidence/E009/README.md) makes these limitations explicit.

The candidate remains experimental: repeated passages can yield a roughly
8.7-second position error, and the held-out recordings demonstrate preserved
outcomes rather than new gains. The next experiment should freeze new
recordings and compare distributed reference excerpts, followed by speech
robustness. These measurements do not establish a performance improvement.
