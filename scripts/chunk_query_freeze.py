#!/usr/bin/env python3
"""Freeze the 22 E010 query records before opening native matching outputs."""
import json
from pathlib import Path
import wave
import real_mix_eval as e

ROOT = Path('target/chunk-query')


def supported_group(rows):
    good = [row for row in rows if row['similarity'] >= .45]
    groups = []
    for seed in good:
        offset = seed['mix_start'] - seed['source_start']/seed['tempo']
        group = [row for row in good if abs(row['mix_start']-row['source_start']/seed['tempo']-offset) <= 5]
        groups.append(group)
    if not groups:
        raise ValueError('No supported anchors')
    group = max(groups, key=lambda g: (len(g), sum(x['similarity'] for x in g), -min(x['mix_start'] for x in g)))
    if len(group) < 2:
        raise ValueError('Fewer than two mutually consistent source anchors')
    return group


def main():
    destination = ROOT/'queries.json'
    if destination.exists():
        raise FileExistsError('Frozen queries already exist; preserve them')
    sources = json.loads((ROOT/'sources.json').read_text())
    groups = {}
    annotations = {}
    for song in sorted(k for k in sources if k not in ['mix', 't05']):
        folder = 'alignment-centered-short' if song == 't01' else 'alignment-centered'
        path = ROOT/folder/(song+'.json')
        group = supported_group(json.loads(path.read_text()))
        groups[song] = group
        annotations[song] = dict(status='algorithmically-supported', anchors=group,
                                 anchor_file=e.identity(path), independently_human_verified=False)
    starts = {song:(min(x['mix_start'] for x in group)+max(x['mix_end'] for x in group))/2-5
              for song,group in groups.items()}
    # The publisher lists t05 between t04 and t06, but its audio correspondence
    # was not established. Preserve this query as explicitly unscored ground truth.
    gap_start = max(x['mix_end'] for x in groups['t04'])
    gap_end = min(x['mix_start'] for x in groups['t06'])
    assert gap_end-gap_start >= 10
    starts['t05'] = (gap_start+gap_end)/2-5
    annotations['t05'] = dict(status='unverified-publisher-order-gap', gap_start=gap_start, gap_end=gap_end,
                               reason='No consistent STFT correspondence to the pinned original; exclude from supported accuracy denominator',
                               independently_human_verified=False)
    assert all(starts[f't{i:02}'] < starts[f't{i+1:02}'] for i in range(1,22)), starts
    mix = sources['mix']['original']
    assert e.sha(mix['path']) == mix['sha256']
    output = ROOT/'queries'
    output.mkdir(exist_ok=True)
    records = []
    with wave.open(mix['path']) as inp:
        for song,start in sorted(starts.items()):
            sample = round(start*44100)
            inp.setpos(sample)
            data = inp.readframes(441000)
            assert len(data) == 882000
            path = output/(song+'.wav')
            with e.wav_writer(path) as out:
                out.writeframes(data)
            row = dict(song=song, title=sources[song]['track']['title'], mix_start_sample=sample,
                       mix_start_seconds=sample/44100, samples=441000, seconds=10,
                       annotation=annotations[song], **e.identity(path))
            records.append(row)
            print(song, f'{start:.3f}s', annotations[song]['status'], flush=True)
    e.save(destination, dict(status='frozen-before-native-query-evaluation', evaluator=e.identity(__file__),
                             sources=e.identity(ROOT/'sources.json'), mix=mix,
                             selection_rule='Center of longest consistent anchor group; t05 is the unverified gap midpoint',
                             queries=records))


if __name__ == '__main__':
    main()
