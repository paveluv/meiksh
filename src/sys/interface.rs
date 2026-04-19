use libc::{self, c_char, c_int, c_long, mode_t};
#[cfg(not(test))]
use std::ffi::CStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::constants::{
    SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT, SIGFPE, SIGHUP, SIGILL, SIGINT, SIGPIPE, SIGQUIT,
    SIGSEGV, SIGSYS, SIGTERM, SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGUSR1, SIGUSR2,
};
use super::error::{SysError, SysResult};
use super::types::{ClockTicks, FileModeMask, Pid};
use crate::hash::ShellMap;

// ---------------------------------------------------------------------------
// Signal mask helpers
// ---------------------------------------------------------------------------

pub(super) fn signal_mask(signal: c_int) -> Option<usize> {
    let bit = match signal {
        SIGHUP => 0,
        SIGINT => 1,
        SIGQUIT => 2,
        SIGILL => 3,
        SIGABRT => 4,
        SIGFPE => 5,
        SIGBUS => 6,
        SIGUSR1 => 7,
        SIGSEGV => 8,
        SIGUSR2 => 9,
        SIGPIPE => 10,
        SIGALRM => 11,
        SIGTERM => 12,
        SIGCHLD => 13,
        SIGTSTP => 14,
        SIGTTIN => 15,
        SIGTTOU => 16,
        SIGSYS => 17,
        SIGCONT => 18,
        SIGTRAP => 19,
        _ => return None,
    };
    Some(1usize << bit)
}

static PENDING_SIGNALS: AtomicUsize = AtomicUsize::new(0);

pub(super) extern "C" fn record_signal(sig: c_int) {
    if let Some(mask) = signal_mask(sig) {
        // Relaxed is sufficient: the kernel provides the memory barrier
        // when delivering a signal, and the shell only observes
        // `PENDING_SIGNALS` between commands on the same thread.
        PENDING_SIGNALS.fetch_or(mask, Ordering::Relaxed);
    }
}

// Signal-state helpers. In production they read/clear the real
// `PENDING_SIGNALS` atomic that `record_signal` updates from the
// signal handler; in tests they delegate to the thread-local
// `TEST_PENDING_SIGNALS` maintained by `test_support`.

#[cfg(not(test))]
pub(super) fn pending_signal_bits() -> usize {
    PENDING_SIGNALS.load(Ordering::Relaxed)
}

#[cfg(test)]
pub(super) fn pending_signal_bits() -> usize {
    super::test_support::test_pending_signal_bits()
}

#[cfg(not(test))]
pub(super) fn take_pending_signal_bits() -> usize {
    PENDING_SIGNALS.swap(0, Ordering::Relaxed)
}

#[cfg(test)]
pub(super) fn take_pending_signal_bits() -> usize {
    super::test_support::test_take_pending_signal_bits()
}

// ---------------------------------------------------------------------------
// errno helpers
// ---------------------------------------------------------------------------

pub(super) fn set_errno(errno: c_int) {
    #[cfg(test)]
    {
        super::test_support::set_test_errno(errno);
        return;
    }

    #[cfg(not(test))]
    unsafe {
        *errno_ptr() = errno;
    }
}

#[cfg(not(test))]
unsafe fn errno_ptr() -> *mut c_int {
    #[cfg(target_os = "macos")]
    {
        unsafe { libc::__error() }
    }
    #[cfg(target_os = "linux")]
    {
        unsafe { libc::__errno_location() }
    }
    #[cfg(target_os = "freebsd")]
    {
        unsafe { libc::__error() }
    }
}

pub(super) fn last_error() -> SysError {
    #[cfg(test)]
    {
        return super::test_support::take_test_error();
    }

    #[cfg(not(test))]
    SysError::Errno(unsafe { *errno_ptr() })
}

#[cfg(coverage)]
pub(super) fn flush_coverage() {
    unsafe {
        unsafe extern "C" {
            fn __llvm_profile_write_file() -> c_int;
        }
        __llvm_profile_write_file();
    }
}

