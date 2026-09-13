# E007: ten-second references against an independently produced DJ mix

Status: running. Requested by the user on 2026-09-13; this takes priority
over the next profiling task. No detector thresholds or algorithms change.

## Frozen design, before recognition results

Sample one of the ten mixes on the first Toucan catalogue page using
Python Random(20260913): the selected mix is **Toucan Music 2005 to 2020**,
90 minutes, 22 listed tracks. The selection is random within that catalogue,
not a random sample of all music. Resolve the exact versions from the
publisher's links, including remix/edit labels. Track identities and URLs
are frozen in `experiments/toucan2020.json`.

- [Mix, source track list and licence](https://www.toucanmusic.com/mixes/tou2020).
- [Publisher's royalty-free terms](https://www.toucanmusic.com/licensing/).
- Licence: CC BY-NC-SA 4.0 for the mix, royalty-free for this noncommercial
  experiment, with attribution to Toucan Music and the listed artists.
  This is not a blanket commercial-use licence. Record source/audio hashes;
  keep full audio out of Git. Any shared transformed audio retains attribution
  and the applicable noncommercial/share-alike terms.

Build clean **10-second references centered at the midpoint of each original
track**, selected without consulting detector results. Midpoints can fall
in a passage omitted from the DJ mix; count these failures and do not move
the excerpts after seeing results. Full-track references are a diagnostic
control to distinguish short-reference coverage from detector robustness.

Compare pinned PR #3 `7f7374e7ddfbf75a5d3e30c70d0d9076779e2c5a` with
current PR #7 `176bce2fef1d1463ff7b815f4f0ee62a21deb435`, using the same
Rust toolchain, Cargo.lock, mono 44100 Hz PCM and default detector settings.
The experiment harness is overlaid separately; pin executable hashes.

Treatments: clean, pitch +2 semitones only, tempo 0.9 only, both plus
voiceover. Use FFmpeg's rubberband filter, with pitch factor `2^(2/12)`
and independent tempo factor 0.9. Slow streams become about 100 minutes.
Voiceover: original test text spoken by Flite, 12 seconds every 30 seconds,
matched to the music's RMS within each spoken block (0 dB ratio), with a
common headroom scale to avoid clipping. Report actual RMS and sample
counts; synthetic speech is a controlled interference, not a real DJ corpus.

Run both versions on every ten-second-reference treatment; also run clean
and combined treatments with full-track references. Run a five-minute
speech/silence negative control. Require the full clean original-source
10-second snippets to identify themselves in a separate controlled stream,
so absent mix coverage is not silently called a detector failure.

Report all 22 tracks, detected/missed by treatment and version, first start
times, confidence/verification evidence and additional starts. A publisher
track list supports track-level retrieval coverage; it alone does not give
exact audible boundaries or false-start attribution inside the mix. Do not
derive ground truth from the detector under test. If independent cue times
are unavailable, label that limit and do not claim precise onset latency or
full precision. Negative-control starts are unambiguous false positives.

No tuning is permitted after opening this test. Acceptance of the existing
optimization requires identical non-timing events for matched inputs;
retain any divergence and investigate it. Detection success is a measured
outcome, not a gate to weaken or a reason to substitute easier tracks.
This single new mix is an exploratory out-of-development-corpus case,
not proof of generalization across independent mixes or recording families.

Resource budget: one 35-minute CI execution plus fixes for concrete setup
or scoring defects. Recognition runs are not a repeated performance
benchmark; report their observed elapsed time descriptively and make no
speedup claim from one execution. Preserve raw output, hashes, transforms,
source identities and tool versions as the proof artifact.

## Results

Pending execution. Do not interpret this protocol as a completed test.
