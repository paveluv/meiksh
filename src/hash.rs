//! Shell-internal hasher and HashMap/HashSet aliases tuned for short
//! byte-string keys (variable names, alias names, function names, path
//! components, etc.).
//!
//! Why not `std::collections::HashMap`'s default `SipHash-1-3`?
//! `SipHash` is cryptographic strength: it is DoS-resistant, which matters
//! for code that hashes attacker-controlled keys (web request handlers,
//! deserialisers, etc.) but not for a shell's own internal state. Perf
//! measurements on the `arithmetic` benchmark under `LC_ALL=C.UTF-8`
//! showed `SipHash` consuming ~20% of arithmetic time just for
//! `env` / `path_cache` / `alias` lookups. `ShellHasher` runs in ~20
//! cycles on a short key versus ~80-120 for SipHash-1-3.
//!
//! Design
//! ------
//! - Mixer: `state = state.rotate_left(5) ^ word.wrapping_mul(MUL)`. A
//!   rotate + xor + multiply is fully pipelined and one cycle deep on
//!   modern x86-64 / aarch64; the same shape is used by FxHash with an
//!   additional arithmetic shift.
//! - 8-byte main loop for the unaligned body.
//! - Length-match `read_short_tail` for 1..=7 trailing bytes, assembling a
//!   single `u64` via aligned sub-word loads. This avoids the per-byte
//!   shift/or chain that otherwise serialises on the memory pipe.
//! - `finish()` applies a full avalanche (xor-shift + multiply, xor-shift)
//!   so that hashbrown's 7-bit tag extraction (which reads the high bits)
//!   is not dominated by the top bytes of the key.
//!
//! Fixed seed
//! ----------
//! No randomization: the shell does not face adversarial inputs and
//! randomization would make `perf` output non-deterministic between runs.
//! The seed is just a non-zero constant so that the empty key does not
//! collide with the `0` integer key in `finish()`.

use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Multiplier from FxHash -- a high-entropy odd 64-bit constant.
const MUL: u64 = 0x517c_c1b7_2722_0a95;

/// Fixed seed.
const SEED: u64 = 0x2432_f6a8_8848_5a30;

/// Fast non-cryptographic hasher for shell-internal byte-string keys.
///
/// Use via [`ShellMap`] / [`ShellSet`] rather than constructing directly.
#[derive(Clone)]
pub(crate) struct ShellHasher {
    state: u64,
    /// Total number of bytes fed to `write()`. Folded into `finish()` so
    /// that keys like `PATH`, `PATH\0`, `PATH\0\0` do not collide -- the
    /// tail mixer alone cannot distinguish trailing zeros.
    length: u64,
}

impl ShellHasher {
    #[inline]
    fn mix(&mut self, word: u64) {
        self.state = self.state.rotate_left(5) ^ word.wrapping_mul(MUL);
    }
}

impl Default for ShellHasher {
    #[inline]
    fn default() -> Self {
        Self {
            state: SEED,
            length: 0,
        }
    }
}

impl Hasher for ShellHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        while bytes.len() >= 8 {
            let (head, rest) = bytes.split_at(8);
            let word = u64::from_le_bytes(head.try_into().unwrap());
            self.mix(word);
            bytes = rest;
        }
        if !bytes.is_empty() {
            let tail = read_short_tail(bytes);
            self.mix(tail);
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        // splitmix64 finalizer with the length folded in -- gives a full
        // 64-bit avalanche so hashbrown's top-7-bit tag is well mixed.
        let mut h = self.state ^ self.length;
        h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        h
    }

    // The single-integer overrides bypass `write()` (no buffer, no
    // length bookkeeping) and feed the value straight into `mix`. These
    // are the paths hashbrown uses for its internal integer keys (table
    // capacity, rehash salt, etc.); we skip `write_u8/i8` etc. because
    // they are unused by our callers and would only clutter coverage.
    #[inline]
    fn write_u32(&mut self, n: u32) {
        self.mix(n as u64);
    }
    #[inline]
    fn write_u64(&mut self, n: u64) {
        self.mix(n);
    }
    #[inline]
    fn write_usize(&mut self, n: usize) {
        self.mix(n as u64);
    }
}