// ---------------------------------------------------------------------------
// Syscall wrappers
//
// Each syscall is provided as a pair of `#[cfg]`-gated free functions:
//
// * `#[cfg(not(test))]` goes straight to `libc::*`.
// * `#[cfg(test)]` routes into the matching `trace_*` dispatcher in
//   `test_support`, which consumes one entry from the thread-local
//   `TRACE_LOG` (set up by `run_trace` / `assert_no_syscalls`).
//
// There is no fn-pointer vtable, no runtime dispatch, and — crucially —
// no code path from `#[cfg(test)]` down to real libc: tests that forget
// to script a syscall panic loudly instead of silently escaping to the
// host kernel.
// ---------------------------------------------------------------------------

#[cfg(not(test))]
pub(super) fn getpid() -> Pid {
    unsafe { libc::getpid() }
}
#[cfg(test)]
pub(super) fn getpid() -> Pid {
    super::test_support::trace_getpid()
}

#[cfg(not(test))]
pub(super) fn getppid() -> Pid {
    unsafe { libc::getppid() }
}
#[cfg(test)]
pub(super) fn getppid() -> Pid {
    super::test_support::trace_getppid()
}

#[cfg(not(test))]
pub(super) fn waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
    unsafe { libc::waitpid(pid, status, options) }
}
#[cfg(test)]
pub(super) fn waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
    super::test_support::trace_waitpid(pid, status, options)
}

#[cfg(not(test))]
pub(super) fn kill(pid: Pid, sig: c_int) -> c_int {
    unsafe { libc::kill(pid, sig) }
}
#[cfg(test)]
pub(super) fn kill(pid: Pid, sig: c_int) -> c_int {
    super::test_support::trace_kill(pid, sig)
}

#[cfg(not(test))]
pub(super) fn signal(sig: c_int, handler: libc::sighandler_t) -> libc::sighandler_t {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = handler;
        libc::sigemptyset(&mut sa.sa_mask);
        let mut old_sa: libc::sigaction = std::mem::zeroed();
        let rc = libc::sigaction(sig, &sa, &mut old_sa);
        [old_sa.sa_sigaction, libc::SIG_ERR][(rc < 0) as usize]
    }
}
#[cfg(test)]
pub(super) fn signal(sig: c_int, handler: libc::sighandler_t) -> libc::sighandler_t {
    super::test_support::trace_signal(sig, handler)
}

#[cfg(not(test))]
pub(super) fn isatty(fd: c_int) -> c_int {
    unsafe { libc::isatty(fd) }
}
#[cfg(test)]
pub(super) fn isatty(fd: c_int) -> c_int {
    super::test_support::trace_isatty(fd)
}

#[cfg(not(test))]
pub(super) fn tcgetpgrp(fd: c_int) -> Pid {
    unsafe { libc::tcgetpgrp(fd) }
}
#[cfg(test)]
pub(super) fn tcgetpgrp(fd: c_int) -> Pid {
    super::test_support::trace_tcgetpgrp(fd)
}

#[cfg(not(test))]
pub(super) fn tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int {
    unsafe { libc::tcsetpgrp(fd, pgrp) }
}
#[cfg(test)]
pub(super) fn tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int {
    super::test_support::trace_tcsetpgrp(fd, pgrp)
}

#[cfg(not(test))]
pub(super) fn setpgid(pid: Pid, pgid: Pid) -> c_int {
    unsafe { libc::setpgid(pid, pgid) }
}
#[cfg(test)]
pub(super) fn setpgid(pid: Pid, pgid: Pid) -> c_int {
    super::test_support::trace_setpgid(pid, pgid)
}

#[cfg(not(test))]
pub(super) fn pipe(fds: &mut [c_int; 2]) -> c_int {
    unsafe { libc::pipe(fds.as_mut_ptr()) }
}
#[cfg(test)]
pub(super) fn pipe(fds: &mut [c_int; 2]) -> c_int {
    super::test_support::trace_pipe(fds)
}

