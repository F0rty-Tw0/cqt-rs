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

/// Multiplicative hasher for the packed hash keys and the matcher's cell
/// keys: the maps are keyed by one integer, so a multiply and a fold are
/// much faster than SipHash.
///
/// The table uses the low bits of the hash for the bucket and the top
/// seven bits as a tag that filters probes before any key comparison, so
/// both ends must be well mixed. The high half of the product is; folding
/// it into the low half mixes that too (the low bits of the product alone
/// depend only on the low, structured bits of the key).
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
        self.write_u64(u64::from(key));
    }
    fn write_u64(&mut self, key: u64) {
        let x = key.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 = x ^ (x >> 32);
    }
}

/// A `HashMap` keyed by one integer, hashed with [`KeyHasher`].
pub type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<KeyHasher>>;

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

    /// Builds the index. A key that occurs more than `max_bucket_per_song`
    /// times in one song says little about where in that song the stream
    /// is, so those occurrences are dropped; the same key's occurrences in
    /// other songs are kept. The cap is therefore independent of how many
    /// songs are watched.
    pub fn build(mut self, max_bucket_per_song: usize) -> Index {
        let cap = max_bucket_per_song.max(1);
        self.hashes
            .sort_unstable_by_key(|(key, e)| (*key, e.song, e.frame, e.bin));
        let mut table = HashMap::with_capacity_and_hasher(
            self.hashes.len() / 2 + 1,
            BuildHasherDefault::default(),
        );
        let mut entries: Vec<Entry> = Vec::with_capacity(self.hashes.len());
        let mut dropped = 0;
        let mut start = 0;
        while start < self.hashes.len() {
            let key = self.hashes[start].0;
            let mut end = start;
            while end < self.hashes.len() && self.hashes[end].0 == key {
                end += 1;
            }
            let first = entries.len();
            let mut song_start = start;
            while song_start < end {
                let song = self.hashes[song_start].1.song;
                let mut song_end = song_start;
                while song_end < end && self.hashes[song_end].1.song == song {
                    song_end += 1;
                }
                if song_end - song_start <= cap {
                    entries.extend(self.hashes[song_start..song_end].iter().map(|(_, e)| *e));
                } else {
                    dropped += song_end - song_start;
                }
                song_start = song_end;
            }
            if entries.len() > first {
                table.insert(key, (first as u32, (entries.len() - first) as u32));
            }
            start = end;
        }
        entries.shrink_to_fit();
        Index {
            names: self.names,
            frames: self.frames,
            table,
            entries,
            dropped,
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
    dropped: usize,
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

    /// Number of hashes dropped at build time because their key occurred
    /// more than the cap times in one song.
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Approximate memory use in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.entries.len() * std::mem::size_of::<Entry>() + self.table.capacity() * 16
    }

    /// Calls `on_entry` for every stored entry whose key is within
    /// `±bin_tolerance` on both bin differences and `±ratio_tolerance` on
    /// the ratio step of `key`.
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
    fn lookup_probes_neighbouring_keys_and_drops_repetitive_keys_per_song() {
        let mut builder = IndexBuilder::new();
        builder.add_song(
            "a",
            1000,
            vec![hash(3, -2, 10, 5, 40, 100), hash(4, -2, 11, 9, 41, 120)],
        );
        builder.add_song("b", 500, (0..20).map(|i| hash(0, 0, 0, i, 1, 10)));
        builder.add_song("c", 500, (0..3).map(|i| hash(0, 0, 0, 7 * i, 2, 11)));
        let index = builder.build(8);
        assert_eq!(index.names(), &["a", "b", "c"]);
        // Song b's 20 occurrences of one key are dropped; song c's three
        // occurrences of the same key stay.
        assert_eq!(index.len(), 5);
        assert_eq!(index.dropped(), 20);
        let mut found = Vec::new();
        index.lookup(encode_key(3, -2, 10), 1, 1, |e| found.push(*e));
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|e| e.song == 0));
        found.clear();
        index.lookup(encode_key(3, -2, 10), 0, 0, |e| found.push(*e));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].frame, 5);
        found.clear();
        index.lookup(encode_key(0, 0, 0), 1, 1, |e| found.push(*e));
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|e| e.song == 2));
    }

    #[test]
    fn cap_does_not_depend_on_the_number_of_songs() {
        // Nine occurrences in one song are dropped whether or not other
        // songs are watched.
        for others in [0, 1, 50] {
            let mut builder = IndexBuilder::new();
            builder.add_song("a", 100, (0..9).map(|i| hash(1, 1, 1, i, 1, 10)));
            for o in 0..others {
                builder.add_song(&format!("o{o}"), 100, vec![hash(2, 2, 2, 1, 1, 10)]);
            }
            let index = builder.build(8);
            let mut found = 0;
            index.lookup(encode_key(1, 1, 1), 0, 0, |_| found += 1);
            assert_eq!(found, 0, "{others} other songs");
            assert_eq!(index.dropped(), 9);
        }
    }

    #[test]
    fn hasher_spreads_the_tag_and_bucket_bits() {
        // hashbrown takes its 7-bit control tag from the top of the hash
        // and the bucket from the bottom; both must vary over the packed
        // keys, which only differ in three narrow fields.
        let mut tags = std::collections::HashSet::new();
        let mut low = std::collections::HashSet::new();
        for d12 in -60..60 {
            for d23 in -60..60 {
                for ratio in 0..32 {
                    let mut h = KeyHasher::default();
                    h.write_u32(encode_key(d12, d23, ratio));
                    let hash = h.finish();
                    tags.insert(hash >> 57);
                    low.insert(hash & 0xffff);
                }
            }
        }
        assert_eq!(tags.len(), 128);
        assert!(low.len() > 60_000, "{}", low.len());
    }
}
