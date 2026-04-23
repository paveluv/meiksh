//! `$include` path resolution and recursion guard (spec § 6.2).

#![allow(dead_code)]

/// Guard against transitive `$include` self-recursion. Each path is
/// canonicalised once and then compared as a byte slice.
#[derive(Default, Debug)]
pub(crate) struct IncludeGuard {
    stack: Vec<Vec<u8>>,
}

impl IncludeGuard {
    /// Attempt to enter `path`. Returns `true` if this is a fresh
    /// include; `false` if `path` is already on the stack (i.e. we'd
    /// be recursing). Callers should [`IncludeGuard::leave`] when the
    /// file has been parsed.
    pub(crate) fn enter(&mut self, path: &[u8]) -> bool {
        if self.stack.iter().any(|p| p == path) {
            return false;
        }
        self.stack.push(path.to_vec());
        true
    }

    pub(crate) fn leave(&mut self, path: &[u8]) {
        if let Some(pos) = self.stack.iter().rposition(|p| p == path) {
            self.stack.remove(pos);
        }
    }
}

/// Resolve `target` relative to `including_file`. Absolute paths are
/// returned verbatim.
pub(crate) fn resolve(including_file: &[u8], target: &[u8]) -> Vec<u8> {
    if target.starts_with(b"/") {
        return target.to_vec();
    }
    let dir = match including_file.iter().rposition(|&b| b == b'/') {
        Some(pos) => &including_file[..=pos],
        None => b"./",
    };
    let mut out = dir.to_vec();
    out.extend_from_slice(target);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn absolute_path_kept_verbatim() {
        assert_no_syscalls(|| {
            assert_eq!(
                resolve(b"/home/u/.inputrc", b"/etc/inputrc"),
                b"/etc/inputrc"
            );
        });
    }

    #[test]
    fn relative_path_joined_to_dir() {
        assert_no_syscalls(|| {
            assert_eq!(resolve(b"/home/u/.inputrc", b"shared"), b"/home/u/shared");
        });
    }

    #[test]
    fn include_guard_blocks_recursion() {
        assert_no_syscalls(|| {
            let mut g = IncludeGuard::default();
            assert!(g.enter(b"/a"));
            assert!(!g.enter(b"/a"));
            assert!(g.enter(b"/b"));
            g.leave(b"/b");
            g.leave(b"/a");
            assert!(g.enter(b"/a"));
        });
    }
}
