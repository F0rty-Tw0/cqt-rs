# E012 matched-source diagnostic

All four approximately corresponding clean-source clips are correctly recognized, with no wrong-parent starts. The same four random mix queries remain no-match under full-song modal fitting. These four adaptive controls are separate from the 288 main runs.

| Track | Random mix start | Approximate clean source start | Mix | Clean source |
| --- | ---: | ---: | --- | --- |
| t04: Beautiful Geometry | 576.507075s | 208.916939s | no-match | correct |
| t10: Pioneers | 1971.946236s | 196.028390s | no-match | correct |
| t13: The Piano Tune | 2756.275805s | 110.046848s | no-match | correct |
| t22: Are You Feeling It? (Rawbase Remix) | 5352.501451s | 266.401383s | no-match | correct |

Each clean cut is ten untransformed seconds, starting at the position predicted by the highest-similarity covering frozen STFT anchor. Anchor resolution and tempo estimation make this an approximate passage comparison, not sample-exact isolation of a DJ effect.

The same pinned binary, complete-song database, modal option and confidence/alignment thresholds are used. The outcome supports investigating fingerprint survival in mixed audio; it does not establish whether EQ, tempo, pitch, layering, encoding or a combination is responsible.

Native commands, positions, source/query hashes, evaluator identity and all raw outputs are in [matched-source evidence](../evidence/E011/matched-source-diagnostics.zip). Reproduce with `python3 scripts/song_index_matched_controls.py` after E011/E012 preparation and main evaluation.