/// Assemble up to 7 trailing bytes into a single `u64` using the
/// overlapping-read technique from xxhash / FxHash. For `len >= 4` we
/// read the first four and last four bytes (overlapping when `len < 8`)
/// and OR them at a length-dependent shift. For `len < 4` we read the
/// first, middle, and last byte. This has no unreachable arms, so
/// every instrumented branch is exercised by the length-range tests.
///
/// The tail is not required to be length-injective on its own: the
/// overall byte count is folded into `finish()` so e.g. `b"PATH"` and
/// `b"PATH\0"` hash distinctly even if their tails collide.
#[inline]
fn read_short_tail(bytes: &[u8]) -> u64 {
    debug_assert!(!bytes.is_empty() && bytes.len() < 8);
    let len = bytes.len();
    if len >= 4 {
        let lo = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as u64;
        let hi = u32::from_le_bytes(bytes[len - 4..].try_into().unwrap()) as u64;
        lo | (hi << ((len - 4) * 8))
    } else {
        let b0 = bytes[0] as u64;
        let bmid = bytes[len / 2] as u64;
        let blast = bytes[len - 1] as u64;
        b0 | (bmid << 8) | (blast << 16)
    }
}

/// `BuildHasher` for `ShellHasher`. Zero-sized; every new hasher starts
/// from `Default` (the fixed seed).
pub(crate) type ShellBuildHasher = BuildHasherDefault<ShellHasher>;

/// HashMap alias using [`ShellHasher`]. Prefer this over
/// `std::collections::HashMap` for any map whose keys are shell-internal
/// byte strings (names, paths) -- see `docs/IMPLEMENTATION_POLICY.md`.
pub(crate) type ShellMap<K, V> = HashMap<K, V, ShellBuildHasher>;

