#!/usr/bin/env python3
"""Independent, provisional STFT cue annotations; never invokes cqt-monitor."""
import json
from pathlib import Path
import subprocess
import numpy as np
from scipy import signal, fft
import real_mix_eval as e

ROOT = Path('target/chunk-query')
PARAMETERS = dict(rate=6000, fft_samples=2048, hop=600, seconds_per_feature=1,
                  midi_low=36, midi_high=96, template_seconds=60, source_stride=30,
                  tempos=[round(float(x), 3) for x in np.arange(.88, 1.121, .02)],
                  pitch_semitones=[-2, -1, 0, 1, 2], minimum_similarity=.45,
                  correlation='window-centered normalized temporal correlation')


def features(path):
    data = subprocess.run(['ffmpeg', '-v', 'error', '-nostdin', '-i', str(path),
                           '-ac', '1', '-ar', '6000', '-f', 'f32le', '-'],
                          capture_output=True, check=True).stdout
    audio = np.frombuffer(data, dtype='<f4')
    frequencies, _, z = signal.stft(audio, fs=6000, nperseg=2048, noverlap=1448,
                                    boundary=None, padded=False)
    power = np.abs(z) ** 2
    bands = []
    for note in range(36, 96):
        lower, upper = 440 * 2**((note-.5-69)/12), 440 * 2**((note+.5-69)/12)
        bins = (frequencies >= lower) & (frequencies < upper)
        bands.append(power[bins].mean(axis=0))
    x = np.log(np.maximum(np.array(bands), 1e-12))
    n = x.shape[1] // 10
    x = x[:, :n*10].reshape(60, n, 10).mean(axis=2)
    # Remove stationary per-band coloration; use no CQT, landmark or native score.
    x -= np.median(x, axis=1, keepdims=True)
    x /= np.maximum(np.std(x, axis=1, keepdims=True), .5)
    x = np.clip(x, -3, 3)
    x /= np.maximum(np.linalg.norm(x, axis=0, keepdims=True), 1e-8)
    return x.astype(np.float32)


def search(mix, source, seconds=60, stride=30):
    size = fft.next_fast_len(mix.shape[1] + 2*seconds)
    # Drop edge bands consistently under pitch shifts.
    mix_fft = {shift: fft.rfft(mix[2+shift:58+shift], n=size, axis=1)
               for shift in PARAMETERS['pitch_semitones']}
    energies = {}
    for tempo in PARAMETERS['tempos']:
        n = len(np.arange(0, seconds-1+.0001, tempo))
        for shift in PARAMETERS['pitch_semitones']:
            x = mix[2+shift:58+shift].astype(np.float64)
            sums = np.pad(np.cumsum(x, axis=1), ((0,0),(1,0)))
            sums2 = np.pad(np.cumsum(x*x, axis=1), ((0,0),(1,0)))
            energy = ((sums2[:,n:]-sums2[:,:-n]) - (sums[:,n:]-sums[:,:-n])**2/n).sum(axis=0)
            energies[(n,shift)] = np.maximum(energy, 1e-12)
    found = []
    for start in range(0, source.shape[1]-seconds+1, stride):
        best = None
        for tempo in PARAMETERS['tempos']:
            positions = np.arange(0, seconds-1+.0001, tempo)
            template = np.stack([np.interp(positions, np.arange(seconds), row[start:start+seconds])
                                 for row in source[2:58]])
            n = template.shape[1]
            template -= template.mean(axis=1, keepdims=True)
            energy = max(float(np.sum(template*template)), 1e-12)
            template_fft = fft.rfft(template[:, ::-1], n=size, axis=1)
            for shift in PARAMETERS['pitch_semitones']:
                scores = fft.irfft((mix_fft[shift] * template_fft).sum(axis=0), n=size)
                valid = scores[n-1:mix.shape[1]] / np.sqrt(energy*energies[(n,shift)])
                at = int(np.argmax(valid))
                candidate = dict(source_start=start, mix_start=at, similarity=float(valid[at]),
                                 tempo=tempo, pitch_semitones=shift, mix_end=at+n)
                if best is None or candidate['similarity'] > best['similarity']:
                    best = candidate
        found.append(best)
    return found


def main():
    sources = json.loads((ROOT/'sources.json').read_text())
    output = ROOT/'alignment-centered'
    output.mkdir(exist_ok=True)
    metadata = dict(status='running', parameters=PARAMETERS, evaluator=e.identity(__file__),
                    sources=e.identity(ROOT/'sources.json'), independent_of_native_matcher=True,
                    annotation_status='provisional algorithmic; not human-verified')
    e.save(output/'metadata.json', metadata)
    def cached(song):
        path = output/f'{song}.npy'
        if not path.exists():
            original = sources[song]['original']
            assert e.sha(original['path']) == original['sha256'], song
            previous = ROOT/'alignment'/(song+'.npy')
            if previous.exists():
                np.save(path, np.load(previous))
            else:
                np.save(path, features(original['path']))
        return np.load(path)
    mix = cached('mix')
    records = {}
    for song in sorted(k for k in sources if k != 'mix'):
        path = output/f'{song}.json'
        if path.exists():
            found = json.loads(path.read_text())
        else:
            found = search(mix, cached(song))
            e.save(path, found)
        records[song] = found
        supported = [r for r in found if r['similarity'] >= PARAMETERS['minimum_similarity']]
        print(song, 'supported', len(supported), '/', len(found),
              'best', sorted(found, key=lambda r: -r['similarity'])[:3], flush=True)
        e.save(output/'anchors.json', records)
    metadata.update(status='completed', anchors=e.identity(output/'anchors.json'))
    e.save(output/'metadata.json', metadata)


if __name__ == '__main__':
    main()
