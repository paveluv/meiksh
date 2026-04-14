/// A string-interning arena backed by `Vec<Box<str>>`.
///
/// Every `intern()` call converts a `String` to `Box<str>` (exact-fit heap
/// allocation), pushes it into the arena, and returns a `&str` whose lifetime
/// is tied to the arena.  Since entries are only ever appended and never
/// removed, all returned references remain valid for the arena's lifetime.
///
/// `UnsafeCell` is required to communicate interior mutability to the
/// compiler: `intern()` takes `&self` (so callers can hold multiple returned
/// `&str` references simultaneously) but mutates the entry list.
pub struct StringArena {
    entries: std::cell::UnsafeCell<Vec<Box<str>>>,
}

impl Default for StringArena {
    fn default() -> Self {
        Self::new()
    }
}

impl StringArena {
    pub fn new() -> Self {
        Self {
            entries: std::cell::UnsafeCell::new(Vec::new()),
        }
    }

    /// Intern an owned `String`, returning a reference that lives as long as
    /// the arena.
    pub fn intern(&self, s: String) -> &str {
        let boxed: Box<str> = s.into_boxed_str();
        let ptr: *const str = &*boxed;
        // SAFETY: The arena is single-threaded and only grows (no removal).
        // Each Box<str> has a stable heap address regardless of Vec
        // reallocation, so previously returned &str references remain valid.
        // UnsafeCell tells the compiler this memory may be mutated through
        // a shared reference, preventing misoptimization.
        unsafe {
            (*self.entries.get()).push(boxed);
            &*ptr
        }
    }

    /// Convenience: intern a borrowed `&str` (copies into the arena).
    pub fn intern_str(&self, s: &str) -> &str {
        self.intern(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_content() {
        let arena = StringArena::new();
        let s = arena.intern("hello".to_string());
        assert_eq!(s, "hello");
    }

    #[test]
    fn intern_str_returns_same_content() {
        let arena = StringArena::new();
        let s = arena.intern_str("world");
        assert_eq!(s, "world");
    }

    #[test]
    fn multiple_interns_coexist() {
        let arena = StringArena::new();
        let a = arena.intern("alpha".to_string());
        let b = arena.intern("beta".to_string());
        let c = arena.intern_str("gamma");
        assert_eq!(a, "alpha");
        assert_eq!(b, "beta");
        assert_eq!(c, "gamma");
    }

    #[test]
    fn empty_string_is_supported() {
        let arena = StringArena::new();
        let s = arena.intern(String::new());
        assert_eq!(s, "");
    }

    #[test]
    fn survives_vec_reallocation() {
        let arena = StringArena::new();
        let mut refs = Vec::new();
        for i in 0..1000 {
            refs.push(arena.intern(format!("entry_{i}")));
        }
        for (i, r) in refs.iter().enumerate() {
            assert_eq!(*r, format!("entry_{i}"));
        }
    }
}
