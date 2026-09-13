#!/usr/bin/env python3
"""Download the pinned E010 corpus and preserve sample-exact reference chunks."""
import concurrent.futures
import json
from pathlib import Path
import struct
import urllib.request
import wave
import real_mix_eval as e

ROOT = Path('target/chunk-query')
RATE = 44100
CHUNK = 10 * RATE


def prepare_one(track, expected):
    song = track['id']
    media = ROOT / 'media'
    source = media / f'{song}.mp3'
    if not source.exists():
        request = urllib.request.Request(track['url'], headers={'User-Agent': 'cqt-rs research experiment'})
        temporary = source.with_suffix('.partial')
        with urllib.request.urlopen(request, timeout=120) as response, temporary.open('wb') as out:
            resolved = response.url
            while block := response.read(1024 * 1024):
                out.write(block)
        temporary.replace(source)
    else:
        resolved = None
    assert e.sha(source) == expected['file']['sha256'], (song, 'source identity')
    full = media / f'{song}-full.wav'
    commands = []
    if not full.exists():
        data = bytearray(e.ffmpeg(['-i', source, '-ac', '1', '-ar', RATE,
                                  '-c:a', 'pcm_s16le', '-f', 'wav', '-'], commands))
        # FFmpeg cannot seek a pipe to finalize its WAV header. Finalize only
        # RIFF/data sizes, preserving the exact expected FFmpeg PCM/WAV bytes.
        chunk = 12
        while data[chunk:chunk+4] != b'data':
            length = struct.unpack_from('<I', data, chunk+4)[0]
            chunk += 8 + length + length % 2
            if chunk+8 > len(data):
                raise ValueError('FFmpeg WAV has no data chunk')
        struct.pack_into('<I', data, 4, len(data)-8)
        struct.pack_into('<I', data, chunk+4, len(data)-chunk-8)
        temporary = full.with_suffix('.tmp')
        temporary.write_bytes(data)
        temporary.replace(full)
    info = e.wav_info(full)
    if song != 'mix':
        assert e.sha(full) == expected['original']['sha256'], (song, 'PCM identity')
    record = dict(track=track, resolved_url=resolved, source=e.identity(source), original=e.identity(full),
                  **info, commands=commands, chunks=[])
    if song == 'mix':
        return song, record
    chunk_dir = ROOT / 'chunks' / song
    chunk_dir.mkdir(parents=True, exist_ok=True)
    with wave.open(str(full)) as inp:
        for number, start in enumerate(range(0, info['samples'], CHUNK)):
            count = min(CHUNK, info['samples'] - start)
            path = chunk_dir / f'{song}-c{number:03}.wav'
            inp.setpos(start)
            with e.wav_writer(path) as out:
                data = inp.readframes(count)
                assert len(data) == count * 2
                out.writeframes(data)
            record['chunks'].append(dict(id=f'{song}-c{number:03}', song=song, start_sample=start,
                                         samples=count, start_seconds=start/RATE, seconds=count/RATE,
                                         partial=count < CHUNK, **e.identity(path)))
        start = (info['samples'] - CHUNK) // 2
        inp.setpos(start)
        path = media / f'{song}-midpoint.wav'
        with e.wav_writer(path) as out:
            out.writeframes(inp.readframes(CHUNK))
        assert e.sha(path) == expected['reference']['sha256'], (song, 'midpoint identity')
        record['midpoint'] = dict(id=song, song=song, start_sample=start, samples=CHUNK,
                                  start_seconds=start/RATE, seconds=10, **e.identity(path))
    return song, record


def main():
    ROOT.joinpath('media').mkdir(parents=True, exist_ok=True)
    plan = json.loads(Path('experiments/toucan2020.json').read_text())
    expected = json.loads(Path('docs/evidence/E009/source-manifest.json').read_text())
    expected['mix'] = {'file': {'sha256': '39d7923a20de5053d56d126eb499a2f6fe7de2cd393a1310566a3e826d72e103'}}
    tracks = [*plan['tracks'], {'id': 'mix', 'url': plan['mix_url']}]
    records = {}
    e.save(ROOT/'preparation.json', dict(status='running', evaluator=e.identity(__file__)))
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
            futures = [pool.submit(prepare_one, t, expected[t['id']]) for t in tracks]
            for future in concurrent.futures.as_completed(futures):
                song, record = future.result()
                records[song] = record
                e.save(ROOT/'sources.json', records)
                print(f"Prepared {song}: {record['seconds']:.3f}s, {len(record['chunks'])} chunks", flush=True)
    except BaseException as ex:
        e.save(ROOT/'preparation.json', dict(status='failed', error=repr(ex), evaluator=e.identity(__file__)))
        raise
    e.save(ROOT/'preparation.json', dict(status='completed', evaluator=e.identity(__file__),
                                        sources=e.identity(ROOT/'sources.json')))


if __name__ == '__main__':
    main()
