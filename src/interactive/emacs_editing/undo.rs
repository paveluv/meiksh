//! Per-line undo stack (spec § 9).
//!
//! The stack owns one [`UndoEntry`] per editing group. Consecutive
//! self-insert keystrokes coalesce into a single `Inserted` entry;
//! every other bindable function records a distinct entry. `undo`
//! pops the most recent entry and reverses its effect. `accept-line`
//! drains the stack.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum UndoEntry {
    Inserted {
        at: usize,
        bytes: Vec<u8>,
    },
    Killed {
        at: usize,
        bytes: Vec<u8>,
    },
    Yanked {
        at: usize,
        len: usize,
    },
    TransposeChars {
        at: usize,
        a_len: usize,
        b_len: usize,
    },
    TransposeWords {
        at: usize,
        left_len: usize,
        gap_len: usize,
        right_len: usize,
    },
    CaseChange {
        at: usize,
        before: Vec<u8>,
    },
    Paste {
        at: usize,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UndoStack {
    stack: Vec<UndoEntry>,
}

impl UndoStack {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Append a fresh entry to the stack. Self-insert runs call this
    /// once per burst, coalescing via `record_insert`.
    pub(crate) fn push(&mut self, entry: UndoEntry) {
        self.stack.push(entry);
    }

    /// Record a `self-insert` burst: coalesce with the previous
    /// [`UndoEntry::Inserted`] entry if it's immediately adjacent,
    /// otherwise push a new one.
    pub(crate) fn record_insert(&mut self, at: usize, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(UndoEntry::Inserted {
            at: prev_at,
            bytes: prev_bytes,
        }) = self.stack.last_mut()
        {
            if *prev_at + prev_bytes.len() == at {
                prev_bytes.extend_from_slice(&bytes);
                return;
            }
        }
        self.stack.push(UndoEntry::Inserted { at, bytes });
    }

    /// Pop the most recent entry, applying its inverse to `buf`/`cursor`.
    /// Returns `false` when the stack is empty so the caller can bell.
    pub(crate) fn undo(&mut self, buf: &mut Vec<u8>, cursor: &mut usize) -> bool {
        let Some(entry) = self.stack.pop() else {
            return false;
        };
        match entry {
            UndoEntry::Inserted { at, bytes } => {
                buf.drain(at..at + bytes.len());
                *cursor = at;
            }
            UndoEntry::Killed { at, bytes } => {
                buf.splice(at..at, bytes.iter().copied());
                *cursor = at + bytes.len();
            }
            UndoEntry::Yanked { at, len } => {
                buf.drain(at..at + len);
                *cursor = at;
            }
            UndoEntry::Paste { at, bytes } => {
                buf.drain(at..at + bytes.len());
                *cursor = at;
            }
            UndoEntry::CaseChange { at, before } => {
                let len = before.len();
                buf.splice(at..at + len, before.iter().copied());
                *cursor = at + len;
            }
            UndoEntry::TransposeChars { at, a_len, b_len } => {
                // [a][b] was swapped to [b][a]; reverse by copying
                // from the current buffer and restoring the order.
                let total = a_len + b_len;
                let mut tmp = buf[at..at + total].to_vec();
                let swapped_b = tmp[..b_len].to_vec();
                let swapped_a = tmp[b_len..].to_vec();
                tmp.clear();
                tmp.extend_from_slice(&swapped_a);
                tmp.extend_from_slice(&swapped_b);
                buf.splice(at..at + total, tmp.into_iter());
                *cursor = at + total;
            }
            UndoEntry::TransposeWords {
                at,
                left_len,
                gap_len,
                right_len,
            } => {
                let total = left_len + gap_len + right_len;
                let region = buf[at..at + total].to_vec();
                // After the transpose the buffer holds right | gap | left;
                // rebuild left | gap | right.
                let new_right = region[..right_len].to_vec();
                let new_gap = region[right_len..right_len + gap_len].to_vec();
                let new_left = region[right_len + gap_len..].to_vec();
                let mut rebuilt = Vec::with_capacity(total);
                rebuilt.extend_from_slice(&new_left);
                rebuilt.extend_from_slice(&new_gap);
                rebuilt.extend_from_slice(&new_right);
                buf.splice(at..at + total, rebuilt.into_iter());
                *cursor = at + total;
            }
        }
        true
    }

    pub(crate) fn clear(&mut self) {
        self.stack.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn inserted_coalesces_adjacent_runs() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            s.record_insert(0, b"ab".to_vec());
            s.record_insert(2, b"cd".to_vec());
            assert_eq!(s.len(), 1);
        });
    }

    #[test]
    fn non_adjacent_inserts_push_new_entry() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            s.record_insert(0, b"ab".to_vec());
            s.record_insert(5, b"cd".to_vec());
            assert_eq!(s.len(), 2);
        });
    }

    #[test]
    fn undo_inserted_removes_span() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = b"hello".to_vec();
            let mut c = 5usize;
            s.record_insert(0, b"hello".to_vec());
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"");
            assert_eq!(c, 0);
        });
    }

    #[test]
    fn undo_killed_reinserts_bytes() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = b"ab".to_vec();
            let mut c = 2usize;
            s.push(UndoEntry::Killed {
                at: 2,
                bytes: b"cde".to_vec(),
            });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"abcde");
            assert_eq!(c, 5);
        });
    }

    #[test]
    fn undo_empty_returns_false() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = Vec::new();
            let mut c = 0;
            assert!(!s.undo(&mut buf, &mut c));
        });
    }

    #[test]
    fn undo_transpose_chars_restores_order() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            // Before: "ab". After transpose: "ba". Undo should yield "ab".
            let mut buf = b"ba".to_vec();
            let mut c = 2usize;
            s.push(UndoEntry::TransposeChars {
                at: 0,
                a_len: 1,
                b_len: 1,
            });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"ab");
        });
    }

    #[test]
    fn undo_case_change_restores_prior_bytes() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = b"HELLO".to_vec();
            let mut c = 5usize;
            s.push(UndoEntry::CaseChange {
                at: 0,
                before: b"hello".to_vec(),
            });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"hello");
        });
    }

    #[test]
    fn record_insert_empty_is_noop() {
        // An empty-bytes record must not push an entry, otherwise
        // `undo` would see a zero-width range to drain.
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            s.record_insert(0, Vec::new());
            assert_eq!(s.len(), 0);
        });
    }

    #[test]
    fn undo_yanked_drains_span_and_resets_cursor() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            // Yank wrote `xy` at offset 2; buffer now holds "abxycd"
            // (original was "abcd", cursor moved past the yank).
            let mut buf = b"abxycd".to_vec();
            let mut c = 4usize;
            s.push(UndoEntry::Yanked { at: 2, len: 2 });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"abcd");
            assert_eq!(c, 2);
        });
    }

    #[test]
    fn undo_paste_drains_inserted_bytes() {
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = b"[hi]".to_vec();
            let mut c = 3usize;
            s.push(UndoEntry::Paste {
                at: 1,
                bytes: b"hi".to_vec(),
            });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"[]");
            assert_eq!(c, 1);
        });
    }

    #[test]
    fn undo_transpose_words_rebuilds_left_gap_right() {
        // Before transpose: "foo   bar"; after transpose: "bar   foo".
        // The undo entry records the original left/gap/right lengths
        // from before the swap. Undo must restore "foo   bar".
        assert_no_syscalls(|| {
            let mut s = UndoStack::new();
            let mut buf = b"bar   foo".to_vec();
            let mut c = buf.len();
            s.push(UndoEntry::TransposeWords {
                at: 0,
                left_len: 3,  // "foo"
                gap_len: 3,   // "   "
                right_len: 3, // "bar"
            });
            assert!(s.undo(&mut buf, &mut c));
            assert_eq!(buf, b"foo   bar");
            assert_eq!(c, 9);
        });
    }
}
