use libc::{c_char, c_int, c_long, mode_t};
use std::cell::RefCell;
use std::ffi::{CStr, CString};

use super::constants::{EINTR, SIG_ERR_HANDLER};
use super::error::{SysError, SysResult};
use super::interface::{set_errno, signal_mask};
use super::types::{ClockTicks, FileModeMask, Pid};

thread_local! {
    static TEST_ERRNO: RefCell<c_int> = const { RefCell::new(0) };
    static TEST_PENDING_SIGNALS: RefCell<usize> = const { RefCell::new(0) };
    static TEST_PROCESS_IDS: RefCell<Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)>> =
        const { RefCell::new(None) };
    static TEST_LOCALE: RefCell<TestLocale> = const { RefCell::new(TestLocale::C) };
}

#[derive(Clone, Copy, PartialEq)]
enum TestLocale {
    C,
    Utf8,
}

pub(crate) fn set_test_locale_c() {
    TEST_LOCALE.with(|cell| *cell.borrow_mut() = TestLocale::C);
}

pub(crate) fn set_test_locale_utf8() {
    TEST_LOCALE.with(|cell| *cell.borrow_mut() = TestLocale::Utf8);
}

/// Exposed for the `cfg(test)` branches in `sys::locale`: returns `true`
/// when the current thread's synthetic test locale is set to UTF-8.
/// The enum itself stays private; callers only need a boolean predicate.
pub(crate) fn test_locale_is_utf8() -> bool {
    TEST_LOCALE.with(|cell| *cell.borrow() == TestLocale::Utf8)
}

pub(crate) fn current_process_ids() -> Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)>
{
    TEST_PROCESS_IDS.with(|cell| *cell.borrow())
}

pub(crate) fn set_test_errno(errno: c_int) {
    TEST_ERRNO.with(|cell| *cell.borrow_mut() = errno);
}

pub(crate) fn take_test_error() -> SysError {
    let errno = TEST_ERRNO.with(|cell| cell.replace(0));
    SysError::Errno(errno)
}

pub(crate) fn test_pending_signal_bits() -> usize {
    TEST_PENDING_SIGNALS.with(|cell| *cell.borrow())
}

pub(crate) fn test_take_pending_signal_bits() -> usize {
    TEST_PENDING_SIGNALS.with(|cell| cell.replace(0))
}

pub(crate) fn set_pending_signals_for_test(signals: &[c_int]) {
    let bits = signals
        .iter()
        .filter_map(|signal| signal_mask(*signal))
        .fold(0usize, |acc, bit| acc | bit);
    TEST_PENDING_SIGNALS.with(|cell| *cell.borrow_mut() = bits);
}

pub(crate) fn with_pending_signals_for_test<T>(signals: &[c_int], f: impl FnOnce() -> T) -> T {
    let previous = TEST_PENDING_SIGNALS.with(|cell| *cell.borrow());
    set_pending_signals_for_test(signals);
    let result = f();
    TEST_PENDING_SIGNALS.with(|cell| *cell.borrow_mut() = previous);
    result
}

pub(crate) fn with_process_ids_for_test<T>(
    ids: (libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t),
    f: impl FnOnce() -> T,
) -> T {
    TEST_PROCESS_IDS.with(|cell| {
        let previous = cell.replace(Some(ids));
        let result = f();
        cell.replace(previous);
        result
    })
}

// --- Syscall Trace Test Infrastructure ---