#[cfg(not(test))]
pub(super) fn dup2(oldfd: c_int, newfd: c_int) -> c_int {
    unsafe { libc::dup2(oldfd, newfd) }
}
#[cfg(test)]
pub(super) fn dup2(oldfd: c_int, newfd: c_int) -> c_int {
    super::test_support::trace_dup2(oldfd, newfd)
}

#[cfg(not(test))]
pub(super) fn close(fd: c_int) -> c_int {
    unsafe { libc::close(fd) }
}
#[cfg(test)]
pub(super) fn close(fd: c_int) -> c_int {
    super::test_support::trace_close(fd)
}

#[cfg(not(test))]
pub(super) fn fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int {
    unsafe { libc::fcntl(fd, cmd, arg) }
}
#[cfg(test)]
pub(super) fn fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int {
    super::test_support::trace_fcntl(fd, cmd, arg)
}

#[cfg(not(test))]
pub(super) fn read(fd: c_int, buf: &mut [u8]) -> isize {
    unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) }
}
#[cfg(test)]
pub(super) fn read(fd: c_int, buf: &mut [u8]) -> isize {
    super::test_support::trace_read(fd, buf)
}

#[cfg(not(test))]
pub(super) fn umask(cmask: FileModeMask) -> FileModeMask {
    unsafe { libc::umask(cmask) }
}
#[cfg(test)]
pub(super) fn umask(cmask: FileModeMask) -> FileModeMask {
    super::test_support::trace_umask(cmask)
}

#[cfg(not(test))]
pub(super) fn times(buffer: *mut libc::tms) -> ClockTicks {
    unsafe { libc::times(buffer) }
}
#[cfg(test)]
pub(super) fn times(buffer: *mut libc::tms) -> ClockTicks {
    super::test_support::trace_times(buffer)
}

#[cfg(not(test))]
pub(super) fn sysconf(name: c_int) -> c_long {
    unsafe { libc::sysconf(name) }
}
#[cfg(test)]
pub(super) fn sysconf(name: c_int) -> c_long {
    super::test_support::trace_sysconf(name)
}

#[cfg(not(test))]
pub(super) fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int {
    unsafe { libc::execvp(file, argv) }
}
#[cfg(test)]
pub(super) fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int {
    super::test_support::trace_execvp(file, argv)
}

#[cfg(not(test))]
pub(super) fn execve(
    file: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    unsafe { libc::execve(file, argv, envp) }
}
#[cfg(test)]
pub(super) fn execve(
    file: *const c_char,
    argv: *const *const c_char,
    envp: *const *const c_char,
) -> c_int {
    super::test_support::trace_execve(file, argv, envp)
}

#[cfg(not(test))]
pub(super) fn open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    unsafe { libc::open(path, flags, mode as c_int) }
}
#[cfg(test)]
pub(super) fn open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    super::test_support::trace_open(path, flags, mode)
}

#[cfg(not(test))]
pub(super) fn write(fd: c_int, data: &[u8]) -> isize {
    unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) }
}
#[cfg(test)]
pub(super) fn write(fd: c_int, data: &[u8]) -> isize {
    super::test_support::trace_write(fd, data)
}

#[cfg(not(test))]
pub(super) fn stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    unsafe { libc::stat(path, buf) }
}
#[cfg(test)]
pub(super) fn stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    super::test_support::trace_stat(path, buf)
}

#[cfg(not(test))]
pub(super) fn lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    unsafe { libc::lstat(path, buf) }
}
#[cfg(test)]
pub(super) fn lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    super::test_support::trace_lstat(path, buf)
}

#[cfg(not(test))]
pub(super) fn fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
    unsafe { libc::fstat(fd, buf) }
}
#[cfg(test)]
pub(super) fn fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
    super::test_support::trace_fstat(fd, buf)
}

