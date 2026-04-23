//! Incremental search state (spec § 7).
//!
//! A pure state machine: tests drive it byte-by-byte with a frozen
//! history vector and assert the resulting (pattern, matched-index,
//! failing) tuple. The outer editor is responsible for actually
//! calling [`read_byte`] / [`redraw`] between feeds and for stashing
//! the pre-search buffer so it can be restored on abort.
//!
//! [`read_byte`]: super::super::editor::input::read_byte
//! [`redraw`]: super::super::editor::redraw::redraw

use super::super::editor::history_search::{Direction, find_substring};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchOutcome {
    /// Still in the mini-buffer; caller should redraw using the
    /// `failing` flag.
    Continue,
    /// User pressed RET (or an implicit accept via another binding):
    /// the current `matched` line is the result.
    Accept,
    /// User pressed C-g: caller should restore the pre-search buffer.
    Abort,
    /// Any non-self-insert / non-repeat byte: caller should exit
    /// search and re-dispatch `byte` against the main keymap.
    Exit { byte: u8 },
}

#[derive(Clone, Debug)]
pub(crate) struct IncrementalSearch<'h> {
    pattern: Vec<u8>,
    /// Where to start scanning from when the pattern next changes.
    /// For forward search this is an "at or after" index; for
    /// backward search it's an "at or before" index (exclusive above).
    anchor: Option<usize>,
    /// Current match index into the history, if any.
    matched: Option<usize>,
    /// True when the last search failed (no match found). Appending
    /// more bytes while failing stays failing until the user deletes
    /// or repeats.
    failing: bool,
    direction: Direction,
    history: &'h [Box<[u8]>],
}

impl<'h> IncrementalSearch<'h> {
    pub(crate) fn new(history: &'h [Box<[u8]>], direction: Direction) -> Self {
        Self {
            pattern: Vec::new(),
            anchor: None,
            matched: None,
            failing: false,
            direction,
            history,
        }
    }

    pub(crate) fn pattern(&self) -> &[u8] {
        &self.pattern
    }

    pub(crate) fn matched(&self) -> Option<usize> {
        self.matched
    }

    pub(crate) fn failing(&self) -> bool {
        self.failing
    }

    pub(crate) fn direction(&self) -> Direction {
        self.direction
    }

    /// Append `b` to the pattern and re-run the search from the
    /// current anchor. Used for self-insert bytes inside the mini-
    /// buffer.
    pub(crate) fn push_byte(&mut self, b: u8) {
        self.pattern.push(b);
        self.rerun_from_anchor();
    }

    /// DEL / BS: shorten the pattern by one byte and re-scan from the
    /// original anchor.
    pub(crate) fn backspace(&mut self) {
        if self.pattern.is_empty() {
            return;
        }
        self.pattern.pop();
        self.rerun_from_anchor();
    }

    /// Repeat the current search in `direction`: advance past the
    /// current match.
    pub(crate) fn repeat(&mut self, direction: Direction) {
        self.direction = direction;
        let next_anchor = self.matched.map(|idx| match direction {
            Direction::Backward => idx,
            Direction::Forward => idx + 1,
        });
        self.anchor = next_anchor;
        let scan_start = match direction {
            Direction::Backward => next_anchor,
            Direction::Forward => next_anchor,
        };
        let found = find_substring(self.history, &self.pattern, scan_start, direction);
        match found {
            Some(idx) => {
                self.matched = Some(idx);
                self.failing = false;
            }
            None => {
                self.failing = true;
            }
        }
    }

    fn rerun_from_anchor(&mut self) {
        let found = find_substring(self.history, &self.pattern, self.anchor, self.direction);
        match found {
            Some(idx) => {
                self.matched = Some(idx);
                self.failing = false;
            }
            None => {
                self.failing = true;
            }
        }
    }

    /// Consume a dispatch byte. Returns a [`SearchOutcome`] describing
    /// what the outer editor should do next.
    pub(crate) fn feed(&mut self, byte: u8) -> SearchOutcome {
        match byte {
            0x07 => SearchOutcome::Abort, // C-g
            b'\r' | b'\n' => SearchOutcome::Accept,
            0x12 => {
                // C-r
                self.repeat(Direction::Backward);
                SearchOutcome::Continue
            }
            0x13 => {
                // C-s
                self.repeat(Direction::Forward);
                SearchOutcome::Continue
            }
            0x08 | 0x7f => {
                self.backspace();
                SearchOutcome::Continue
            }
            b if b >= b' ' && b < 0x7f => {
                self.push_byte(b);
                SearchOutcome::Continue
            }
            other => SearchOutcome::Exit { byte: other },
        }
    }
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
    fn backward_push_narrows_to_match() {
        assert_no_syscalls(|| {
            let h = hist(&[b"ls -l", b"echo hi", b"echo bye"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            assert_eq!(s.feed(b'e'), SearchOutcome::Continue);
            assert_eq!(s.matched(), Some(2));
            assert_eq!(s.feed(b'c'), SearchOutcome::Continue);
            assert_eq!(s.matched(), Some(2));
        });
    }

    #[test]
    fn delete_widens_pattern() {
        assert_no_syscalls(|| {
            let h = hist(&[b"cat", b"echo", b"cat"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            s.push_byte(b'c');
            s.push_byte(b'a');
            s.backspace();
            assert_eq!(s.pattern(), b"c");
            assert_eq!(s.matched(), Some(2));
        });
    }

    #[test]
    fn repeat_advances_past_current_match() {
        assert_no_syscalls(|| {
            let h = hist(&[b"echo a", b"ls", b"echo b", b"pwd"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            s.push_byte(b'e');
            assert_eq!(s.matched(), Some(2));
            s.repeat(Direction::Backward);
            assert_eq!(s.matched(), Some(0));
        });
    }

    #[test]
    fn failing_when_no_match() {
        assert_no_syscalls(|| {
            let h = hist(&[b"hi"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            s.push_byte(b'z');
            assert!(s.failing());
            assert_eq!(s.matched(), None);
        });
    }

    #[test]
    fn ctrl_g_aborts() {
        assert_no_syscalls(|| {
            let h = hist(&[b"x"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            assert_eq!(s.feed(0x07), SearchOutcome::Abort);
        });
    }

    #[test]
    fn newline_accepts() {
        assert_no_syscalls(|| {
            let h = hist(&[b"x"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            assert_eq!(s.feed(b'\n'), SearchOutcome::Accept);
            assert_eq!(s.feed(b'\r'), SearchOutcome::Accept);
        });
    }

    #[test]
    fn other_key_exits_with_redispatch() {
        assert_no_syscalls(|| {
            let h = hist(&[b"x"]);
            let mut s = IncrementalSearch::new(&h, Direction::Backward);
            match s.feed(0x01) {
                SearchOutcome::Exit { byte } => assert_eq!(byte, 0x01),
                other => panic!("unexpected outcome: {other:?}"),
            }
        });
    }
}
