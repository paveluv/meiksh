//! RAII wrapper around raw-mode termios setup for interactive editors.

use crate::sys;

/// Switches the terminal connected to `stdin` into raw mode on
/// construction (disables ICANON, ECHO, ISIG), restoring the original
/// attributes on drop. Borrow the saved termios via [`Self::saved`] if
/// a client needs to step outside raw mode temporarily (for example
/// to hand the terminal to an external editor).
pub(crate) struct RawMode {
    saved: sys::types::Termios,
}

impl RawMode {
    pub(crate) fn enter() -> sys::error::SysResult<Self> {
        let saved = sys::tty::get_terminal_attrs(sys::constants::STDIN_FILENO)?;
        let mut raw = saved;
        raw.c_lflag &= !(sys::constants::ICANON | sys::constants::ECHO | sys::constants::ISIG);
        raw.c_cc[sys::constants::VMIN] = 1;
        raw.c_cc[sys::constants::VTIME] = 0;
        sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &raw)?;
        Ok(Self { saved })
    }

    /// Borrow the original termios captured at `enter()`-time. Useful
    /// when an editor temporarily restores canonical mode (running an
    /// external editor, delivering SIGTSTP) and wants to re-derive the
    /// raw flags from the same baseline.
    pub(crate) fn saved(&self) -> &sys::types::Termios {
        &self.saved
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = sys::tty::set_terminal_attrs(sys::constants::STDIN_FILENO, &self.saved);
    }
}