#[cfg(not(test))]
pub(super) fn access(path: *const c_char, mode: c_int) -> c_int {
    unsafe { libc::access(path, mode) }
}
#[cfg(test)]
pub(super) fn access(path: *const c_char, mode: c_int) -> c_int {
    super::test_support::trace_access(path, mode)
}

#[cfg(not(test))]
pub(super) fn chdir(path: *const c_char) -> c_int {
    unsafe { libc::chdir(path) }
}
#[cfg(test)]
pub(super) fn chdir(path: *const c_char) -> c_int {
    super::test_support::trace_chdir(path)
}

#[cfg(not(test))]
pub(super) fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    unsafe { libc::getcwd(buf, size) }
}
#[cfg(test)]
pub(super) fn getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    super::test_support::trace_getcwd(buf, size)
}

#[cfg(not(test))]
pub(super) fn opendir(path: *const c_char) -> *mut libc::DIR {
    unsafe { libc::opendir(path) }
}
#[cfg(test)]
pub(super) fn opendir(path: *const c_char) -> *mut libc::DIR {
    super::test_support::trace_opendir(path)
}

#[cfg(not(test))]
pub(super) fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
    unsafe { libc::readdir(dirp) }
}
#[cfg(test)]
pub(super) fn readdir(dirp: *mut libc::DIR) -> *mut libc::dirent {
    super::test_support::trace_readdir(dirp)
}

#[cfg(not(test))]
pub(super) fn closedir(dirp: *mut libc::DIR) -> c_int {
    unsafe { libc::closedir(dirp) }
}
#[cfg(test)]
pub(super) fn closedir(dirp: *mut libc::DIR) -> c_int {
    super::test_support::trace_closedir(dirp)
}

#[cfg(not(test))]
pub(super) fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
    unsafe { libc::realpath(path, resolved) }
}
#[cfg(test)]
pub(super) fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
    super::test_support::trace_realpath(path, resolved)
}

#[cfg(not(test))]
pub(super) fn unlink(path: *const c_char) -> c_int {
    unsafe { libc::unlink(path) }
}
#[cfg(test)]
pub(super) fn unlink(path: *const c_char) -> c_int {
    super::test_support::trace_unlink(path)
}

#[cfg(not(test))]
pub(super) fn fork() -> Pid {
    unsafe { libc::fork() }
}
#[cfg(test)]
pub(super) fn fork() -> Pid {
    super::test_support::trace_fork()
}

#[cfg(not(test))]
pub(super) fn exit_process(status: c_int) {
    #[cfg(coverage)]
    flush_coverage();
    unsafe { libc::_exit(status) }
}
#[cfg(test)]
pub(super) fn exit_process(status: c_int) {
    super::test_support::trace_exit_process(status)
}

#[cfg(not(test))]
pub(super) fn setenv(key: &[u8], value: &[u8]) -> SysResult<()> {
    // libc's own `setenv(3)` already rejects empty keys and keys with
    // `=` with EINVAL, so the pre-check is redundant here.
    let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
    let c_value = crate::bstr::to_cstring(value).map_err(|_| SysError::NulInPath)?;
    let rc = unsafe { libc::setenv(c_key.as_ptr(), c_value.as_ptr(), 1) };
    if rc == 0 { Ok(()) } else { Err(last_error()) }
}
#[cfg(test)]
pub(super) fn setenv(key: &[u8], value: &[u8]) -> SysResult<()> {
    super::test_support::trace_setenv(key, value)
}

#[cfg(not(test))]
#[allow(dead_code)]
pub(super) fn unsetenv(key: &[u8]) -> SysResult<()> {
    // libc's own `unsetenv(3)` rejects empty keys and keys with `=`
    // with EINVAL, so the pre-check is redundant here.
    let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
    let rc = unsafe { libc::unsetenv(c_key.as_ptr()) };
    if rc == 0 { Ok(()) } else { Err(last_error()) }
}
#[cfg(test)]
pub(super) fn unsetenv(key: &[u8]) -> SysResult<()> {
    super::test_support::trace_unsetenv(key)
}

