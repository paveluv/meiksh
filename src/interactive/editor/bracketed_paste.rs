//! Bracketed-paste sequences and a streaming detector for the
//! `\e[200~` / `\e[201~` frame markers.
//!
//! Enabling bracketed paste (`\e[?2004h`) causes a compliant terminal
//! to wrap literally-pasted content in the start/end sequences below,
//! so the editor can treat the entire pasted region as a single
//! `self-insert` run that bypasses keymap dispatch. See spec
//! [docs/features/emacs-editing-mode.md] sections 3.2 and 8.

use crate::sys;

pub(crate) const PASTE_START: &[u8] = b"\x1b[200~";
pub(crate) const PASTE_END: &[u8] = b"\x1b[201~";

const ENABLE_BRACKETED_PASTE: &[u8] = b"\x1b[?2004h";
const DISABLE_BRACKETED_PASTE: &[u8] = b"\x1b[?2004l";

/// Emit the "enable bracketed paste" terminal sequence on stdout.
pub(crate) fn enter_paste_mode() {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, ENABLE_BRACKETED_PASTE);
}

/// Emit the "disable bracketed paste" terminal sequence on stdout.
pub(crate) fn leave_paste_mode() {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, DISABLE_BRACKETED_PASTE);
}

/// Outcome of feeding a byte into the streaming frame detector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FrameEvent {
    /// No frame boundary was detected; the byte remains queued in the
    /// detector's internal buffer, waiting for more input to decide.
    Pending,
    /// A complete start marker was just consumed. Subsequent bytes
    /// belong to the pasted content until the next `End` event.
    Start,
    /// A complete end marker was just consumed. The surrounding
    /// dispatch loop should close the paste group.
    End,
    /// The current prefix can't be a start/end marker. The buffered
    /// bytes are emitted as literals; the caller should treat them as
    /// ordinary keystrokes.
    EmitLiteral(Vec<u8>),
}

/// Streaming start/end-marker detector. Fed one byte at a time, it
/// accumulates any partial prefix of `ESC [ 2 0 0 ~` / `ESC [ 2 0 1 ~`
/// and emits an event once the ambiguity is resolved.
///
/// This is intentionally pure: the dispatch loop reads bytes, feeds
/// them here, and acts on the returned event. That keeps the actual
/// I/O out of unit tests.
#[derive(Default)]
pub(crate) struct FrameDetector {
    buf: Vec<u8>,
}

impl FrameDetector {
    pub(crate) fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Feed one byte into the detector.
    pub(crate) fn feed(&mut self, byte: u8) -> FrameEvent {
        self.buf.push(byte);
        let start_prefix = PASTE_START.starts_with(&self.buf[..]);
        let end_prefix = PASTE_END.starts_with(&self.buf[..]);
        if self.buf[..] == *PASTE_START {
            self.buf.clear();
            return FrameEvent::Start;
        }
        if self.buf[..] == *PASTE_END {
            self.buf.clear();
            return FrameEvent::End;
        }
        if start_prefix || end_prefix {
            return FrameEvent::Pending;
        }
        let drained = std::mem::take(&mut self.buf);
        FrameEvent::EmitLiteral(drained)
    }

    /// Drain any bytes buffered while waiting for a frame marker.
    /// Returns an empty slice when nothing is pending. The dispatch
    /// loop should call this at end-of-input so partial prefixes are
    /// emitted as literal keystrokes instead of being lost.
    pub(crate) fn drain(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn frame_detector_recognizes_start_marker() {
        assert_no_syscalls(|| {
            let mut d = FrameDetector::new();
            let mut events = Vec::new();
            for &b in PASTE_START {
                events.push(d.feed(b));
            }
            assert!(matches!(events.last(), Some(FrameEvent::Start)));
            assert_eq!(
                events[..events.len() - 1]
                    .iter()
                    .filter(|e| !matches!(e, FrameEvent::Pending))
                    .count(),
                0
            );
        });
    }

    #[test]
    fn frame_detector_recognizes_end_marker() {
        assert_no_syscalls(|| {
            let mut d = FrameDetector::new();
            let mut events = Vec::new();
            for &b in PASTE_END {
                events.push(d.feed(b));
            }
            assert!(matches!(events.last(), Some(FrameEvent::End)));
        });
    }

    #[test]
    fn frame_detector_emits_literals_on_mismatch() {
        assert_no_syscalls(|| {
            let mut d = FrameDetector::new();
            let e1 = d.feed(b'\x1b'); // matches prefix
            let e2 = d.feed(b'['); // still matches prefix
            let e3 = d.feed(b'x'); // breaks prefix
            assert!(matches!(e1, FrameEvent::Pending));
            assert!(matches!(e2, FrameEvent::Pending));
            match e3 {
                FrameEvent::EmitLiteral(bytes) => assert_eq!(bytes, b"\x1b[x"),
                _ => panic!("expected EmitLiteral"),
            }
        });
    }

    #[test]
    fn frame_detector_drain_recovers_pending_bytes() {
        assert_no_syscalls(|| {
            let mut d = FrameDetector::new();
            let _ = d.feed(b'\x1b');
            let _ = d.feed(b'[');
            assert_eq!(d.drain(), b"\x1b[");
            assert_eq!(d.drain(), b"");
        });
    }

    #[test]
    fn frame_detector_ordinary_byte_is_literal() {
        assert_no_syscalls(|| {
            let mut d = FrameDetector::new();
            match d.feed(b'a') {
                FrameEvent::EmitLiteral(b) => assert_eq!(b, b"a"),
                _ => panic!("expected EmitLiteral for ordinary byte"),
            }
        });
    }
}
