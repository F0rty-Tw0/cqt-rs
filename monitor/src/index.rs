//! Hash index over the watched songs.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use crate::hashes::{Hash, HashKey};

/// Where a hash occurs in a watched song.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Index of the song in the watch list.
    pub song: u16,
    /// Anchor bin.
    pub bin: u16,
    /// Span of the triplet in frames.
    pub span: u16,
    /// Anchor frame.
    pub frame: u32,
}

/// Multiplicative hasher for the packed keys: the map is keyed by a `u32`
/// only, so a single multiply is faster than SipHash.
#[derive(Default)]
pub struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u32(&mut self, key: u32) {
        self.0 = (u64::from(key).wrapping_mul(0x9E37_79B9_7F4A_7C15)) >> 20;
    }
}

/// Accumulates the hashes of the watched songs.
#[derive(Debug, Default)]
pub struct IndexBuilder {
    names: Vec<String>,
    frames: Vec<u32>,
    hashes: Vec<(HashKey, Entry)>,
}

impl IndexBuilder {
    /// Starts an empty watch list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a song; `frames` is its length in frames.
    pub fn add_song(
        &mut self,
        name: &str,
        frames: u32,
        hashes: impl IntoIterator<Item = Hash>,
    ) -> u16 {
        let song = u16::try_from(self.names.len()).expect("at most 65535 songs");
        self.names.push(name.to_owned());
        self.frames.push(frames);
        for h in hashes {
            self.hashes.push((
                h.key,
                Entry {
                    song,
                    bin: h.bin as u16,
                    span: h.span.min(u32::from(u16::MAX)) as u16,
                    frame: h.frame.min(u64::from(u32::MAX)) as u32,
                },
            ));
        }
        song
    }

    /// Builds the index. Keys shared by more than `max_bucket_per_song`
    /// entries per watched song carry too little information and are
    /// skipped at lookup time.
    pub fn build(mut self, max_bucket_per_song: usize) -> Index {
        self.hashes
            .sort_unstable_by_key(|(key, e)| (*key, e.song, e.frame, e.bin));
        let mut table = HashMap::with_capacity_and_hasher(
            self.hashes.len() / 2 + 1,
            BuildHasherDefault::default(),
        );
        let entries: Vec<Entry> = self.hashes.iter().map(|(_, e)| *e).collect();
        let mut start = 0;
        while start < self.hashes.len() {
            let key = self.hashes[start].0;
            let mut end = start;
            while end < self.hashes.len() && self.hashes[end].0 == key {
                end += 1;
            }
            table.insert(key, (start as u32, (end - start) as u32));
            start = end;
        }
        let max_bucket = (max_bucket_per_song.max(1) * self.names.len().max(1)) as u32;
        Index {
            names: self.names,
            frames: self.frames,
            table,
            entries,
            max_bucket,
        }
    }
}

/// Hash table from keys to the places they occur in the watched songs.
#[derive(Debug)]
pub struct Index {
    names: Vec<String>,
    frames: Vec<u32>,
    table: HashMap<HashKey, (u32, u32), BuildHasherDefault<KeyHasher>>,
    entries: Vec<Entry>,
    max_bucket: u32,
}

impl Index {
    /// Names of the watched songs, in song-index order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Length of a watched song in frames.
    pub fn song_frames(&self, song: u16) -> u32 {
        self.frames[usize::from(song)]
    }

    /// Number of stored hashes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Approximate memory use in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<Entry>() + self.table.capacity() * 16
    }

    /// Calls `on_entry` for every stored entry whose key is within
    /// `±bin_tolerance` on both bin differences and `±ratio_tolerance` on
    /// the ratio step of `key`, skipping over-full buckets.
    pub fn lookup<F: FnMut(&Entry)>(
        &self,
        key: HashKey,
        bin_tolerance: i32,
        ratio_tolerance: i32,
        mut on_entry: F,
    ) {
        for e1 in -bin_tolerance..=bin_tolerance {
            for e2 in -bin_tolerance..=bin_tolerance {
                for dr in -ratio_tolerance..=ratio_tolerance {
                    let probe = key
                        .wrapping_add((e1 << 20) as u32)
                        .wrapping_add((e2 << 10) as u32)
                        .wrapping_add(dr as u32);
                    if let Some(&(start, len)) = self.table.get(&probe) {
                        if len > self.max_bucket {
                            continue;
                        }
                        for entry in &self.entries[start as usize..(start + len) as usize] {
                            on_entry(entry);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashes::encode_key;

    fn hash(d12: i32, d23: i32, r: i32, frame: u64, bin: u32, span: u32) -> Hash {
        Hash {
            key: encode_key(d12, d23, r),
            frame,
            bin,
            span,
        }
    }

    #[test]
    fn lookup_probes_neighbouring_keys_and_skips_full_buckets() {
        let mut builder = IndexBuilder::new();
        builder.add_song(
            "a",
            1000,
            vec![hash(3, -2, 10, 5, 40, 100), hash(4, -2, 11, 9, 41, 120)],
        );
        builder.add_song("b", 500, (0..20).map(|i| hash(0, 0, 0, i, 1, 10)));
        let index = builder.build(8);
        assert_eq!(index.names(), &["a".to_string(), "b".to_string()]);
        assert_eq!(index.len(), 22);
        let mut found = Vec::new();
        index.lookup(encode_key(3, -2, 10), 1, 1, |e| found.push(*e));
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|e| e.song == 0));
        found.clear();
        index.lookup(encode_key(3, -2, 10), 0, 0, |e| found.push(*e));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].frame, 5);
        // The bucket of 20 entries exceeds 8 per song × 2 songs.
        found.clear();
        index.lookup(encode_key(0, 0, 0), 1, 1, |e| found.push(*e));
        assert!(found.is_empty());
    }
}
