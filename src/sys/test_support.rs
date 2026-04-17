use libc::{c_char, c_int, c_long, mode_t};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::Mutex;

use super::constants::{EINTR, SIG_ERR_HANDLER};
use super::error::{SysError, SysResult};
use super::interface::{SystemInterface, set_errno, signal_mask};
use super::types::{ClockTicks, FileModeMask, Pid};

thread_local! {
    static TEST_INTERFACE: RefCell<Option<SystemInterface>> = const { RefCell::new(None) };
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

fn test_locale() -> TestLocale {
    TEST_LOCALE.with(|cell| *cell.borrow())
}

fn syscall_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::new(());
    &LOCK
}

pub(super) fn current_interface() -> Option<SystemInterface> {
    TEST_INTERFACE.with(|cell| *cell.borrow())
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

pub(super) fn with_test_interface<T>(iface: SystemInterface, f: impl FnOnce() -> T) -> T {
    let _guard = syscall_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    TEST_INTERFACE.with(|cell| {
        let previous = cell.replace(Some(iface));
        let result = f();
        cell.replace(previous);
        result
    })
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
    EnvMap(HashMap<Vec<u8>, Vec<u8>>),
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

// Trace-dispatching syscall implementations
fn trace_getpid() -> Pid {
    let entry = trace_dispatch("getpid", &[]);
    apply_trace_result_pid(&entry)
}
fn trace_getppid() -> Pid {
    let entry = trace_dispatch("getppid", &[]);
    apply_trace_result_pid(&entry)
}
fn trace_waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
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
            TraceResult::Status(s) => {
                unsafe {
                    *status = s << 8;
                }
                return pid;
            }
            TraceResult::StoppedSig(sig) => {
                unsafe {
                    *status = (sig << 8) | 0x7f;
                }
                return pid;
            }
            TraceResult::SignaledSig(sig) => {
                unsafe {
                    *status = sig & 0x7f;
                }
                return pid;
            }
            TraceResult::ContinuedStatus => {
                unsafe {
                    *status = 0xffff;
                }
                return pid;
            }
            _ => {}
        }
    }
    apply_trace_result_pid(&entry)
}
fn trace_kill(pid: Pid, sig: c_int) -> c_int {
    let entry = trace_dispatch(
        "kill",
        &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(sig as i64)],
    );
    apply_trace_result_int(&entry)
}
fn trace_signal(sig: c_int, handler: libc::sighandler_t) -> libc::sighandler_t {
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
fn trace_isatty(fd: c_int) -> c_int {
    let entry = trace_dispatch("isatty", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_int(&entry)
}
fn trace_tcgetpgrp(fd: c_int) -> Pid {
    let entry = trace_dispatch("tcgetpgrp", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_pid(&entry)
}
fn trace_tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int {
    let entry = trace_dispatch(
        "tcsetpgrp",
        &[ArgMatcher::Fd(fd), ArgMatcher::Int(pgrp as i64)],
    );
    apply_trace_result_int(&entry)
}
fn trace_setpgid(pid: Pid, pgid: Pid) -> c_int {
    let entry = trace_dispatch(
        "setpgid",
        &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(pgid as i64)],
    );
    apply_trace_result_int(&entry)
}
fn trace_pipe(fds: &mut [c_int; 2]) -> c_int {
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
fn trace_dup2(oldfd: c_int, newfd: c_int) -> c_int {
    let entry = trace_dispatch("dup2", &[ArgMatcher::Fd(oldfd), ArgMatcher::Fd(newfd)]);
    apply_trace_result_int(&entry)
}
fn trace_close(fd: c_int) -> c_int {
    let entry = trace_dispatch("close", &[ArgMatcher::Fd(fd)]);
    apply_trace_result_int(&entry)
}
fn trace_fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int {
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
fn trace_read(fd: c_int, buf: &mut [u8]) -> isize {
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
fn trace_umask(cmask: FileModeMask) -> FileModeMask {
    let entry = trace_dispatch("umask", &[ArgMatcher::Int(cmask as i64)]);
    match &entry.result {
        TraceResult::Int(v) => *v as FileModeMask,
        other => panic!("trace result type mismatch for 'umask': expected Int, got {other:?}"),
    }
}
fn trace_times(_buffer: *mut libc::tms) -> ClockTicks {
    let entry = trace_dispatch("times", &[ArgMatcher::Any]);
    match &entry.result {
        TraceResult::Int(v) => *v as ClockTicks,
        TraceResult::Err(_) => ClockTicks::MAX,
        other => {
            panic!("trace result type mismatch for 'times': expected Int/Err, got {other:?}")
        }
    }
}
fn trace_monotonic_clock_ns() -> u64 {
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
fn test_setup_locale() {
    set_test_locale_c();
}
fn test_reinit_locale() {
    let val = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    let upper = val.to_ascii_uppercase();
    if upper.contains("UTF-8") || upper.contains("UTF8") {
        set_test_locale_utf8();
    } else {
        set_test_locale_c();
    }
}
fn test_classify_byte(class: &[u8], byte: u8) -> bool {
    test_classify_char(class, byte as u32)
}
fn test_classify_char(class: &[u8], wc: u32) -> bool {
    if wc <= 0x7f {
        let byte = wc as u8;
        return byte.is_ascii_alphabetic() && class == b"alpha"
            || byte.is_ascii_alphanumeric() && class == b"alnum"
            || byte.is_ascii_digit() && class == b"digit"
            || byte.is_ascii_lowercase() && class == b"lower"
            || byte.is_ascii_uppercase() && class == b"upper"
            || (byte == b' ' || byte == b'\t') && class == b"blank"
            || byte.is_ascii_whitespace() && class == b"space"
            || byte.is_ascii_hexdigit() && class == b"xdigit"
            || byte.is_ascii_punctuation() && class == b"punct"
            || byte.is_ascii_graphic() && class == b"graph"
            || (byte.is_ascii_graphic() || byte == b' ') && class == b"print"
            || byte.is_ascii_control() && class == b"cntrl";
    }
    if test_locale() == TestLocale::C {
        return false;
    }
    if let Some(ch) = char::from_u32(wc) {
        match class {
            b"alpha" => ch.is_alphabetic(),
            b"alnum" => ch.is_alphanumeric(),
            b"digit" => ch.is_ascii_digit(),
            b"lower" => ch.is_lowercase(),
            b"upper" => ch.is_uppercase(),
            b"blank" => ch == ' ' || ch == '\t',
            b"space" => ch.is_whitespace(),
            b"xdigit" => ch.is_ascii_hexdigit(),
            b"punct" => !ch.is_alphanumeric() && !ch.is_whitespace() && !ch.is_control(),
            b"graph" => !ch.is_whitespace() && !ch.is_control(),
            b"print" => !ch.is_control(),
            b"cntrl" => ch.is_control(),
            _ => false,
        }
    } else {
        false
    }
}
fn test_decode_char(bytes: &[u8]) -> (u32, usize) {
    if bytes.is_empty() {
        return (0, 0);
    }
    if test_locale() == TestLocale::C {
        return (bytes[0] as u32, 1);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            if let Some(ch) = s.chars().next() {
                (ch as u32, ch.len_utf8())
            } else {
                (0, 0)
            }
        }
        Err(e) => {
            let valid_up_to = e.valid_up_to();
            if valid_up_to > 0 {
                let s = &bytes[..valid_up_to];
                let ch = std::str::from_utf8(s).unwrap().chars().next().unwrap();
                (ch as u32, ch.len_utf8())
            } else {
                (bytes[0] as u32, 1)
            }
        }
    }
}
fn test_encode_char(wc: u32, buf: &mut [u8]) -> usize {
    if test_locale() == TestLocale::C {
        if wc <= 0x7f {
            buf[0] = wc as u8;
            return 1;
        }
        return 0;
    }
    if let Some(ch) = char::from_u32(wc) {
        let mut tmp = [0u8; 4];
        let s = ch.encode_utf8(&mut tmp);
        let n = s.len();
        buf[..n].copy_from_slice(&tmp[..n]);
        n
    } else {
        0
    }
}
fn test_mb_cur_max() -> usize {
    if test_locale() == TestLocale::C { 1 } else { 4 }
}
fn test_to_upper(wc: u32) -> u32 {
    if test_locale() == TestLocale::C {
        if wc >= b'a' as u32 && wc <= b'z' as u32 {
            return wc - 32;
        }
        return wc;
    }
    char::from_u32(wc)
        .map(|c| {
            let mut it = c.to_uppercase();
            it.next().unwrap_or(c) as u32
        })
        .unwrap_or(wc)
}
fn test_to_lower(wc: u32) -> u32 {
    if test_locale() == TestLocale::C {
        if wc >= b'A' as u32 && wc <= b'Z' as u32 {
            return wc + 32;
        }
        return wc;
    }
    char::from_u32(wc)
        .map(|c| {
            let mut it = c.to_lowercase();
            it.next().unwrap_or(c) as u32
        })
        .unwrap_or(wc)
}
fn test_char_width(wc: u32) -> usize {
    if test_locale() == TestLocale::C {
        if wc < 0x20 || wc == 0x7f { 0 } else { 1 }
    } else if let Some(ch) = char::from_u32(wc) {
        if ch.is_control() { 0 } else { 1 }
    } else {
        0
    }
}
fn test_strcoll(a: &[u8], b: &[u8]) -> std::cmp::Ordering {
    a.cmp(b)
}
fn test_decimal_point() -> u8 {
    b'.'
}
fn trace_sysconf(name: c_int) -> c_long {
    let entry = trace_dispatch("sysconf", &[ArgMatcher::Int(name as i64)]);
    match &entry.result {
        TraceResult::Int(v) => *v as c_long,
        other => {
            panic!("trace result type mismatch for 'sysconf': expected Int, got {other:?}")
        }
    }
}
fn trace_execvp(file: *const c_char, _argv: *const *const c_char) -> c_int {
    let name = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(file) });
    let entry = trace_dispatch("execvp", &[ArgMatcher::Str(name), ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
fn trace_execve(
    file: *const c_char,
    _argv: *const *const c_char,
    _envp: *const *const c_char,
) -> c_int {
    let name = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(file) });
    let entry = trace_dispatch("execve", &[ArgMatcher::Str(name), ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
fn trace_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
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
fn trace_write(fd: c_int, data: &[u8]) -> isize {
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

fn trace_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("stat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!(
            "trace result type mismatch for 'stat': expected StatDir/StatFile/Err, got {:?}",
            entry.result
        )
    })
}

fn trace_lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("lstat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!("trace result type mismatch for 'lstat': expected StatDir/StatFile/StatSymlink/Err, got {:?}", entry.result)
    })
}

fn trace_fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
    let entry = trace_dispatch("fstat", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
    fill_stat_buf(&entry.result, buf).unwrap_or_else(|| {
        panic!(
            "trace result type mismatch for 'fstat': expected StatDir/StatFile/Err, got {:?}",
            entry.result
        )
    })
}

fn trace_access(path: *const c_char, mode: c_int) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch(
        "access",
        &[ArgMatcher::Str(p), ArgMatcher::Int(mode as i64)],
    );
    apply_trace_result_int(&entry)
}
fn trace_chdir(path: *const c_char) -> c_int {
    let p = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(path) });
    let entry = trace_dispatch("chdir", &[ArgMatcher::Str(p)]);
    apply_trace_result_int(&entry)
}
fn trace_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
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
fn trace_opendir(path: *const c_char) -> *mut libc::DIR {
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
fn trace_readdir(_dirp: *mut libc::DIR) -> *mut libc::dirent {
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
fn trace_closedir(_dirp: *mut libc::DIR) -> c_int {
    let entry = trace_dispatch("closedir", &[ArgMatcher::Any]);
    apply_trace_result_int(&entry)
}
fn trace_unlink(path: *const c_char) -> c_int {
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

fn trace_realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
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
fn trace_fork() -> Pid {
    let entry = trace_dispatch("fork", &[]);
    apply_trace_result_pid(&entry)
}
fn trace_exit_process(status: c_int) {
    let _entry = trace_dispatch("exit_process", &[ArgMatcher::Int(status as i64)]);
    TEST_EXIT_STATUS.with(|cell| cell.replace(Some(status)));
    std::panic::panic_any(ChildExitPanic(status));
}

fn trace_setenv(key: &[u8], value: &[u8]) -> SysResult<()> {
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

fn trace_unsetenv(key: &[u8]) -> SysResult<()> {
    let entry = trace_dispatch("unsetenv", &[ArgMatcher::Str(key.to_vec())]);
    match entry.result {
        TraceResult::Int(0) => Ok(()),
        TraceResult::Err(errno) => Err(SysError::Errno(errno)),
        other => panic!("unsetenv trace: unexpected result {other:?}"),
    }
}

fn trace_getenv(key: &[u8]) -> Option<Vec<u8>> {
    let entry = trace_dispatch("getenv", &[ArgMatcher::Str(key.to_vec())]);
    match entry.result {
        TraceResult::StrVal(s) => Some(s),
        TraceResult::NullStr => None,
        other => panic!("getenv trace: unexpected result {other:?}"),
    }
}

fn trace_get_environ() -> HashMap<Vec<u8>, Vec<u8>> {
    let entry = trace_dispatch("get_environ", &[]);
    match entry.result {
        TraceResult::EnvMap(map) => map,
        other => panic!("get_environ trace: unexpected result {other:?}"),
    }
}

fn trace_getpwnam(name: &[u8]) -> Option<Vec<u8>> {
    let entry = trace_dispatch("getpwnam", &[ArgMatcher::Str(name.to_vec())]);
    match entry.result {
        TraceResult::StrVal(s) => Some(s),
        TraceResult::NullStr => None,
        other => panic!("getpwnam trace: unexpected result {other:?}"),
    }
}
fn trace_tcgetattr(_fd: c_int, _termios_p: *mut libc::termios) -> c_int {
    let entry = trace_dispatch("tcgetattr", &[ArgMatcher::Fd(_fd)]);
    apply_trace_result_int(&entry)
}
fn trace_tcsetattr(_fd: c_int, _action: c_int, _termios_p: *const libc::termios) -> c_int {
    let entry = trace_dispatch(
        "tcsetattr",
        &[ArgMatcher::Fd(_fd), ArgMatcher::Int(_action as i64)],
    );
    apply_trace_result_int(&entry)
}

#[allow(dead_code)]
pub(crate) struct ChildExitPanic(pub i32);

pub(super) fn trace_interface() -> SystemInterface {
    SystemInterface {
        getpid: trace_getpid,
        getppid: trace_getppid,
        waitpid: trace_waitpid,
        kill: trace_kill,
        signal: trace_signal,
        isatty: trace_isatty,
        tcgetpgrp: trace_tcgetpgrp,
        tcsetpgrp: trace_tcsetpgrp,
        setpgid: trace_setpgid,
        pipe: trace_pipe,
        dup2: trace_dup2,
        close: trace_close,
        fcntl: trace_fcntl,
        read: trace_read,
        umask: trace_umask,
        times: trace_times,
        sysconf: trace_sysconf,
        execvp: trace_execvp,
        execve: trace_execve,
        open: trace_open,
        write: trace_write,
        stat: trace_stat,
        lstat: trace_lstat,
        fstat: trace_fstat,
        access: trace_access,
        chdir: trace_chdir,
        getcwd: trace_getcwd,
        opendir: trace_opendir,
        readdir: trace_readdir,
        closedir: trace_closedir,
        realpath: trace_realpath,
        unlink: trace_unlink,
        fork: trace_fork,
        exit_process: trace_exit_process,
        setenv: trace_setenv,
        unsetenv: trace_unsetenv,
        getenv: trace_getenv,
        get_environ: trace_get_environ,
        tcgetattr: trace_tcgetattr,
        tcsetattr: trace_tcsetattr,
        getpwnam: trace_getpwnam,
        pending_signal_bits: test_pending_signal_bits,
        take_pending_signal_bits: test_take_pending_signal_bits,
        monotonic_clock_ns: trace_monotonic_clock_ns,
        setup_locale: test_setup_locale,
        reinit_locale: test_reinit_locale,
        classify_byte: test_classify_byte,
        classify_char: test_classify_char,
        decode_char: test_decode_char,
        encode_char: test_encode_char,
        mb_cur_max: test_mb_cur_max,
        to_upper: test_to_upper,
        to_lower: test_to_lower,
        char_width: test_char_width,
        strcoll: test_strcoll,
        decimal_point: test_decimal_point,
    }
}

pub(super) fn no_interface_table() -> SystemInterface {
    fn panic_getpid() -> Pid {
        panic!("unexpected syscall 'getpid' in pure-logic test")
    }
    fn panic_getppid() -> Pid {
        panic!("unexpected syscall 'getppid' in pure-logic test")
    }
    fn panic_waitpid(_: Pid, _: *mut c_int, _: c_int) -> Pid {
        panic!("unexpected syscall 'waitpid' in pure-logic test")
    }
    fn panic_kill(_: Pid, _: c_int) -> c_int {
        panic!("unexpected syscall 'kill' in pure-logic test")
    }
    fn panic_signal(_: c_int, _: libc::sighandler_t) -> libc::sighandler_t {
        panic!("unexpected syscall 'signal' in pure-logic test")
    }
    fn panic_isatty(_: c_int) -> c_int {
        panic!("unexpected syscall 'isatty' in pure-logic test")
    }
    fn panic_tcgetpgrp(_: c_int) -> Pid {
        panic!("unexpected syscall 'tcgetpgrp' in pure-logic test")
    }
    fn panic_tcsetpgrp(_: c_int, _: Pid) -> c_int {
        panic!("unexpected syscall 'tcsetpgrp' in pure-logic test")
    }
    fn panic_setpgid(_: Pid, _: Pid) -> c_int {
        panic!("unexpected syscall 'setpgid' in pure-logic test")
    }
    fn panic_pipe(_: &mut [c_int; 2]) -> c_int {
        panic!("unexpected syscall 'pipe' in pure-logic test")
    }
    fn panic_dup2(_: c_int, _: c_int) -> c_int {
        panic!("unexpected syscall 'dup2' in pure-logic test")
    }
    fn panic_close(_: c_int) -> c_int {
        panic!("unexpected syscall 'close' in pure-logic test")
    }
    fn panic_fcntl(_: c_int, _: c_int, _: c_int) -> c_int {
        panic!("unexpected syscall 'fcntl' in pure-logic test")
    }
    fn panic_read(_: c_int, _: &mut [u8]) -> isize {
        panic!("unexpected syscall 'read' in pure-logic test")
    }
    fn panic_umask(_: FileModeMask) -> FileModeMask {
        panic!("unexpected syscall 'umask' in pure-logic test")
    }
    fn panic_times(_: *mut libc::tms) -> ClockTicks {
        panic!("unexpected syscall 'times' in pure-logic test")
    }
    fn panic_sysconf(_: c_int) -> c_long {
        panic!("unexpected syscall 'sysconf' in pure-logic test")
    }
    fn panic_execvp(_: *const c_char, _: *const *const c_char) -> c_int {
        panic!("unexpected syscall 'execvp' in pure-logic test")
    }
    fn panic_execve(_: *const c_char, _: *const *const c_char, _: *const *const c_char) -> c_int {
        panic!("unexpected syscall 'execve' in pure-logic test")
    }
    fn panic_open(_: *const c_char, _: c_int, _: mode_t) -> c_int {
        panic!("unexpected syscall 'open' in pure-logic test")
    }
    fn panic_write(_: c_int, _: &[u8]) -> isize {
        panic!("unexpected syscall 'write' in pure-logic test")
    }
    fn panic_stat(_: *const c_char, _: *mut libc::stat) -> c_int {
        panic!("unexpected syscall 'stat' in pure-logic test")
    }
    fn panic_lstat(_: *const c_char, _: *mut libc::stat) -> c_int {
        panic!("unexpected syscall 'lstat' in pure-logic test")
    }
    fn panic_fstat(_: c_int, _: *mut libc::stat) -> c_int {
        panic!("unexpected syscall 'fstat' in pure-logic test")
    }
    fn panic_access(_: *const c_char, _: c_int) -> c_int {
        panic!("unexpected syscall 'access' in pure-logic test")
    }
    fn panic_chdir(_: *const c_char) -> c_int {
        panic!("unexpected syscall 'chdir' in pure-logic test")
    }
    fn panic_getcwd(_: *mut c_char, _: usize) -> *mut c_char {
        panic!("unexpected syscall 'getcwd' in pure-logic test")
    }
    fn panic_opendir(_: *const c_char) -> *mut libc::DIR {
        panic!("unexpected syscall 'opendir' in pure-logic test")
    }
    fn panic_readdir(_: *mut libc::DIR) -> *mut libc::dirent {
        panic!("unexpected syscall 'readdir' in pure-logic test")
    }
    fn panic_closedir(_: *mut libc::DIR) -> c_int {
        panic!("unexpected syscall 'closedir' in pure-logic test")
    }
    fn panic_unlink(_: *const c_char) -> c_int {
        panic!("unexpected syscall 'unlink' in pure-logic test")
    }
    fn panic_realpath(_: *const c_char, _: *mut c_char) -> *mut c_char {
        panic!("unexpected syscall 'realpath' in pure-logic test")
    }
    fn panic_fork() -> Pid {
        panic!("unexpected syscall 'fork' in pure-logic test")
    }
    fn panic_exit_process(_: c_int) {
        panic!("unexpected syscall 'exit_process' in pure-logic test")
    }
    fn panic_setenv(_: &[u8], _: &[u8]) -> SysResult<()> {
        panic!("unexpected call 'setenv' in pure-logic test")
    }
    fn panic_unsetenv(_: &[u8]) -> SysResult<()> {
        panic!("unexpected call 'unsetenv' in pure-logic test")
    }
    fn panic_getenv(_: &[u8]) -> Option<Vec<u8>> {
        panic!("unexpected call 'getenv' in pure-logic test")
    }
    fn panic_get_environ() -> HashMap<Vec<u8>, Vec<u8>> {
        panic!("unexpected call 'get_environ' in pure-logic test")
    }
    fn panic_tcgetattr(_: c_int, _: *mut libc::termios) -> c_int {
        panic!("unexpected syscall 'tcgetattr' in pure-logic test")
    }
    fn panic_tcsetattr(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
        panic!("unexpected syscall 'tcsetattr' in pure-logic test")
    }
    fn panic_getpwnam(_: &[u8]) -> Option<Vec<u8>> {
        panic!("unexpected call 'getpwnam' in pure-logic test")
    }
    fn panic_monotonic_clock_ns() -> u64 {
        panic!("unexpected call 'monotonic_clock_ns' in pure-logic test")
    }
    SystemInterface {
        getpid: panic_getpid,
        getppid: panic_getppid,
        waitpid: panic_waitpid,
        kill: panic_kill,
        signal: panic_signal,
        isatty: panic_isatty,
        tcgetpgrp: panic_tcgetpgrp,
        tcsetpgrp: panic_tcsetpgrp,
        setpgid: panic_setpgid,
        pipe: panic_pipe,
        dup2: panic_dup2,
        close: panic_close,
        fcntl: panic_fcntl,
        read: panic_read,
        umask: panic_umask,
        times: panic_times,
        sysconf: panic_sysconf,
        execvp: panic_execvp,
        execve: panic_execve,
        open: panic_open,
        write: panic_write,
        stat: panic_stat,
        lstat: panic_lstat,
        fstat: panic_fstat,
        access: panic_access,
        chdir: panic_chdir,
        getcwd: panic_getcwd,
        opendir: panic_opendir,
        readdir: panic_readdir,
        closedir: panic_closedir,
        realpath: panic_realpath,
        unlink: panic_unlink,
        fork: panic_fork,
        exit_process: panic_exit_process,
        setenv: panic_setenv,
        unsetenv: panic_unsetenv,
        getenv: panic_getenv,
        get_environ: panic_get_environ,
        tcgetattr: panic_tcgetattr,
        tcsetattr: panic_tcsetattr,
        getpwnam: panic_getpwnam,
        pending_signal_bits: test_pending_signal_bits,
        take_pending_signal_bits: test_take_pending_signal_bits,
        monotonic_clock_ns: panic_monotonic_clock_ns,
        setup_locale: test_setup_locale,
        reinit_locale: test_reinit_locale,
        classify_byte: test_classify_byte,
        classify_char: test_classify_char,
        decode_char: test_decode_char,
        encode_char: test_encode_char,
        mb_cur_max: test_mb_cur_max,
        to_upper: test_to_upper,
        to_lower: test_to_lower,
        char_width: test_char_width,
        strcoll: test_strcoll,
        decimal_point: test_decimal_point,
    }
}

pub(crate) fn assert_no_syscalls<T>(f: impl FnOnce() -> T) -> T {
    with_test_interface(no_interface_table(), f)
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
        let iface = trace_interface();

        TRACE_LOG.with(|cell| {
            let prev_trace = cell.replace(Some(path.clone()));
            let prev_index = TRACE_INDEX.with(|idx| idx.replace(0));
            let prev_children = CHILD_TRACES.with(|c| std::mem::take(&mut *c.borrow_mut()));

            let result = std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(|| with_test_interface(iface, &f))
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
