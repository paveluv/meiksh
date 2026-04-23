//! Consolidated test-only libc wrappers for the integration-test suite.
//!
//! This is the **single** file in `tests/integration/` permitted to import
//! `libc`, per `docs/IMPLEMENTATION_POLICY.md` § "Low-Level Interface
//! Boundary". Every other integration-test module must consume the safe
//! wrappers defined here rather than importing `libc` itself. The CI check
//! `scripts/check-libc-boundary.sh` enforces this rule mechanically.
//!
//! The helpers are kept intentionally narrow: each wraps exactly the
//! POSIX primitive the existing integration tests need (PTY setup,
//! non-blocking stdin, SIGUSR1-ignore, etc.). Extend this module rather
//! than reintroducing a direct `libc::` call in another test file.

#![allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods,
    dead_code
)]

use std::io;
use std::os::fd::RawFd;

/// Open a fresh pseudo-terminal pair. Returns `(primary, secondary)`
/// raw file descriptors on success. Returns `None` when the host has no
/// PTY support (some CI sandboxes); callers treat that as a test skip
/// rather than a hard failure, matching the existing convention.
pub fn open_pty_pair() -> Option<(RawFd, RawFd)> {
    let mut primary: i32 = -1;
    let mut secondary: i32 = -1;
    let rc = unsafe {
        libc::openpty(
            &mut primary,
            &mut secondary,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return None;
    }
    Some((primary, secondary))
}

/// Duplicate an open file descriptor (unspecified target). Returns the
/// new fd on success; the caller owns it and is responsible for closing
/// it (typically via `Stdio::from_raw_fd` transferring ownership to a
/// child process).
pub fn dup_fd(fd: RawFd) -> io::Result<RawFd> {
    let rc = unsafe { libc::dup(fd) };
    if rc < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(rc)
}

/// Close a file descriptor. Errors are ignored on purpose; close(2) can
/// fail with EBADF if the fd was already closed, which is common during
/// teardown after ownership has been transferred to a child.
pub fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

/// Toggle `O_NONBLOCK` on an open file descriptor.
pub fn set_nonblocking(fd: RawFd, nonblocking: bool) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    let new_flags = if nonblocking {
        flags | libc::O_NONBLOCK
    } else {
        flags & !libc::O_NONBLOCK
    };
    if unsafe { libc::fcntl(fd, libc::F_SETFL, new_flags) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Start a new session and adopt `fd` as the controlling terminal. When
/// `dup_stderr` is true, also `dup2(fd, 2)` so the child's stderr goes
/// to the terminal.
///
/// Intended to be called from inside a child `pre_exec` closure. The
/// caller must be a single-threaded child process between `fork` and
/// `execve`.
pub fn make_controlling_tty_in_child(fd: RawFd, dup_stderr: bool) -> io::Result<()> {
    if unsafe { libc::setsid() } < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::ioctl(fd, libc::TIOCSCTTY as _, 0) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if dup_stderr && unsafe { libc::dup2(fd, 2) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Install `SIG_IGN` on SIGUSR1 for the current process. Intended for
/// child `pre_exec` closures that want to test the "ignored on entry"
/// signal path.
pub fn ignore_sigusr1() -> io::Result<()> {
    let rc = unsafe { libc::signal(libc::SIGUSR1, libc::SIG_IGN) };
    if rc == libc::SIG_ERR {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
