//! Tolerant lookups against an index the size of a few watched songs.

use std::hint::black_box;

use cqt_monitor::{Hash, Index, IndexBuilder, encode_key};
use criterion::{Criterion, criterion_group, criterion_main};

fn pseudo_hashes(seed: u64, count: usize) -> Vec<Hash> {
    let mut seed = seed;
    (0..count)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            // Fields distributed like real triplets: small bin differences
            // and ratio steps 0..32.
            let d12 = (seed % 121) as i32 - 60;
            let d23 = ((seed >> 8) % 121) as i32 - 60;
            let ratio = ((seed >> 16) % 33) as i32;
            Hash {
                key: encode_key(d12, d23, ratio),
                frame: i as u64 / 8,
                bin: 20 + ((seed >> 24) % 120) as u32,
                span: 10 + ((seed >> 32) % 300) as u32,
            }
        })
        .collect()
}

fn index(songs: usize) -> Index {
    let mut builder = IndexBuilder::new();
    for song in 0..songs {
        builder.add_song(
            &format!("s{song}"),
            10_000,
            pseudo_hashes(song as u64 + 1, 120_000),
        );
    }
    builder.build(8)
}

fn lookups(c: &mut Criterion) {
    let index = index(4);
    // Queries drawn from the same distribution: about every probe hits a
    // bucket. Then queries with an unused ratio step: every probe misses.
    let hits = pseudo_hashes(77, 4_096);
    let misses: Vec<Hash> = hits
        .iter()
        .map(|h| Hash {
            key: h.key + 200,
            ..*h
        })
        .collect();
    for (name, queries) in [("hit", &hits), ("miss", &misses)] {
        c.bench_function(&format!("lookup_{name}_x4096"), |b| {
            b.iter(|| {
                let mut n = 0u64;
                for h in queries {
                    index.lookup(black_box(h.key), 1, 1, |e| n += u64::from(e.span));
                }
                black_box(n)
            })
        });
    }
}

criterion_group!(benches, lookups);
criterion_main!(benches);
