//! The emacs kill buffer (spec § 6).
//!
//! Successive kill operations append (or prepend) bytes to the same
//! logical buffer, while any non-kill dispatch causes the next kill
//! to *replace* the buffer. The append/prepend distinction depends on
//! direction: forward kills (C-k, M-d) append, backward kills (C-w,
//! M-DEL) prepend. `yank` (C-y) pastes the current buffer.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KillDirection {
    Forward,
    Backward,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct KillBuffer {
    bytes: Vec<u8>,
    /// Whether the most recent recorded dispatch was a kill. Any
    /// non-kill operation resets this so the next kill replaces
    /// instead of appending.
    last_was_kill: bool,
}

impl KillBuffer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Accept a killed span. Called by every kill function. Returns
    /// the new buffer contents for callers that want to echo.
    pub(crate) fn kill(&mut self, span: Vec<u8>, direction: KillDirection) -> &[u8] {
        if self.last_was_kill {
            match direction {
                KillDirection::Forward => self.bytes.extend_from_slice(&span),
                KillDirection::Backward => {
                    let mut new = span;
                    new.extend_from_slice(&self.bytes);
                    self.bytes = new;
                }
            }
        } else {
            self.bytes = span;
        }
        self.last_was_kill = true;
        &self.bytes
    }

    /// Mark that a non-kill dispatch ran: the next kill will replace.
    pub(crate) fn mark_non_kill(&mut self) {
        self.last_was_kill = false;
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn replace_when_no_previous_kill() {
        assert_no_syscalls(|| {
            let mut k = KillBuffer::new();
            k.kill(b"abc".to_vec(), KillDirection::Forward);
            assert_eq!(k.as_slice(), b"abc");
        });
    }

    #[test]
    fn forward_kills_append() {
        assert_no_syscalls(|| {
            let mut k = KillBuffer::new();
            k.kill(b"abc".to_vec(), KillDirection::Forward);
            k.kill(b"def".to_vec(), KillDirection::Forward);
            assert_eq!(k.as_slice(), b"abcdef");
        });
    }

    #[test]
    fn backward_kills_prepend() {
        assert_no_syscalls(|| {
            let mut k = KillBuffer::new();
            k.kill(b"def".to_vec(), KillDirection::Forward);
            k.kill(b"abc".to_vec(), KillDirection::Backward);
            assert_eq!(k.as_slice(), b"abcdef");
        });
    }

    #[test]
    fn non_kill_marker_resets_append() {
        assert_no_syscalls(|| {
            let mut k = KillBuffer::new();
            k.kill(b"abc".to_vec(), KillDirection::Forward);
            k.mark_non_kill();
            k.kill(b"xyz".to_vec(), KillDirection::Forward);
            assert_eq!(k.as_slice(), b"xyz");
        });
    }

    #[test]
    fn mixed_directions_after_mark_replace() {
        assert_no_syscalls(|| {
            let mut k = KillBuffer::new();
            k.kill(b"abc".to_vec(), KillDirection::Backward);
            k.mark_non_kill();
            k.kill(b"def".to_vec(), KillDirection::Backward);
            assert_eq!(k.as_slice(), b"def");
        });
    }
}
