//! Blocking byte-level I/O primitives used by the interactive editors.
//!
//! These wrap [`crate::sys::fd_io`] so keystroke dispatch loops and
//! redraw code don't have to thread syscalls or STDIN/STDOUT file
//! descriptor constants through every call site.

use crate::sys;

/// Read a single byte from `stdin`. Returns `Ok(None)` on EOF (which
/// matters for C-d / piped input), `Ok(Some(b))` for a real byte, or
/// the underlying [`SysError`] on read failure.
///
/// [`SysError`]: crate::sys::error::SysError
pub(crate) fn read_byte() -> sys::error::SysResult<Option<u8>> {
    let mut buf = [0u8; 1];
    match sys::fd_io::read_fd(sys::constants::STDIN_FILENO, &mut buf) {
        Ok(0) => Ok(None),
        Ok(_) => Ok(Some(buf[0])),
        Err(e) => Err(e),
    }
}

/// Best-effort write of `data` to `stdout`. Errors are swallowed on
/// purpose; the editor can't do anything useful with a failed redraw
/// and the caller is usually rendering state it already committed to
/// an internal buffer.
pub(crate) fn write_bytes(data: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, data);
}

/// Ring the terminal bell.
pub(crate) fn bell() {
    write_bytes(b"\x07");
}
