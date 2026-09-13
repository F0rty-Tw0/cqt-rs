#!/usr/bin/env python3
"""Freeze E011's corrected catalogue and four paired ten-second query sets."""
import concurrent.futures
import copy
import hashlib
import io
import json
from pathlib import Path
import random
import re
import subprocess
import urllib.request
import wave

import numpy as np
import chunk_query_align as align
from chunk_query_freeze import supported_group
import real_mix_eval as e

ROOT = Path('target/song-index')
OLD = Path('target/chunk-query')
RATE = 44100
LENGTH = 10 * RATE
CORRECT_URL = 'https://archive.org/download/tou285/tou285_dave_kent_roots_and_shoots.mp3'
CORRECT_SHA1 = '3d62d606c3f6665ac9677db9956afe231de545c0'


def atomic_bytes(path, data):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_bytes(data)
    temporary.replace(path)


def pcm_wav(path, pcm):
    buffer = io.BytesIO()
    with wave.open(buffer, 'wb') as out:
        out.setparams((1, 2, RATE, 0, 'NONE', 'not compressed'))
        out.writeframes(pcm)
    atomic_bytes(path, buffer.getvalue())


def cut(source, start, count, destination):
    with wave.open(str(source)) as inp:
        inp.setpos(start)
        pcm = inp.readframes(count)
    if len(pcm) != 2 * count:
        raise ValueError(('incomplete PCM', source, start, count, len(pcm)))
    pcm_wav(destination, pcm)
    return dict(start_sample=start, start_seconds=start/RATE,
                samples=count, seconds=count/RATE, **e.identity(destination))


def download(url, path):
    if not path.exists():
        request = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
        with urllib.request.urlopen(request, timeout=120) as response:
            atomic_bytes(path, response.read())
    return e.identity(path)


def decode(source, path):
    if not path.exists():
        result = subprocess.run(['ffmpeg', '-v', 'error', '-nostdin', '-i', str(source),
                                 '-ac', '1', '-ar', str(RATE), '-f', 's16le', '-'],
                                capture_output=True, check=True)
        pcm_wav(path, result.stdout)
    return dict(**e.identity(path), **e.wav_info(path))


def tags(path):
    result = subprocess.run(['ffprobe', '-v', 'error', '-show_entries',
                             'format_tags=title,artist', '-of', 'json', str(path)],
                            capture_output=True, check=True)
    return json.loads(result.stdout)['format'].get('tags', {})


def base_title(title):
    return re.sub(r'[^a-z0-9]', '', title.split('(')[0].lower())


def verify_title(track, actual):
    if base_title(track['title']) != base_title(actual.get('title', '')):
        raise ValueError(('source title mismatch', track['title'], actual))


def intervals_for(anchors):
    """Union of inclusive legal start-sample intervals; no overlap weighting."""
    intervals = sorted((round(a['mix_start']*RATE), round(a['mix_end']*RATE)-LENGTH)
                       for a in anchors if a['mix_end']-a['mix_start'] >= 10)
    merged = []
    for lo, hi in intervals:
        if merged and lo <= merged[-1][1] + 1:
            merged[-1][1] = max(hi, merged[-1][1])
        else:
            merged.append([lo, hi])
    return merged


def choose_start(intervals, rng):
    count = sum(hi-lo+1 for lo, hi in intervals)
    if count <= 0:
        raise ValueError('No legal ten-second query interval')
    draw = rng.randrange(count)
    for lo, hi in intervals:
        size = hi-lo+1
        if draw < size:
            return lo + draw
        draw -= size
    raise AssertionError('unreachable')


