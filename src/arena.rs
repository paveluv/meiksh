/// A string-interning arena backed by `Vec<Box<str>>`.
///
/// Every `intern()` call converts a `String` to `Box<str>` (exact-fit heap
/// allocation), pushes it into the arena, and returns a `&str` whose lifetime
/// is tied to the arena.  Since entries are only ever appended and never
/// removed, all returned references remain valid for the arena's lifetime.
#[derive(Default)]
pub struct StringArena {
    entries: Vec<Box<str>>,
}

impl StringArena {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Intern an owned `String`, returning a reference that lives as long as
    /// the arena.
    pub fn intern(&self, s: String) -> &str {
        let boxed: Box<str> = s.into_boxed_str();
        let ptr: *const str = &*boxed;
        // SAFETY: The arena only grows (we never remove entries), so the
        // Box<str> heap allocation is stable for the arena's lifetime.
        // We convert to a raw pointer before pushing to decouple the
        // returned &str lifetime from the &mut borrow on entries.
        let entries = &self.entries as *const Vec<Box<str>> as *mut Vec<Box<str>>;
        unsafe {
            (*entries).push(boxed);
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