#[cfg(not(test))]
pub(super) fn getenv(key: &[u8]) -> Option<Vec<u8>> {
    let c_key = crate::bstr::to_cstring(key).ok()?;
    let ptr = unsafe { libc::getenv(c_key.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(ptr) }))
    }
}
#[cfg(test)]
pub(super) fn getenv(key: &[u8]) -> Option<Vec<u8>> {
    super::test_support::trace_getenv(key)
}

#[cfg(not(test))]
pub(super) fn get_environ() -> ShellMap<Vec<u8>, Vec<u8>> {
    unsafe extern "C" {
        static environ: *const *const c_char;
    }
    let mut map = ShellMap::default();
    unsafe {
        let mut ptr = environ;
        while !(*ptr).is_null() {
            let entry_cstr = CStr::from_ptr(*ptr);
            let entry = entry_cstr.to_bytes();
            if let Some(eq_pos) = entry.iter().position(|&b| b == b'=') {
                let key = entry[..eq_pos].to_vec();
                let value = entry[eq_pos + 1..].to_vec();
                map.insert(key, value);
            }
            ptr = ptr.add(1);
        }
    }
    map
}
#[cfg(test)]
pub(super) fn get_environ() -> ShellMap<Vec<u8>, Vec<u8>> {
    super::test_support::trace_get_environ()
}

#[cfg(not(test))]
pub(super) fn tcgetattr(fd: c_int, termios_p: *mut libc::termios) -> c_int {
    unsafe { libc::tcgetattr(fd, termios_p) }
}
#[cfg(test)]
pub(super) fn tcgetattr(fd: c_int, termios_p: *mut libc::termios) -> c_int {
    super::test_support::trace_tcgetattr(fd, termios_p)
}

#[cfg(not(test))]
pub(super) fn tcsetattr(fd: c_int, action: c_int, termios_p: *const libc::termios) -> c_int {
    unsafe { libc::tcsetattr(fd, action, termios_p) }
}
#[cfg(test)]
pub(super) fn tcsetattr(fd: c_int, action: c_int, termios_p: *const libc::termios) -> c_int {
    super::test_support::trace_tcsetattr(fd, action, termios_p)
}

#[cfg(not(test))]
pub(super) fn getpwnam(name: &[u8]) -> Option<Vec<u8>> {
    let c_name = crate::bstr::to_cstring(name).ok()?;
    let pw = unsafe { libc::getpwnam(c_name.as_ptr()) };
    if pw.is_null() {
        return None;
    }
    let dir = unsafe { CStr::from_ptr((*pw).pw_dir) };
    Some(crate::bstr::bytes_from_cstr(dir))
}
#[cfg(test)]
pub(super) fn getpwnam(name: &[u8]) -> Option<Vec<u8>> {
    super::test_support::trace_getpwnam(name)
}

#[cfg(not(test))]
pub(super) fn monotonic_clock_ns() -> u64 {
    let mut ts = std::mem::MaybeUninit::<libc::timespec>::zeroed();
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, ts.as_mut_ptr()) };
    let ts = unsafe { ts.assume_init() };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}
#[cfg(test)]
pub(super) fn monotonic_clock_ns() -> u64 {
    super::test_support::trace_monotonic_clock_ns()
}

#[cfg(test)]
mod tests {
    use crate::sys::process::{current_pid, parent_pid};
    use crate::sys::test_support;
    use crate::trace_entries;

    #[test]
    fn trace_getpid_and_getppid_dispatch() {
        test_support::run_trace(
            trace_entries![
                getpid() -> pid(42),
                getppid() -> pid(43),
            ],
            || {
                assert_eq!(current_pid(), 42);
                assert_eq!(parent_pid(), 43);
            },
        );
    }
}
