//! Substring / prefix scans over the shell history, shared by vi
//! `/`+`?` search and emacs `C-r` / `C-s` incremental search.
//!
//! These functions intentionally operate on `&[Box<[u8]>]` — the same
//! layout the history module already uses — so callers can pass a
//! borrow of the live history vector without copying.

/// Direction of a history scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Direction {
    /// From newer to older entries (emacs `C-r`, vi `/`).
    Backward,
    /// From older to newer entries (emacs `C-s`, vi `?`).
    Forward,
}

/// Find the first history entry (starting at `start` and walking
/// `direction`) whose *prefix* matches `pattern`. `start` is an
/// inclusive index; passing `None` starts from one-past-the-end when
/// going backward and from zero when going forward.
pub(crate) fn find_prefix(
    history: &[Box<[u8]>],
    pattern: &[u8],
    start: Option<usize>,
    direction: Direction,
) -> Option<usize> {
    let len = history.len();
    if len == 0 {
        return None;
    }
    match direction {
        Direction::Backward => {
            let from = start.unwrap_or(len);
            let mut i = from;
            while i > 0 {
                i -= 1;
                if history[i].starts_with(pattern) {
                    return Some(i);
                }
            }
            None
        }
        Direction::Forward => {
            let from = start.unwrap_or(0);
            for i in from..len {
                if history[i].starts_with(pattern) {
                    return Some(i);
                }
            }
            None
        }
    }
}

/// Find the first history entry whose *substring* matches `pattern`.
/// `start` and `direction` behave as in [`find_prefix`].
pub(crate) fn find_substring(
    history: &[Box<[u8]>],
    pattern: &[u8],
    start: Option<usize>,
    direction: Direction,
) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    let len = history.len();
    if len == 0 {
        return None;
    }
    match direction {
        Direction::Backward => {
            let from = start.unwrap_or(len);
            let mut i = from;
            while i > 0 {
                i -= 1;
                if has_substring(&history[i], pattern) {
                    return Some(i);
                }
            }
            None
        }
        Direction::Forward => {
            let from = start.unwrap_or(0);
            for i in from..len {
                if has_substring(&history[i], pattern) {
                    return Some(i);
                }
            }
            None
        }
    }
}

fn has_substring(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > hay.len() {
        return needle.is_empty();
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    fn hist(lines: &[&[u8]]) -> Vec<Box<[u8]>> {
        lines
            .iter()
            .map(|l| l.to_vec().into_boxed_slice())
            .collect()
    }

    #[test]
    fn find_prefix_hits_backward() {
        assert_no_syscalls(|| {
            let h = hist(&[b"echo a", b"ls -l", b"echo b", b"cat"]);
            assert_eq!(find_prefix(&h, b"echo", None, Direction::Backward), Some(2));
            assert_eq!(
                find_prefix(&h, b"echo", Some(2), Direction::Backward),
                Some(0)
            );
        });
    }

    #[test]
    fn find_prefix_miss_returns_none() {
        assert_no_syscalls(|| {
            let h = hist(&[b"ls", b"pwd"]);
            assert_eq!(find_prefix(&h, b"echo", None, Direction::Backward), None);
        });
    }

    #[test]
    fn find_prefix_forward() {
        assert_no_syscalls(|| {
            let h = hist(&[b"echo a", b"ls", b"echo b"]);
            assert_eq!(
                find_prefix(&h, b"echo", Some(1), Direction::Forward),
                Some(2)
            );
        });
    }

    #[test]
    fn find_substring_hits_both_directions() {
        assert_no_syscalls(|| {
            let h = hist(&[b"git status", b"gitk --all", b"ls status"]);
            assert_eq!(
                find_substring(&h, b"status", None, Direction::Backward),
                Some(2)
            );
            assert_eq!(
                find_substring(&h, b"git ", Some(0), Direction::Forward),
                Some(0)
            );
        });
    }

    #[test]
    fn find_substring_empty_returns_none() {
        assert_no_syscalls(|| {
            let h = hist(&[b"x"]);
            assert_eq!(find_substring(&h, b"", None, Direction::Backward), None);
        });
    }

    #[test]
    fn empty_history_returns_none() {
        assert_no_syscalls(|| {
            let h: Vec<Box<[u8]>> = Vec::new();
            assert_eq!(find_prefix(&h, b"x", None, Direction::Backward), None);
            assert_eq!(find_substring(&h, b"x", None, Direction::Forward), None);
        });
    }
}
