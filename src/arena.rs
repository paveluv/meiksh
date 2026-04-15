/// A byte-interning arena backed by `Vec<Box<[u8]>>`.
///
/// Every `intern()` call converts a `Vec<u8>` to `Box<[u8]>` (exact-fit heap
/// allocation), pushes it into the arena, and returns a `&[u8]` whose lifetime
/// is tied to the arena.  Since entries are only ever appended and never
/// removed, all returned references remain valid for the arena's lifetime.
///
/// `UnsafeCell` is required to communicate interior mutability to the
/// compiler: `intern()` takes `&self` (so callers can hold multiple returned
/// `&[u8]` references simultaneously) but mutates the entry list.
pub struct ByteArena {
    entries: std::cell::UnsafeCell<Vec<Box<[u8]>>>,
}

impl Default for ByteArena {
    fn default() -> Self {
        Self::new()
    }
}

impl ByteArena {
    pub fn new() -> Self {
        Self {
            entries: std::cell::UnsafeCell::new(Vec::new()),
        }
    }

    /// Intern an owned `Vec<u8>`, returning a reference that lives as long as
    /// the arena.
    pub fn intern_vec(&self, s: Vec<u8>) -> &[u8] {
        let boxed: Box<[u8]> = s.into_boxed_slice();
        let ptr: *const [u8] = &*boxed;
        // SAFETY: The arena is single-threaded and only grows (no removal).
        // Each Box<[u8]> has a stable heap address regardless of Vec
        // reallocation, so previously returned &[u8] references remain valid.
        // UnsafeCell tells the compiler this memory may be mutated through
        // a shared reference, preventing misoptimization.
        unsafe {
            (*self.entries.get()).push(boxed);
            &*ptr
        }
    }

    /// Convenience: intern a borrowed `&[u8]` (copies into the arena).
    pub fn intern_bytes(&self, s: &[u8]) -> &[u8] {
        self.intern_vec(s.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_content() {
        let arena = ByteArena::default();
        let s = arena.intern_vec(b"hello".to_vec());
        assert_eq!(s, b"hello");
    }

    #[test]
    fn intern_bytes_returns_same_content() {
        let arena = ByteArena::new();
        let s = arena.intern_bytes(b"world");
        assert_eq!(s, b"world");
    }

    #[test]
    fn multiple_interns_coexist() {
        let arena = ByteArena::new();
        let a = arena.intern_vec(b"alpha".to_vec());
        let b = arena.intern_vec(b"beta".to_vec());
        let c = arena.intern_bytes(b"gamma");
        assert_eq!(a, b"alpha");
        assert_eq!(b, b"beta");
        assert_eq!(c, b"gamma");
    }

    #[test]
    fn empty_bytes_supported() {
        let arena = ByteArena::new();
        let s = arena.intern_vec(Vec::new());
        assert_eq!(s, b"");
    }

    #[test]
    fn survives_vec_reallocation() {
        let arena = ByteArena::new();
        let mut refs = Vec::new();
        for i in 0..1000u32 {
            let mut entry = b"entry_".to_vec();
            let mut n = i;
            if n == 0 {
                entry.push(b'0');
            } else {
                let start = entry.len();
                while n > 0 {
                    entry.push(b'0' + (n % 10) as u8);
                    n /= 10;
                }
                entry[start..].reverse();
            }
            refs.push(arena.intern_vec(entry));
        }
        for (i, r) in refs.iter().enumerate() {
            let mut expected = b"entry_".to_vec();
            let mut n = i as u32;
            if n == 0 {
                expected.push(b'0');
            } else {
                let start = expected.len();
                while n > 0 {
                    expected.push(b'0' + (n % 10) as u8);
                    n /= 10;
                }
                expected[start..].reverse();
            }
            assert_eq!(*r, expected.as_slice());
        }
    }
}
