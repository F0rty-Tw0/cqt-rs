#!/usr/bin/env python3
"""Recover E011's exact frozen audio without rerunning selection/alignment."""
import concurrent.futures
import copy
import json
from pathlib import Path
import subprocess
import urllib.request
import zipfile

import real_mix_eval as e
from song_index_prepare import atomic_bytes, cut, pcm_wav

ROOT = Path('target/continuation')


def recover():
    ROOT.mkdir(parents=True, exist_ok=True)
    (ROOT/'preparation').mkdir(exist_ok=True)
    with zipfile.ZipFile('docs/evidence/E011/inputs-and-provenance.zip') as z:
        old = json.loads(z.read('manifest.json'))
        sources = json.loads(z.read('sources.json'))
    e.save(ROOT/'frozen-e011-manifest.json', old)
    e.save(ROOT/'frozen-e011-sources.json', sources)

    def source(key, record, url, canonical=False):
        media = ROOT/'media'/(key+'.mp3')
        output = ROOT/'media'/(key+'-full.wav')
        if not media.exists():
            request = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
            with urllib.request.urlopen(request, timeout=120) as response:
                atomic_bytes(media, response.read())
        assert e.sha(media) == record['source']['sha256'], ('source hash', key)
        if not output.exists():
            command = ['ffmpeg', '-hide_banner', '-loglevel', 'error', '-nostdin',
                       '-y', '-i', str(media), '-ac', '1', '-ar', '44100']
            if canonical:
                command += ['-f', 's16le', '-']
            else:
                command += ['-c:a', 'pcm_s16le', str(output)]
            result = subprocess.run(command, capture_output=True, check=True)
            if canonical:
                pcm_wav(output, result.stdout)
            e.save(ROOT/'preparation'/(key+'.json'), dict(command=command,
                   returncode=result.returncode, stderr=result.stderr.decode()))
        if 'original' in record:
            assert e.sha(output) == record['original']['sha256'], ('decoded hash', key)
        print('Recovered', key, flush=True)
        return key, output

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        futures = [pool.submit(source, k, r, r['track']['url'], k == 't05')
                   for k, r in sorted(sources.items())]
        futures += [pool.submit(source, q['id'], q, q['attribution']['url'], True)
                    for q in old['queries'] if q['group'] == 'negative']
        paths = dict(f.result() for f in concurrent.futures.as_completed(futures))
    manifest = copy.deepcopy(old)
    for r in manifest['references']['full']:
        r['path'] = str(paths[r['id']].resolve())
    for q in manifest['queries']:
        key = q['id'] if q['group'] == 'negative' else (
            'mix' if q['group'] in ('frozen', 'mix') else q['song'])
        output = ROOT/'queries'/(q['id']+'.wav')
        record = cut(paths[key], q['start_sample'], q['samples'], output)
        assert record['sha256'] == q['sha256'], ('query hash', q['id'])
        q['path'] = str(output.resolve())
    manifest['references'].pop('chunks')
    manifest['mix'] = dict(**e.identity(paths['mix']), **e.wav_info(paths['mix']))
    e.save(ROOT/'recovered.json', manifest)
    return manifest


if __name__ == '__main__':
    recover()