#[derive(Clone, Debug)]
pub(crate) enum ArgMatcher {
    Any,
    Int(i64),
    Str(Vec<u8>),
    Bytes(Vec<u8>),
    Fd(c_int),
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum TraceResult {
    Auto,
    Int(i64),
    Fd(c_int),
    Pid(Pid),
    Bytes(Vec<u8>),
    Err(c_int),
    Interrupt(c_int),
    Status(i32),
    StoppedSig(i32),
    SignaledSig(i32),
    ContinuedStatus,
    Fds(c_int, c_int),
    Void,
    CwdBytes(Vec<u8>),
    RealpathBytes(Vec<u8>),
    StatDir,
    StatFile(mode_t),
    StatFileSize(u64),
    StatFifo,
    StatSymlink,
    DirEntryBytes(Vec<u8>),
    StrVal(Vec<u8>),
    NullStr,
    EnvMap(crate::hash::ShellMap<Vec<u8>, Vec<u8>>),
}

#[derive(Clone, Debug)]
pub(crate) struct TraceEntry {
    pub syscall: &'static str,
    pub args: Vec<ArgMatcher>,
    pub result: TraceResult,
    pub child_trace: Option<Vec<TraceEntry>>,
}

thread_local! {
    static TRACE_LOG: RefCell<Option<Vec<TraceEntry>>> = const { RefCell::new(None) };
    static TRACE_INDEX: RefCell<usize> = const { RefCell::new(0) };
    static CHILD_TRACES: RefCell<Vec<Vec<TraceEntry>>> = const { RefCell::new(Vec::new()) };
    static TEST_EXIT_STATUS: RefCell<Option<i32>> = const { RefCell::new(None) };
}

fn trace_dispatch(name: &str, args: &[ArgMatcher]) -> TraceEntry {
    TRACE_LOG.with(|cell| {
        let borrow = cell.borrow();
        let trace = borrow.as_ref().unwrap_or_else(|| panic!("syscall '{name}' called but no trace is active"));
        let index = TRACE_INDEX.with(|idx| {
            let i = *idx.borrow();
            *idx.borrow_mut() = i + 1;
            i
        });
        if index >= trace.len() {
            panic!(
                "unexpected syscall '{name}' at index {index} (trace has {} entries)\n  called with args: {args:?}",
                trace.len()
            );
        }
        let entry = trace[index].clone();
        if entry.syscall != name {
            panic!(
                "trace mismatch at index {index}: expected '{expected}', got '{name}'\n  expected args: {expected_args:?}\n  actual args: {args:?}",
                expected = entry.syscall,
                expected_args = entry.args,
            );
        }
        if name == "write" && entry.args.len() >= 2 {
            let is_stdout_or_stderr = matches!(
                &entry.args[0],
                ArgMatcher::Fd(1) | ArgMatcher::Fd(2)
            );
            if is_stdout_or_stderr && matches!(&entry.args[1], ArgMatcher::Any) {
                panic!(
                    "trace at index {index}: write to stdout/stderr must use ArgMatcher::Bytes, not ArgMatcher::Any",
                );
            }
        }
        for (i, (expected, actual)) in entry.args.iter().zip(args.iter()).enumerate() {
            match (expected, actual) {
                (ArgMatcher::Any, _) => {}
                (ArgMatcher::Int(e), ArgMatcher::Int(a)) if e == a => {}
                (ArgMatcher::Fd(e), ArgMatcher::Fd(a)) if e == a => {}
                (ArgMatcher::Fd(e), ArgMatcher::Int(a)) if *e as i64 == *a => {}
                (ArgMatcher::Int(e), ArgMatcher::Fd(a)) if *e == *a as i64 => {}
                (ArgMatcher::Str(e), ArgMatcher::Str(a)) if e == a => {}
                (ArgMatcher::Bytes(e), ArgMatcher::Bytes(a)) if e == a => {}
                _ => panic!(
                    "trace arg mismatch at index {index}, syscall '{name}', arg {i}: expected {expected:?}, got {actual:?}",
                ),
            }
        }
        if let Some(child_trace) = &entry.child_trace {
            CHILD_TRACES.with(|cell| {
                cell.borrow_mut().push(child_trace.clone());
            });
        }
        entry
    })
}

fn apply_interrupt(signal: c_int) {
    set_pending_signals_for_test(&[signal]);
    set_errno(EINTR);
}

fn apply_trace_result_int(entry: &TraceEntry) -> c_int {
    match &entry.result {
        TraceResult::Auto => panic!(
            "TraceResult::Auto not supported for '{}'; handle it in the caller",
            entry.syscall
        ),
        TraceResult::Int(v) => *v as c_int,
        TraceResult::Fd(fd) => *fd,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        TraceResult::Interrupt(signal) => {
            apply_interrupt(*signal);
            -1
        }
        other => panic!(
            "trace result type mismatch for '{}': expected Int/Fd/Err/Interrupt, got {other:?}",
            entry.syscall
        ),
    }
}

fn apply_trace_result_isize(entry: &TraceEntry) -> isize {
    match &entry.result {
        TraceResult::Auto => panic!(
            "TraceResult::Auto not supported for '{}'; handle it in the caller",
            entry.syscall
        ),
        TraceResult::Int(v) => *v as isize,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        TraceResult::Interrupt(signal) => {
            apply_interrupt(*signal);
            -1
        }
        _ => panic!(
            "trace result type mismatch for '{}': expected Int/Err/Interrupt",
            entry.syscall
        ),
    }
}

fn apply_trace_result_pid(entry: &TraceEntry) -> Pid {
    match &entry.result {
        TraceResult::Pid(p) => *p,
        TraceResult::Int(v) => *v as Pid,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        TraceResult::Interrupt(signal) => {
            apply_interrupt(*signal);
            -1
        }
        other => panic!(
            "trace result type mismatch for '{}': expected Pid/Err/Interrupt, got {other:?}",
            entry.syscall
        ),
    }
}

// Abstract, platform-independent wait-status encoding used only inside unit
// tests. The `#[cfg(test)]` wait-status decoders in `sys::process`
// interpret the tag in the high byte and carry the subordinate code
// (exit status or signal number) in the low 8 bits. This keeps tests
// strictly logical: no unit test needs to know the host libc's actual
// WIFEXITED / WIFCONTINUED bit layout.
pub(super) const WAIT_TAG_MASK: u32 = 0xff00_0000;
pub(super) const WAIT_TAG_EXITED: u32 = 0x0100_0000;
pub(super) const WAIT_TAG_SIGNALED: u32 = 0x0200_0000;
pub(super) const WAIT_TAG_STOPPED: u32 = 0x0300_0000;
pub(super) const WAIT_TAG_CONTINUED: u32 = 0x0400_0000;

/// Build a synthetic wait-status representing normal exit with `code`.
/// Only meaningful together with the fake decoders installed by the test
/// interface; production code never sees these values.
#[allow(dead_code)]
pub(crate) fn encode_exited(code: i32) -> c_int {
    (WAIT_TAG_EXITED | ((code as u32) & 0xff)) as c_int
}
/// Build a synthetic wait-status representing termination by `sig`.
#[allow(dead_code)]
pub(crate) fn encode_signaled(sig: i32) -> c_int {
    (WAIT_TAG_SIGNALED | ((sig as u32) & 0xff)) as c_int
}
#[allow(dead_code)]
pub(crate) fn encode_stopped(sig: i32) -> c_int {
    (WAIT_TAG_STOPPED | ((sig as u32) & 0xff)) as c_int
}
#[allow(dead_code)]
pub(crate) fn encode_continued() -> c_int {
    WAIT_TAG_CONTINUED as c_int
}

// Trace-dispatching syscall implementations
pub(super) fn trace_getpid() -> Pid {
    let entry = trace_dispatch("getpid", &[]);
    apply_trace_result_pid(&entry)
}
pub(super) fn trace_getppid() -> Pid {
    let entry = trace_dispatch("getppid", &[]);
    apply_trace_result_pid(&entry)
}
pub(super) fn trace_waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
    let entry = trace_dispatch(
        "waitpid",
        &[
            ArgMatcher::Int(pid as i64),
            ArgMatcher::Any,
            ArgMatcher::Int(options as i64),
        ],
    );
    if !status.is_null() {
        match entry.result {
            TraceResult::Status(code) => {
                unsafe {
                    *status = encode_exited(code);
                }
                return pid;
            }
            TraceResult::StoppedSig(sig) => {
                unsafe {
                    *status = encode_stopped(sig);
                }
                return pid;
            }
            TraceResult::SignaledSig(sig) => {
                unsafe {
                    *status = encode_signaled(sig);
                }
                return pid;
            }
            TraceResult::ContinuedStatus => {
                unsafe {
                    *status = encode_continued();
                }
                return pid;
            }
            _ => {}
        }
    }
    apply_trace_result_pid(&entry)
}
pub(super) fn trace_kill(pid: Pid, sig: c_int) -> c_int {
    let entry = trace_dispatch(
        "kill",
        &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(sig as i64)],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_signal(sig: c_int, handler: libc::sighandler_t) -> libc::sighandler_t {
    let _ = handler;
    let entry = trace_dispatch("signal", &[ArgMatcher::Int(sig as i64), ArgMatcher::Any]);
    match &entry.result {
        TraceResult::Int(v) => *v as libc::sighandler_t,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            SIG_ERR_HANDLER
        }
        _ => 0 as libc::sighandler_t,
    }
}
pub(super) fn trace_isatty(fd: c_int) -> c_int {
    let entry = trace_dispatch("isatty", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_tcgetpgrp(fd: c_int) -> Pid {
    let entry = trace_dispatch("tcgetpgrp", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_pid(&entry)
}
pub(super) fn trace_tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int {
    let entry = trace_dispatch(
        "tcsetpgrp",
        &[ArgMatcher::Fd(fd), ArgMatcher::Int(pgrp as i64)],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_setpgid(pid: Pid, pgid: Pid) -> c_int {
    let entry = trace_dispatch(
        "setpgid",
        &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(pgid as i64)],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_pipe(fds: &mut [c_int; 2]) -> c_int {
    let entry = trace_dispatch("pipe", &[]);
    match &entry.result {
        TraceResult::Fds(r, w) => {
            fds[0] = *r;
            fds[1] = *w;
            0
        }
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        other => {
            panic!("trace result type mismatch for 'pipe': expected Fds/Err, got {other:?}")
        }
    }
}
pub(super) fn trace_dup2(oldfd: c_int, newfd: c_int) -> c_int {
    let entry = trace_dispatch("dup2", &[ArgMatcher::Fd(oldfd), ArgMatcher::Fd(newfd)]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_close(fd: c_int) -> c_int {
    let entry = trace_dispatch("close", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int {
    let entry = trace_dispatch(
        "fcntl",
        &[
            ArgMatcher::Fd(fd),
            ArgMatcher::Int(cmd as i64),
            ArgMatcher::Int(arg as i64),
        ],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_read(fd: c_int, buf: &mut [u8]) -> isize {
    let entry = trace_dispatch("read", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
    match &entry.result {
        TraceResult::Bytes(data) => {
            let n = data.len().min(buf.len());
            buf[..n].copy_from_slice(&data[..n]);
            n as isize
        }
        TraceResult::Int(v) => *v as isize,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        TraceResult::Interrupt(signal) => {
            apply_interrupt(*signal);
            -1
        }
        other => panic!(
            "trace result type mismatch for 'read': expected Bytes/Int/Err/Interrupt, got {other:?}"
        ),
    }
}
pub(super) fn trace_umask(cmask: FileModeMask) -> FileModeMask {
    let entry = trace_dispatch("umask", &[ArgMatcher::Int(cmask as i64)]);
    match &entry.result {
        TraceResult::Int(v) => *v as FileModeMask,
        other => panic!("trace result type mismatch for 'umask': expected Int, got {other:?}"),
    }
}
pub(super) fn trace_times(_buffer: *mut libc::tms) -> ClockTicks {
    let entry = trace_dispatch("times", &[ArgMatcher::Any]);
    match &entry.result {
        TraceResult::Int(v) => *v as ClockTicks,
        TraceResult::Err(_) => ClockTicks::MAX,
        other => {
            panic!("trace result type mismatch for 'times': expected Int/Err, got {other:?}")
        }
    }
}
pub(super) fn trace_monotonic_clock_ns() -> u64 {
    let entry = trace_dispatch("monotonic_clock_ns", &[]);
    match &entry.result {
        TraceResult::Int(ns) => *ns as u64,
        other => {
            panic!(
                "trace result type mismatch for 'monotonic_clock_ns': expected Int, got {other:?}"
            )
        }
    }
}
pub(super) fn trace_sysconf(name: c_int) -> c_long {
    let entry = trace_dispatch("sysconf", &[ArgMatcher::Int(name as i64)]);
    match &entry.result {
        TraceResult::Int(v) => *v as c_long,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        other => {
            panic!("trace result type mismatch for 'sysconf': expected Int/Err, got {other:?}")
        }
    }
}
pub(super) fn trace_execvp(file: *const c_char, _argv: *const *const c_char) -> c_int {
    let name = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(file) });
    let entry = trace_dispatch("execvp", &[ArgMatcher::Str(name), ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_execve(
    file: *const c_char,
    _argv: *const *const c_char,
    _envp: *const *const c_char,
) -> c_int {
    let name = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(file) });
    let entry = trace_dispatch("execve", &[ArgMatcher::Str(name), ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch(
        "open",
        &[
            ArgMatcher::Str(p),
            ArgMatcher::Int(flags as i64),
            ArgMatcher::Int(mode as i64),
        ],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_write(fd: c_int, data: &[u8]) -> isize {
    let entry = trace_dispatch(
        "write",
        &[ArgMatcher::Fd(fd), ArgMatcher::Bytes(data.to_vec())],
    );
    match &entry.result {
        TraceResult::Auto => data.len() as isize,
        _ => apply_trace_result_isize(&entry),
    }
}
fn fill_stat_buf(result: &TraceResult, buf: *mut libc::stat) -> Option<c_int> {
    match result {
        TraceResult::StatDir => {
            unsafe {
                std::ptr::write_bytes(buf, 0, 1);
                (*buf).st_mode = libc::S_IFDIR | 0o755;
            }
            Some(0)
        }
        TraceResult::StatFile(mode) => {
            unsafe {
                std::ptr::write_bytes(buf, 0, 1);
                (*buf).st_mode = libc::S_IFREG | *mode;
            }
            Some(0)
        }
        TraceResult::StatFileSize(size) => {
            unsafe {
                std::ptr::write_bytes(buf, 0, 1);
                (*buf).st_mode = libc::S_IFREG | 0o644;
                (*buf).st_size = *size as i64;
            }
            Some(0)
        }
        TraceResult::StatFifo => {
            unsafe {
                std::ptr::write_bytes(buf, 0, 1);
                (*buf).st_mode = libc::S_IFIFO | 0o644;
            }
            Some(0)
        }
        TraceResult::StatSymlink => {
            unsafe {
                std::ptr::write_bytes(buf, 0, 1);
                (*buf).st_mode = libc::S_IFLNK | 0o777;
            }
            Some(0)
        }
        TraceResult::Err(errno) => {
            set_errno(*errno);
            Some(-1)
        }
        TraceResult::Int(v) => Some(*v as c_int),
        _ => None,
    }
}

pub(super) fn trace_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("stat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!(
            "trace result type mismatch for 'stat': expected StatDir/StatFile/Err, got {:?}",
            entry.result
        )
    })
}

pub(super) fn trace_lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("lstat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!("trace result type mismatch for 'lstat': expected StatDir/StatFile/StatSymlink/Err, got {:?}", entry.result)
    })
}

pub(super) fn trace_fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
    let entry = trace_dispatch("fstat", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!(
            "trace result type mismatch for 'fstat': expected StatDir/StatFile/Err, got {:?}",
            entry.result
        )
    })
}

pub(super) fn trace_access(path: *const c_char, mode: c_int) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch(
        "access",
        &[ArgMatcher::Str(p), ArgMatcher::Int(mode as i64)],
    );
    apply_trace_result_int(&entry)
}
pub(super) fn trace_chdir(path: *const c_char) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("chdir", &[ArgMatcher::Str(p)]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    let entry = trace_dispatch("getcwd", &[]);
    match &entry.result {
        TraceResult::CwdBytes(s) => {
            if s.len() + 1 > size {
                set_errno(libc::ERANGE);
                return std::ptr::null_mut();
            }
            unsafe {
                std::ptr::copy_nonoverlapping(s.as_ptr(), buf as *mut u8, s.len());
                *buf.add(s.len()) = 0;
            }
            buf
        }
        TraceResult::Err(errno) => {
            set_errno(*errno);
            std::ptr::null_mut()
        }
        other => {
            panic!("trace result type mismatch for 'getcwd': expected CwdBytes/Err, got {other:?}")
        }
    }
}
pub(super) fn trace_opendir(path: *const c_char) -> *mut libc::DIR {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("opendir", &[ArgMatcher::Str(p)]);
    match &entry.result {
        TraceResult::Int(v) => *v as *mut libc::DIR,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            std::ptr::null_mut()
        }
        other => {
            panic!("trace result type mismatch for 'opendir': expected Int/Err, got {other:?}")
        }
    }
}
thread_local! {
    static FAKE_DIRENT: std::cell::RefCell<libc::dirent> = const { std::cell::RefCell::new(unsafe { std::mem::zeroed() }) };
}
pub(super) fn trace_readdir(_dirp: *mut libc::DIR) -> *mut libc::dirent {
    let entry = trace_dispatch("readdir", &[ArgMatcher::Any]);
    match &entry.result {
        TraceResult::DirEntryBytes(name) => FAKE_DIRENT.with(|cell| {
            let mut d = cell.borrow_mut();
            d.d_name = unsafe { std::mem::zeroed() };
            let len = name.len().min(d.d_name.len() - 1);
            for (i, &b) in name[..len].iter().enumerate() {
                d.d_name[i] = b as i8;
            }
            d.d_name[len] = 0;
            &mut *d as *mut libc::dirent
        }),
        TraceResult::Int(0) => {
            set_errno(0);
            std::ptr::null_mut()
        }
        TraceResult::Err(errno) => {
            set_errno(*errno);
            std::ptr::null_mut()
        }
        other => panic!(
            "trace result type mismatch for 'readdir': expected DirEntryBytes/Int(0)/Err, got {other:?}"
        ),
    }
}
pub(super) fn trace_closedir(_dirp: *mut libc::DIR) -> c_int {
    let entry = trace_dispatch("closedir", &[ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_unlink(path: *const c_char) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("unlink", &[ArgMatcher::Str(p)]);
    match &entry.result {
        TraceResult::Int(v) => *v as c_int,
        TraceResult::Err(errno) => {
            set_errno(*errno);
            -1
        }
        other => panic!(
            "trace result type mismatch for 'unlink': expected Int/Err, got {:?}",
            other
        ),
    }
}

pub(super) fn trace_realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("realpath", &[ArgMatcher::Str(p), ArgMatcher::Any]);
    match &entry.result {
        TraceResult::RealpathBytes(s) => {
            let c_result = CString::new(s.clone()).unwrap();
            if resolved.is_null() {
                let ptr =
                    unsafe { libc::malloc(c_result.as_bytes_with_nul().len()) } as *mut c_char;
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        c_result.as_ptr(),
                        ptr,
                        c_result.as_bytes_with_nul().len(),
                    );
                }
                ptr
            } else {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        c_result.as_ptr(),
                        resolved,
                        c_result.as_bytes_with_nul().len(),
                    );
                }
                resolved
            }
        }
        TraceResult::Err(errno) => {
            set_errno(*errno);
            std::ptr::null_mut()
        }
        other => panic!(
            "trace result type mismatch for 'realpath': expected RealpathBytes/Err, got {other:?}"
        ),
    }
}
pub(super) fn trace_fork() -> Pid {
    let entry = trace_dispatch("fork", &[]);
    apply_trace_result_pid(&entry)
}
pub(super) fn trace_exit_process(status: c_int) {
    let _entry = trace_dispatch("exit_process", &[ArgMatcher::Int(status as i64)]);
    TEST_EXIT_STATUS.with(|cell| cell.replace(Some(status)));
    std::panic::panic_any(ChildExitPanic(status));
}

pub(super) fn trace_setenv(key: &[u8], value: &[u8]) -> SysResult<()> {
    let entry = trace_dispatch(
        "setenv",
        &[
            ArgMatcher::Str(key.to_vec()),
            ArgMatcher::Str(value.to_vec()),
        ],
    );
    match entry.result {
        TraceResult::Int(0) => Ok(()),
        TraceResult::Err(errno) => Err(SysError::Errno(errno)),
        other => panic!("setenv trace: unexpected result {other:?}"),
    }
}

pub(super) fn trace_unsetenv(key: &[u8]) -> SysResult<()> {
    let entry = trace_dispatch("unsetenv", &[ArgMatcher::Str(key.to_vec())]);
    match entry.result {
        TraceResult::Int(0) => Ok(()),
        TraceResult::Err(errno) => Err(SysError::Errno(errno)),
        other => panic!("unsetenv trace: unexpected result {other:?}"),
    }
}

pub(super) fn trace_getenv(key: &[u8]) -> Option<Vec<u8>> {
    let entry = trace_dispatch("getenv", &[ArgMatcher::Str(key.to_vec())]);
    match entry.result {
        TraceResult::StrVal(s) => Some(s),
        TraceResult::NullStr => None,
        other => panic!("getenv trace: unexpected result {other:?}"),
    }
}

pub(super) fn trace_get_environ() -> crate::hash::ShellMap<Vec<u8>, Vec<u8>> {
    let entry = trace_dispatch("get_environ", &[]);
    match entry.result {
        TraceResult::EnvMap(map) => map,
        other => panic!("get_environ trace: unexpected result {other:?}"),
    }
}

pub(super) fn trace_getpwnam(name: &[u8]) -> Option<Vec<u8>> {
    let entry = trace_dispatch("getpwnam", &[ArgMatcher::Str(name.to_vec())]);
    match entry.result {
        TraceResult::StrVal(s) => Some(s),
        TraceResult::NullStr => None,
        other => panic!("getpwnam trace: unexpected result {other:?}"),
    }
}
pub(super) fn trace_tcgetattr(_fd: c_int, _termios_p: *mut libc::termios) -> c_int {
    let entry = trace_dispatch("tcgetattr", &[ArgMatcher::Fd(_fd)]);
    apply_trace_result_int(&entry)
}
pub(super) fn trace_tcsetattr(
    _fd: c_int,
    _action: c_int,
    _termios_p: *const libc::termios,
) -> c_int {
    let entry = trace_dispatch(
        "tcsetattr",
        &[ArgMatcher::Fd(_fd), ArgMatcher::Int(_action as i64)],
    );
    apply_trace_result_int(&entry)
}

#[allow(dead_code)]
pub(crate) struct ChildExitPanic(pub i32);

/// Run `f` with an empty trace installed: any syscall through
/// `super::interface::*` will consult the empty `TRACE_LOG` and panic via
/// `trace_dispatch`, catching pure-logic tests that accidentally escape
/// into `sys::*`.
pub(crate) fn assert_no_syscalls<T>(f: impl FnOnce() -> T) -> T {
    TRACE_LOG.with(|cell| {
        let prev_trace = cell.replace(Some(Vec::new()));
        let prev_index = TRACE_INDEX.with(|idx| idx.replace(0));
        let result = f();
        TRACE_INDEX.with(|idx| idx.replace(prev_index));
        cell.replace(prev_trace);
        result
    })
}

fn validate_fork_child_traces(trace: &[TraceEntry]) {
    for entry in trace {
        if entry.syscall == "fork" {
            if let TraceResult::Pid(pid) = &entry.result {
                if *pid > 0 && entry.child_trace.is_none() {
                    panic!(
                        "fork trace entry returns Pid({pid}) (parent path) but has no child_trace — \
                         use t_fork(TraceResult::Pid({pid}), vec![...]) to provide the child trace"
                    );
                }
            }
            if let Some(child) = &entry.child_trace {
                validate_fork_child_traces(child);
            }
        }
    }
}

pub(crate) fn run_trace(trace: Vec<TraceEntry>, f: impl Fn()) {
    validate_fork_child_traces(&trace);
    let paths = enumerate_fork_paths(&trace);

    for (run_index, path) in paths.iter().enumerate() {
        let is_parent = run_index == 0;

        TRACE_LOG.with(|cell| {
            let prev_trace = cell.replace(Some(path.clone()));
            let prev_index = TRACE_INDEX.with(|idx| idx.replace(0));
            let prev_children = CHILD_TRACES.with(|c| std::mem::take(&mut *c.borrow_mut()));

            let result = std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(&f)
            );

            let consumed = TRACE_INDEX.with(|idx| *idx.borrow());
            let total = cell.borrow().as_ref().map_or(0, |t| t.len());

            CHILD_TRACES.with(|c| *c.borrow_mut() = prev_children);
            TRACE_INDEX.with(|idx| idx.replace(prev_index));
            cell.replace(prev_trace);

            match result {
                Ok(_) => {
                    if !is_parent {
                        panic!(
                            "fork child run {run_index} returned normally (expected ChildExitPanic)"
                        );
                    }
                    if consumed < total {
                        panic!(
                            "trace not fully consumed in parent run: {consumed}/{total} entries used"
                        );
                    }
                }
                Err(payload) => {
                    if payload.downcast_ref::<ChildExitPanic>().is_some() {
                        if is_parent {
                            panic!(
                                "parent run (run 0) got ChildExitPanic — exit_process called in parent path"
                            );
                        }
                        if consumed < total {
                            panic!(
                                "child trace not fully consumed in run {run_index}: {consumed}/{total} entries used"
                            );
                        }
                    } else {
                        std::panic::resume_unwind(payload);
                    }
                }
            }
        });
    }
}

#[allow(dead_code)]
pub(crate) fn take_child_traces() -> Vec<Vec<TraceEntry>> {
    CHILD_TRACES.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

impl TraceEntry {
    fn without_child_trace(&self) -> TraceEntry {
        TraceEntry {
            syscall: self.syscall,
            args: self.args.clone(),
            result: self.result.clone(),
            child_trace: None,
        }
    }

    fn with_result(&self, result: TraceResult) -> TraceEntry {
        TraceEntry {
            syscall: self.syscall,
            args: self.args.clone(),
            result,
            child_trace: self.child_trace.clone(),
        }
    }
}

fn exit_process_entry() -> TraceEntry {
    TraceEntry {
        syscall: "exit_process",
        args: vec![ArgMatcher::Any],
        result: TraceResult::Void,
        child_trace: None,
    }
}

fn enumerate_fork_paths(trace: &[TraceEntry]) -> Vec<Vec<TraceEntry>> {
    let mut paths = vec![];

    let parent: Vec<TraceEntry> = trace.iter().map(|e| e.without_child_trace()).collect();
    paths.push(parent);

    for (i, entry) in trace.iter().enumerate() {
        if let Some(child_trace) = &entry.child_trace {
            let prefix: Vec<TraceEntry> =
                trace[..i].iter().map(|e| e.without_child_trace()).collect();
            let fork_as_child = entry.with_result(TraceResult::Pid(0)).without_child_trace();

            let mut child_path = prefix.clone();
            child_path.push(fork_as_child.clone());
            child_path.extend(child_trace.iter().map(|e| e.without_child_trace()));
            child_path.push(exit_process_entry());
            paths.push(child_path);

            let nested = enumerate_fork_paths(child_trace);
            for nested_path in nested.into_iter().skip(1) {
                let mut full = prefix.clone();
                full.push(fork_as_child.clone());
                full.extend(nested_path);
                paths.push(full);
            }
        }
    }

    paths
}

// Helper constructors for trace entries
pub(crate) fn t(syscall: &'static str, args: Vec<ArgMatcher>, result: TraceResult) -> TraceEntry {
    TraceEntry {
        syscall,
        args,
        result,
        child_trace: None,
    }
}

pub(crate) fn t_fork(result: TraceResult, child: Vec<TraceEntry>) -> TraceEntry {
    TraceEntry {
        syscall: "fork",
        args: vec![],
        result,
        child_trace: Some(child),
    }
}

/// Normalize byte data for trace `ArgMatcher::Bytes` / `DirEntryBytes` (macro helpers).
#[allow(dead_code)]
pub(crate) fn trace_bytes_from_ref<B: AsRef<[u8]> + ?Sized>(b: &B) -> Vec<u8> {
    b.as_ref().to_vec()
}

/// Normalize byte data for trace `ArgMatcher::Str` (macro `str(...)` args).
#[allow(dead_code)]
pub(crate) fn trace_str_from_ref<B: AsRef<[u8]> + ?Sized>(b: &B) -> Vec<u8> {
    b.as_ref().to_vec()
}

/// `read(STDIN_FILENO, _)` returning each chunk, then EOF (`Int(0)`).
#[allow(dead_code)]
pub(crate) fn stdin_chunks<I, B>(chunks: I) -> Vec<TraceEntry>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    use crate::sys::constants::STDIN_FILENO;
    let mut out: Vec<TraceEntry> = chunks
        .into_iter()
        .map(|chunk| {
            t(
                "read",
                vec![ArgMatcher::Fd(STDIN_FILENO), ArgMatcher::Any],
                TraceResult::Bytes(chunk.as_ref().to_vec()),
            )
        })
        .collect();
    out.push(t(
        "read",
        vec![ArgMatcher::Fd(STDIN_FILENO), ArgMatcher::Any],
        TraceResult::Int(0),
    ));
    out
}

/// One stdin `read` with payload `data`, then EOF.
#[allow(dead_code)]
pub(crate) fn stdin_bytes<B: AsRef<[u8]> + ?Sized>(data: &B) -> Vec<TraceEntry> {
    stdin_chunks(std::iter::once(data))
}

/// `times` identical stdin reads of `chunk`, then EOF.
#[allow(dead_code)]
pub(crate) fn stdin_repeat<B: AsRef<[u8]> + Clone>(chunk: B, times: usize) -> Vec<TraceEntry> {
    stdin_chunks((0..times).map(|_| chunk.clone()))
}

#[allow(dead_code)]
pub(crate) trait IntoArgMatcher {
    fn into_arg(self) -> ArgMatcher;
}
impl IntoArgMatcher for i32 {
    fn into_arg(self) -> ArgMatcher {
        ArgMatcher::Int(self as i64)
    }
}
impl IntoArgMatcher for i64 {
    fn into_arg(self) -> ArgMatcher {
        ArgMatcher::Int(self)
    }
}
impl IntoArgMatcher for &str {
    fn into_arg(self) -> ArgMatcher {
        ArgMatcher::Str(self.as_bytes().to_vec())
    }
}
impl IntoArgMatcher for &[u8] {
    fn into_arg(self) -> ArgMatcher {
        ArgMatcher::Bytes(self.to_vec())
    }
}
impl IntoArgMatcher for &Vec<u8> {
    fn into_arg(self) -> ArgMatcher {
        ArgMatcher::Bytes(self.clone())
    }
}

#[allow(dead_code)]
pub(crate) fn arg_from(v: impl IntoArgMatcher) -> ArgMatcher {
    v.into_arg()
}