/// HashSet alias using [`ShellHasher`]. Same guidance as [`ShellMap`].
#[allow(dead_code)]
pub(crate) type ShellSet<K> = HashSet<K, ShellBuildHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    use std::hash::BuildHasher;

    fn hash_bytes(bytes: &[u8]) -> u64 {
        let builder = ShellBuildHasher::default();
        let mut h = builder.build_hasher();
        h.write(bytes);
        h.finish()
    }

    #[test]
    fn deterministic_across_calls() {
        assert_eq!(hash_bytes(b""), hash_bytes(b""));
        assert_eq!(hash_bytes(b"PATH"), hash_bytes(b"PATH"));
        assert_eq!(hash_bytes(b"/usr/local/bin"), hash_bytes(b"/usr/local/bin"));
    }

    #[test]
    fn distinct_keys_give_distinct_hashes() {
        let keys: [&[u8]; 10] = [
            b"",
            b"a",
            b"A",
            b"PATH",
            b"PWD",
            b"HOME",
            b"USER",
            b"LANG",
            b"PATH\0",
            b"PATH\0\0",
        ];
        let mut seen: Vec<u64> = Vec::with_capacity(keys.len());
        for k in keys {
            let h = hash_bytes(k);
            assert!(!seen.contains(&h), "collision on {:?}", k);
            seen.push(h);
        }
    }

    #[test]
    fn read_short_tail_lengths_distinct() {
        // Short branch (len < 4) uses b0 | bmid<<8 | blast<<16, which
        // duplicates the single byte when len == 1.
        assert_eq!(read_short_tail(b"a"), 0x61_61_61);
        assert_eq!(read_short_tail(b"ab"), 0x62_62_61);
        assert_eq!(read_short_tail(b"abc"), 0x63_62_61);
        // Wide branch (len >= 4): lo | (hi << ((len - 4) * 8)).
        assert_eq!(read_short_tail(b"abcd"), 0x6463_6261);
        assert_eq!(read_short_tail(b"abcde"), 0x65_6463_6261);
        assert_eq!(read_short_tail(b"abcdef"), 0x6665_6463_6261);
        assert_eq!(read_short_tail(b"abcdefg"), 0x67_6665_6463_6261);

        // Tail values are not guaranteed length-injective on their own
        // (length folding in `finish()` handles that); but within each
        // branch, different bytes at the same length must give different
        // tails.
        assert_ne!(read_short_tail(b"a"), read_short_tail(b"b"));
        assert_ne!(read_short_tail(b"ab"), read_short_tail(b"ba"));
    }

    #[test]
    fn bit_avalanche_single_flip_changes_many_bits() {
        // Flipping a single bit in the key should change at least a third
        // of the output bits. This is a sanity check against a flat mixer
        // that leaks key bits directly into the hash.
        let base: [u8; 16] = *b"envvar-name-12AB";
        let base_hash = hash_bytes(&base);
        for bit in 0..(base.len() * 8) {
            let mut mutated = base;
            mutated[bit / 8] ^= 1 << (bit % 8);
            let h = hash_bytes(&mutated);
            let delta = (base_hash ^ h).count_ones();
            assert!(
                delta >= 24,
                "weak avalanche: bit {} flipped only {} output bits",
                bit,
                delta
            );
        }
    }

    #[test]
    fn bucket_distribution_on_env_like_keys() {
        // Hash 256 shell-variable-sounding names and verify the bottom 6
        // bits (i.e. a 64-bucket table) stay reasonably balanced. Max
        // occupancy per bucket must stay below 10 -- comfortably under the
        // ~256/64 = 4 expected average plus reasonable variance.
        let names: Vec<Vec<u8>> = (0..256)
            .map(|i| format!("VAR_{:03x}_NAME", i).into_bytes())
            .collect();
        let mut buckets = [0u32; 64];
        for n in &names {
            let h = hash_bytes(n);
            buckets[(h & 0x3f) as usize] += 1;
        }
        let max = *buckets.iter().max().unwrap();
        let min = *buckets.iter().min().unwrap();
        assert!(
            max <= 10,
            "bucket skew: max {} min {} buckets {:?}",
            max,
            min,
            buckets
        );
        assert!(min >= 1, "empty bucket for 256 keys over 64 slots");
    }

    #[test]
    fn integer_writes_reach_mixer() {
        let builder = ShellBuildHasher::default();
        let mut a = builder.build_hasher();
        a.write_u32(42);
        let mut b = builder.build_hasher();
        b.write_u32(43);
        assert_ne!(a.finish(), b.finish());

        let mut c = builder.build_hasher();
        c.write_u64(42);
        // u32(42) and u64(42) both go through mix(42 as u64) so they must
        // produce the same hash. This is an intentional property: the
        // hasher sees a single word, not an integer type.
        let mut d = builder.build_hasher();
        d.write_u32(42);
        assert_eq!(c.finish(), d.finish());
    }

    #[test]
    fn shellmap_roundtrip() {
        let mut map: ShellMap<Vec<u8>, u32> = ShellMap::default();
        map.insert(b"PATH".to_vec(), 1);
        map.insert(b"HOME".to_vec(), 2);
        assert_eq!(map.get(b"PATH".as_ref()), Some(&1));
        assert_eq!(map.get(b"HOME".as_ref()), Some(&2));
        assert_eq!(map.get(b"MISSING".as_ref()), None);
    }

    #[test]
    fn shellset_roundtrip() {
        let mut set: ShellSet<Vec<u8>> = ShellSet::default();
        set.insert(b"PATH".to_vec());
        assert!(set.contains(b"PATH".as_ref()));
        assert!(!set.contains(b"MISSING".as_ref()));
    }

    #[test]
    fn hasher_clone_preserves_state() {
        let mut h1 = ShellHasher::default();
        h1.write(b"PATH");
        let h2 = h1.clone();
        assert_eq!(h1.finish(), h2.finish());
    }
}