def prepare_negative(key, expected):
    source = ROOT/'media'/(key+'.mp3')
    record = download(expected['url'], source)
    assert record['sha256'] == expected['file']['sha256'], key
    full = ROOT/'media'/(key+'-full.wav')
    info = decode(source, full)
    assert info['samples'] == round(expected['source_seconds']*RATE), key
    start = (info['samples']-LENGTH)//2
    query = cut(full, start, LENGTH, ROOT/'queries'/(key+'.wav'))
    assert query['sha256'] == expected['reference']['sha256'], key
    return dict(id=key, group='negative', song=None, title=expected['track']['title'],
                supported=True, source=record, attribution=expected['track'], **query)


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    if (ROOT/'manifest.json').exists():
        raise FileExistsError('Preserve the frozen E011 manifest')
    sources = copy.deepcopy(json.loads((OLD/'sources.json').read_text()))
    audit = []
    for song, record in sorted(sources.items()):
        for field in ['source', 'original']:
            assert e.sha(record[field]['path']) == record[field]['sha256'], (song, field)
        if song == 'mix':
            continue
        actual = tags(record['source']['path'])
        row = dict(song=song, expected=record['track'], observed=actual,
                   source=record['source'], title_matches=base_title(record['track']['title']) == base_title(actual.get('title','')))
        audit.append(row)
        if song != 't05':
            verify_title(record['track'], actual)
    e.save(ROOT/'source-tag-audit.json', audit)
    assert not next(r for r in audit if r['song']=='t05')['title_matches']
    source = ROOT/'media/t05.mp3'
    corrected_file = download(CORRECT_URL, source)
    assert hashlib.sha1(source.read_bytes()).hexdigest() == CORRECT_SHA1
    actual = tags(source)
    verify_title(sources['t05']['track'], actual)
    assert actual['artist'] == 'Dave Kent'
    full = ROOT/'media/t05-full.wav'
    decoded = decode(source, full)
    corrected = dict(track=dict(sources['t05']['track'], url=CORRECT_URL),
                     source=corrected_file, original=e.identity(full),
                     samples=decoded['samples'], seconds=decoded['seconds'], rate=RATE, chunks=[])
    for number, start in enumerate(range(0, decoded['samples'], LENGTH)):
        count = min(LENGTH, decoded['samples']-start)
        chunk_id = f't05-c{number:03}'
        corrected['chunks'].append(dict(id=chunk_id, song='t05', partial=count<LENGTH,
            **cut(full, start, count, ROOT/'chunks/t05'/(chunk_id+'.wav'))))
    correction = dict(status='source-identity-corrected-before-native-evaluation',
                      publisher_page='https://www.toucanmusic.com/releases/tou285',
                      archive_metadata='https://archive.org/metadata/tou285',
                      archive_file_sha1=CORRECT_SHA1, old=sources['t05'], new=corrected,
                      tags=actual, license='http://creativecommons.org/licenses/by-nc-sa/3.0/')
    sources['t05'] = corrected
    e.save(ROOT/'source-correction.json', correction)
    e.save(ROOT/'sources.json', sources)
    print('Corrected t05:', decoded['seconds'], actual, flush=True)

    expected = json.loads(Path('docs/evidence/E009/source-manifest.json').read_text())
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        pending = [pool.submit(prepare_negative, key, expected[key]) for key in sorted(expected) if key.startswith('n')]
        negatives = [future.result() for future in pending]
    e.save(ROOT/'negatives.json', negatives)

    # Independent annotation; do not consult native detector candidates.
    features = align.features(full)
    np.save(ROOT/'t05-features.npy', features)
    mix_features = np.load(OLD/'alignment-centered/mix.npy')
    anchors = align.search(mix_features, features)
    e.save(ROOT/'t05-anchors.json', anchors)
    try:
        group = supported_group(anchors)
        annotation_search = dict(template_seconds=60, source_stride=30, mix_window='whole mix')
    except ValueError as ex:
        # Same bounded fallback as E010, with its pre-existing publisher-order
        # interval. Save the unsuccessful long-template attempt unchanged.
        e.save(ROOT/'t05-long-annotation-failure.json', dict(error=str(ex), anchors=e.identity(ROOT/'t05-anchors.json'),
               next_search=dict(template_seconds=20, source_stride=10, mix_window=[700,1000])))
        short = align.search(mix_features[:,700:1000], features, seconds=20, stride=10)
        for row in short:
            row['mix_start'] += 700
            row['mix_end'] += 700
        e.save(ROOT/'t05-short-anchors.json', short)
        group = supported_group(short)
        annotation_search = dict(template_seconds=20, source_stride=10, mix_window=[700,1000])
    annotation = dict(status='algorithmically-supported', anchors=group,
                      independently_human_verified=False, method=align.PARAMETERS,
                      source=corrected['original'], aligner=e.identity(align.__file__), search=annotation_search)
    e.save(ROOT/'t05-annotation.json', annotation)
    print('t05 independently aligned:', [(a['mix_start'],a['mix_end'],round(a['similarity'],3)) for a in group], flush=True)

    old_queries = json.loads((OLD/'queries.json').read_text())
    annotations = {q['song']:q['annotation'] for q in old_queries['queries']}
    annotations['t05'] = annotation
    references = dict(chunks=[], full=[])
    queries = []
    for song in sorted(s for s in sources if s != 'mix'):
        record = sources[song]
        references['chunks'].extend(record['chunks'])
        references['full'].append(dict(id=song, song=song, start_sample=0, start_seconds=0,
            samples=record['samples'], seconds=record['seconds'], **record['original']))
        complete = [c for c in record['chunks'] if not c['partial']]
        exact = complete[len(complete)//2]
        queries.append(dict(id='exact-'+song, group='exact', song=song, title=record['track']['title'],
                            supported=True, selection='middle complete database chunk',
                            **{k:exact[k] for k in ['path','bytes','sha256','start_sample','start_seconds','samples','seconds']}))
        intervals = [[0, record['samples']-LENGTH]]
        seed = 'E011:20260913:clean:'+song
        start = choose_start(intervals, random.Random(seed))
        query = cut(record['original']['path'], start, LENGTH, ROOT/'queries'/('clean-'+song+'.wav'))
        queries.append(dict(id='clean-'+song, group='clean', song=song, title=record['track']['title'],
                            supported=True, seed=seed, legal_start_intervals=intervals, **query))
        old = next(q for q in old_queries['queries'] if q['song']==song)
        start = old['mix_start_sample']
        intervals = intervals_for(annotations[song]['anchors'])
        # Preserve E010's support rule for existing labels. Its t01 center
        # lies between consistent anchors, while random cuts use the stricter
        # within-anchor rule. Only t05 receives a newly established label.
        supported = (any(lo <= start <= hi for lo,hi in intervals) if song=='t05'
                     else old['annotation']['status']=='algorithmically-supported')
        queries.append(dict(id='frozen-'+song, group='frozen', song=song, title=record['track']['title'],
                            supported=supported, annotation=annotations[song],
                            start_sample=start, start_seconds=start/RATE,
                            **{k:old[k] for k in ['path','bytes','sha256','samples','seconds']}))
        seed = 'E011:20260913:mix:'+song
        start = choose_start(intervals, random.Random(seed))
        query = cut(sources['mix']['original']['path'], start, LENGTH, ROOT/'queries'/('mix-'+song+'.wav'))
        queries.append(dict(id='mix-'+song, group='mix', song=song, title=record['track']['title'],
                            supported=True, seed=seed, legal_start_intervals=intervals,
                            annotation=annotations[song], **query))
    queries.extend(negatives)
    for rows in [*references.values(), queries]:
        for row in rows:
            assert e.sha(row['path']) == row['sha256'], row['path']
    manifest = dict(status='frozen-before-native-E011-evaluation', evaluator=e.identity(__file__),
                    sources=e.identity(ROOT/'sources.json'), source_correction=e.identity(ROOT/'source-correction.json'),
                    queries=queries, references=references, query_count=len(queries),
                    seed_rule='Python Random seeded separately by E011:20260913:<clean|mix>:<song>')
    e.save(ROOT/'manifest.json', manifest)
    print('Frozen', len(queries), 'queries;', {k:len(v) for k,v in references.items()}, flush=True)
    print('Unsupported frozen cues:', [q['song'] for q in queries if not q['supported']], flush=True)


if __name__ == '__main__':
    main()
