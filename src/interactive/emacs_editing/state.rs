//! Mutable per-read_line state for the emacs editor.
//!
//! All fields are `pub(super)` so the sibling [`functions`],
//! [`keymap`], and [`undo`] modules can mutate them directly without
//! opaque setter methods for every operation. The struct itself is
//! created exactly once per call to [`super::read_line`] and lives on
//! the stack there; no heap ownership is transferred out.
//!
//! [`functions`]: super::functions
//! [`keymap`]: super::keymap
//! [`undo`]: super::undo

use super::keymap::EmacsFn;
use super::kill_buffer::KillBuffer;
use super::undo::UndoStack;

/// Tracks whether `yank-last-arg` is walking through prior commands
/// (spec 5.3). Any non-`yank_last_arg` dispatch clears it.
#[derive(Clone, Debug, Default)]
pub(crate) struct YankArgState {
    /// Index into history (from most recent backward) picked on the
    /// previous yank-last-arg invocation.
    pub(super) hist_offset: usize,
    /// Byte offset in the line where the most recent yank was
    /// inserted — undone before the next yank.
    pub(super) last_insert_at: usize,
    /// Length of the most recently yanked argument, so the next call
    /// can replace it in place.
    pub(super) last_insert_len: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct EmacsState {
    pub(super) buf: Vec<u8>,
    pub(super) cursor: usize,
    pub(super) kill: KillBuffer,
    pub(super) undo: UndoStack,
    pub(super) hist_index: Option<usize>,
    pub(super) edit_line: Vec<u8>,
    pub(super) last_fn: Option<EmacsFn>,
    pub(super) yank_last_arg: Option<YankArgState>,
    pub(super) erase_char: u8,
    pub(super) paste_group: Option<Vec<u8>>,
    pub(super) accepted: bool,
    pub(super) eof: bool,
}

impl EmacsState {
    pub(super) fn new(erase_char: u8) -> Self {
        Self {
            buf: Vec::new(),
            cursor: 0,
            kill: KillBuffer::new(),
            undo: UndoStack::new(),
            hist_index: None,
            edit_line: Vec::new(),
            last_fn: None,
            yank_last_arg: None,
            erase_char,
            paste_group: None,
            accepted: false,
            eof: false,
        }
    }

    pub(super) fn begin_paste_group(&mut self) {
        self.paste_group = Some(Vec::new());
    }

    pub(super) fn insert_paste_byte(&mut self, b: u8) {
        if let Some(g) = self.paste_group.as_mut() {
            g.push(b);
        }
    }

    pub(super) fn end_paste_group(&mut self) {
        if let Some(group) = self.paste_group.take() {
            self.insert_bytes_at_cursor(&group);
        }
    }

    /// Insert `bytes` at the current cursor position and advance the
    /// cursor past them. This is the canonical entry point for
    /// `self-insert` and bracketed-paste runs.
    pub(super) fn insert_bytes_at_cursor(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let at = self.cursor;
        self.buf.splice(at..at, bytes.iter().copied());
        self.cursor += bytes.len();
        self.undo.record_insert(at, bytes.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn insert_bytes_advances_cursor_and_records_undo() {
        assert_no_syscalls(|| {
            let mut s = EmacsState::new(0x7f);
            s.insert_bytes_at_cursor(b"abc");
            assert_eq!(s.buf, b"abc");
            assert_eq!(s.cursor, 3);
            s.insert_bytes_at_cursor(b"X");
            assert_eq!(s.buf, b"abcX");
            assert_eq!(s.cursor, 4);
        });
    }

    #[test]
    fn paste_group_collapses_into_single_insert() {
        assert_no_syscalls(|| {
            let mut s = EmacsState::new(0x7f);
            s.begin_paste_group();
            s.insert_paste_byte(b'h');
            s.insert_paste_byte(b'i');
            s.end_paste_group();
            assert_eq!(s.buf, b"hi");
            assert_eq!(s.cursor, 2);
        });
    }
}
