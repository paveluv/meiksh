use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicUsize, Ordering};
use libc::{self, c_char, c_int, c_long, mode_t};

pub type Pid = libc::pid_t;
pub type RawFd = c_int;
pub type FileModeMask = libc::mode_t;
type ClockTicks = libc::clock_t;

const SC_CLK_TCK: c_int = libc::_SC_CLK_TCK;
const F_GETFL: c_int = libc::F_GETFL;
const F_SETFL: c_int = libc::F_SETFL;
const O_NONBLOCK: c_int = libc::O_NONBLOCK;

pub const STDIN_FILENO: c_int = libc::STDIN_FILENO;
pub const STDOUT_FILENO: c_int = libc::STDOUT_FILENO;
pub const STDERR_FILENO: c_int = libc::STDERR_FILENO;
pub const SIGHUP: c_int = libc::SIGHUP;
pub const SIGINT: c_int = libc::SIGINT;
pub const SIGQUIT: c_int = libc::SIGQUIT;
pub const SIGABRT: c_int = libc::SIGABRT;
pub const SIGALRM: c_int = libc::SIGALRM;
pub const SIGCONT: c_int = libc::SIGCONT;
pub const SIGTERM: c_int = libc::SIGTERM;
pub const WNOHANG: c_int = libc::WNOHANG;
pub const ENOENT: c_int = libc::ENOENT;
pub const ENOEXEC: c_int = libc::ENOEXEC;
pub const EBADF: c_int = libc::EBADF;
pub const ECHILD: c_int = libc::ECHILD;
pub const EACCES: c_int = libc::EACCES;
pub const EEXIST: c_int = libc::EEXIST;
pub const EINVAL: c_int = libc::EINVAL;
pub const ENOTTY: c_int = libc::ENOTTY;
pub const EILSEQ: c_int = libc::EILSEQ;
pub const EIO: c_int = libc::EIO;
pub const EISDIR: c_int = libc::EISDIR;
pub const EINTR: c_int = libc::EINTR;

const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

pub const O_RDONLY: c_int = libc::O_RDONLY;
pub const O_WRONLY: c_int = libc::O_WRONLY;
pub const O_RDWR: c_int = libc::O_RDWR;
pub const O_CREAT: c_int = libc::O_CREAT;
pub const O_TRUNC: c_int = libc::O_TRUNC;
pub const O_APPEND: c_int = libc::O_APPEND;
pub const O_EXCL: c_int = libc::O_EXCL;
pub const O_CLOEXEC: c_int = libc::O_CLOEXEC;

pub const F_OK: c_int = libc::F_OK;
pub const R_OK: c_int = libc::R_OK;
pub const W_OK: c_int = libc::W_OK;
pub const X_OK: c_int = libc::X_OK;

pub const S_IFMT: mode_t = libc::S_IFMT;
pub const S_IFDIR: mode_t = libc::S_IFDIR;
pub const S_IFREG: mode_t = libc::S_IFREG;
pub const S_IFIFO: mode_t = libc::S_IFIFO;
pub const S_IXUSR: mode_t = libc::S_IXUSR;
pub const S_IXGRP: mode_t = libc::S_IXGRP;
pub const S_IXOTH: mode_t = libc::S_IXOTH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SysError {
    Errno(c_int),
    NulInPath,
}

pub type SysResult<T> = Result<T, SysError>;

impl std::fmt::Display for SysError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SysError::Errno(errno) => {
                let msg = unsafe { CStr::from_ptr(libc::strerror(*errno)) };
                write!(f, "{}", msg.to_string_lossy())
            }
            SysError::NulInPath => write!(f, "path contains null byte"),
        }
    }
}

impl std::error::Error for SysError {}

impl SysError {
    pub fn errno(&self) -> Option<c_int> {
        match self {
            SysError::Errno(e) => Some(*e),
            _ => None,
        }
    }

    pub fn is_enoent(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::ENOENT)
    }

    pub fn is_ebadf(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::EBADF)
    }

    pub fn is_enoexec(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::ENOEXEC)
    }

    pub fn is_eintr(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == EINTR)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Syscalls {
    getpid: fn() -> Pid,
    waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    kill: fn(Pid, c_int) -> c_int,
    signal: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
    isatty: fn(c_int) -> c_int,
    tcgetpgrp: fn(c_int) -> Pid,
    tcsetpgrp: fn(c_int, Pid) -> c_int,
    setpgid: fn(Pid, Pid) -> c_int,
    pipe: fn(*mut c_int) -> c_int,
    dup: fn(c_int) -> c_int,
    dup2: fn(c_int, c_int) -> c_int,
    close: fn(c_int) -> c_int,
    fcntl: fn(c_int, c_int, c_int) -> c_int,
    read: fn(c_int, *mut u8, usize) -> isize,
    umask: fn(FileModeMask) -> FileModeMask,
    times: fn(*mut libc::tms) -> ClockTicks,
    sysconf: fn(c_int) -> c_long,
    execvp: fn(*const c_char, *const *const c_char) -> c_int,
    // Filesystem syscalls
    open: fn(*const c_char, c_int, mode_t) -> c_int,
    write: fn(c_int, *const u8, usize) -> isize,
    stat: fn(*const c_char, *mut libc::stat) -> c_int,
    lstat: fn(*const c_char, *mut libc::stat) -> c_int,
    fstat: fn(c_int, *mut libc::stat) -> c_int,
    access: fn(*const c_char, c_int) -> c_int,
    chdir: fn(*const c_char) -> c_int,
    getcwd: fn(*mut c_char, usize) -> *mut c_char,
    opendir: fn(*const c_char) -> *mut libc::DIR,
    readdir: fn(*mut libc::DIR) -> *mut libc::dirent,
    closedir: fn(*mut libc::DIR) -> c_int,
    realpath: fn(*const c_char, *mut c_char) -> *mut c_char,
    readlink: fn(*const c_char, *mut c_char, usize) -> isize,
    unlink: fn(*const c_char) -> c_int,
    // Process syscalls
    fork: fn() -> Pid,
    exit_process: fn(c_int),
}

pub(crate) fn default_syscalls() -> Syscalls {
    Syscalls {
        getpid: || unsafe { libc::getpid() },
        waitpid: |pid, status, options| unsafe { libc::waitpid(pid, status, options) },
        kill: |pid, sig| unsafe { libc::kill(pid, sig) },
        signal: |sig, handler| unsafe { libc::signal(sig, handler) },
        isatty: |fd| unsafe { libc::isatty(fd) },
        tcgetpgrp: |fd| unsafe { libc::tcgetpgrp(fd) },
        tcsetpgrp: |fd, pgrp| unsafe { libc::tcsetpgrp(fd, pgrp) },
        setpgid: |pid, pgid| unsafe { libc::setpgid(pid, pgid) },
        pipe: |fds| unsafe { libc::pipe(fds) },
        dup: |fd| unsafe { libc::dup(fd) },
        dup2: |oldfd, newfd| unsafe { libc::dup2(oldfd, newfd) },
        close: |fd| unsafe { libc::close(fd) },
        fcntl: |fd, cmd, arg| unsafe { libc::fcntl(fd, cmd, arg) },
        read: |fd, buf, count| unsafe { libc::read(fd, buf.cast(), count) },
        umask: |cmask| unsafe { libc::umask(cmask) },
        times: |buffer| unsafe { libc::times(buffer) },
        sysconf: |name| unsafe { libc::sysconf(name) },
        execvp: |file, argv| unsafe { libc::execvp(file, argv) },
        open: |path, flags, mode| unsafe { libc::open(path, flags, mode as c_int) },
        write: |fd, buf, count| unsafe { libc::write(fd, buf.cast(), count) },
        stat: |path, buf| unsafe { libc::stat(path, buf) },
        lstat: |path, buf| unsafe { libc::lstat(path, buf) },
        fstat: |fd, buf| unsafe { libc::fstat(fd, buf) },
        access: |path, mode| unsafe { libc::access(path, mode) },
        chdir: |path| unsafe { libc::chdir(path) },
        getcwd: |buf, size| unsafe { libc::getcwd(buf, size) },
        opendir: |path| unsafe { libc::opendir(path) },
        readdir: |dirp| unsafe { libc::readdir(dirp) },
        closedir: |dirp| unsafe { libc::closedir(dirp) },
        realpath: |path, resolved| unsafe { libc::realpath(path, resolved) },
        readlink: |path, buf, bufsiz| unsafe { libc::readlink(path, buf, bufsiz) },
        unlink: |path| unsafe { libc::unlink(path) },
        fork: || unsafe { libc::fork() },
        exit_process: |status| unsafe { libc::_exit(status) },
    }
}

static PENDING_SIGNALS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn record_signal(sig: c_int) {
    if let Some(mask) = signal_mask(sig) {
        PENDING_SIGNALS.fetch_or(mask, Ordering::SeqCst);
    }
}

fn syscalls() -> Syscalls {
    #[cfg(test)]
    {
        return test_support::current_syscalls().unwrap_or_else(default_syscalls);
    }

    #[cfg(not(test))]
    {
        default_syscalls()
    }
}

pub(crate) fn set_errno(errno: c_int) {
    #[cfg(test)]
    {
        test_support::set_test_errno(errno);
        return;
    }

    #[cfg(not(test))]
    unsafe {
        *libc::__error() = errno;
    }
}

fn last_error() -> SysError {
    #[cfg(test)]
    {
        return test_support::take_test_error();
    }

    #[cfg(not(test))]
    SysError::Errno(unsafe { *libc::__error() })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    thread_local! {
        static TEST_SYSCALLS: RefCell<Option<Syscalls>> = const { RefCell::new(None) };
        static TEST_ERRNO: RefCell<c_int> = const { RefCell::new(0) };
        static TEST_PENDING_SIGNALS: RefCell<usize> = const { RefCell::new(0) };
        static TEST_PROCESS_IDS: RefCell<Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)>> =
            const { RefCell::new(None) };
    }

    fn syscall_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    pub(crate) fn current_syscalls() -> Option<Syscalls> {
        TEST_SYSCALLS.with(|cell| *cell.borrow())
    }

    pub(crate) fn current_process_ids() -> Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)> {
        TEST_PROCESS_IDS.with(|cell| *cell.borrow())
    }

    pub(super) fn set_test_errno(errno: c_int) {
        TEST_ERRNO.with(|cell| *cell.borrow_mut() = errno);
    }

    pub(super) fn take_test_error() -> SysError {
        let errno = TEST_ERRNO.with(|cell| cell.replace(0));
        SysError::Errno(errno)
    }

    pub(super) fn test_pending_signal_bits() -> usize {
        TEST_PENDING_SIGNALS.with(|cell| *cell.borrow())
    }

    pub(super) fn test_take_pending_signal_bits() -> usize {
        TEST_PENDING_SIGNALS.with(|cell| cell.replace(0))
    }

    pub(crate) fn with_test_syscalls<T>(syscalls: Syscalls, f: impl FnOnce() -> T) -> T {
        let _guard = syscall_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        TEST_SYSCALLS.with(|cell| {
            let previous = cell.replace(Some(syscalls));
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
        Str(String),
        Bytes(Vec<u8>),
        Fd(c_int),
    }

    #[derive(Clone, Debug)]
    #[allow(dead_code)]
    pub(crate) enum TraceResult {
        Int(i64),
        Fd(c_int),
        Pid(Pid),
        Bytes(Vec<u8>),
        Err(c_int),
        Status(i32),
        Fds(c_int, c_int),
        Void,
        CwdStr(String),
        RealpathStr(String),
        ReadlinkStr(String),
        StatDir,
        StatFile(mode_t),
        StatFifo,
        DirEntry(String),
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

    fn apply_trace_result_int(entry: &TraceEntry) -> c_int {
        match &entry.result {
            TraceResult::Int(v) => *v as c_int,
            TraceResult::Fd(fd) => *fd,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            other => panic!("trace result type mismatch for '{}': expected Int/Fd/Err, got {other:?}", entry.syscall),
        }
    }

    fn apply_trace_result_isize(entry: &TraceEntry) -> isize {
        match &entry.result {
            TraceResult::Int(v) => *v as isize,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            _ => panic!("trace result type mismatch for '{}': expected Int/Err", entry.syscall),
        }
    }

    fn apply_trace_result_pid(entry: &TraceEntry) -> Pid {
        match &entry.result {
            TraceResult::Pid(p) => *p,
            TraceResult::Int(v) => *v as Pid,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            other => panic!("trace result type mismatch for '{}': expected Pid/Err, got {other:?}", entry.syscall),
        }
    }

    // Trace-dispatching syscall implementations
    fn trace_getpid() -> Pid {
        let entry = trace_dispatch("getpid", &[]);
        apply_trace_result_pid(&entry)
    }
    fn trace_waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
        let entry = trace_dispatch("waitpid", &[ArgMatcher::Int(pid as i64), ArgMatcher::Any, ArgMatcher::Int(options as i64)]);
        if !status.is_null() {
            if let TraceResult::Status(s) = entry.result {
                unsafe { *status = s << 8; }
                return pid;
            }
        }
        apply_trace_result_pid(&entry)
    }
    fn trace_kill(pid: Pid, sig: c_int) -> c_int {
        let entry = trace_dispatch("kill", &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(sig as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_signal(sig: c_int, handler: libc::sighandler_t) -> libc::sighandler_t {
        let _ = handler;
        let entry = trace_dispatch("signal", &[ArgMatcher::Int(sig as i64), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::Int(v) => *v as libc::sighandler_t,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
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
        let entry = trace_dispatch("tcsetpgrp", &[ArgMatcher::Fd(fd), ArgMatcher::Int(pgrp as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_setpgid(pid: Pid, pgid: Pid) -> c_int {
        let entry = trace_dispatch("setpgid", &[ArgMatcher::Int(pid as i64), ArgMatcher::Int(pgid as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_pipe(fds: *mut c_int) -> c_int {
        let entry = trace_dispatch("pipe", &[]);
        match &entry.result {
            TraceResult::Fds(r, w) => {
                unsafe { *fds = *r; *fds.add(1) = *w; }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            other => panic!("trace result type mismatch for 'pipe': expected Fds/Err, got {other:?}"),
        }
    }
    fn trace_dup(fd: c_int) -> c_int {
        let entry = trace_dispatch("dup", &[ArgMatcher::Fd(fd)]);
        apply_trace_result_int(&entry)
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
        let entry = trace_dispatch("fcntl", &[ArgMatcher::Fd(fd), ArgMatcher::Int(cmd as i64), ArgMatcher::Int(arg as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_read(fd: c_int, buf: *mut u8, count: usize) -> isize {
        let entry = trace_dispatch("read", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::Bytes(data) => {
                let n = data.len().min(count);
                unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), buf, n); }
                n as isize
            }
            TraceResult::Int(v) => *v as isize,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            other => panic!("trace result type mismatch for 'read': expected Bytes/Int/Err, got {other:?}"),
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
            other => panic!("trace result type mismatch for 'times': expected Int/Err, got {other:?}"),
        }
    }
    fn trace_sysconf(name: c_int) -> c_long {
        let entry = trace_dispatch("sysconf", &[ArgMatcher::Int(name as i64)]);
        match &entry.result {
            TraceResult::Int(v) => *v as c_long,
            other => panic!("trace result type mismatch for 'sysconf': expected Int, got {other:?}"),
        }
    }
    fn trace_execvp(file: *const c_char, _argv: *const *const c_char) -> c_int {
        let name = unsafe { CStr::from_ptr(file) }.to_string_lossy().to_string();
        let entry = trace_dispatch("execvp", &[ArgMatcher::Str(name), ArgMatcher::Any]);
        apply_trace_result_int(&entry)
    }
    fn trace_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("open", &[ArgMatcher::Str(p), ArgMatcher::Int(flags as i64), ArgMatcher::Int(mode as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_write(fd: c_int, buf: *const u8, count: usize) -> isize {
        let data = unsafe { std::slice::from_raw_parts(buf, count) };
        let entry = trace_dispatch("write", &[ArgMatcher::Fd(fd), ArgMatcher::Bytes(data.to_vec())]);
        apply_trace_result_isize(&entry)
    }
    fn trace_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("stat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::StatDir => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFDIR | 0o755; }
                0
            }
            TraceResult::StatFile(mode) => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFREG | mode; }
                0
            }
            TraceResult::StatFifo => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFIFO | 0o644; }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            TraceResult::Int(v) => *v as c_int,
            other => panic!("trace result type mismatch for 'stat': expected StatDir/StatFile/Err, got {other:?}"),
        }
    }
    fn trace_lstat(path: *const c_char, buf: *mut libc::stat) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("lstat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::StatDir => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFDIR | 0o755; }
                0
            }
            TraceResult::StatFile(mode) => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFREG | mode; }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            TraceResult::Int(v) => *v as c_int,
            other => panic!("trace result type mismatch for 'lstat': expected StatDir/StatFile/Err, got {other:?}"),
        }
    }
    fn trace_fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
        let entry = trace_dispatch("fstat", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::StatDir => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFDIR | 0o755; }
                0
            }
            TraceResult::StatFile(mode) => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFREG | mode; }
                0
            }
            TraceResult::StatFifo => {
                unsafe { std::ptr::write_bytes(buf, 0, 1); (*buf).st_mode = libc::S_IFIFO | 0o644; }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            TraceResult::Int(v) => *v as c_int,
            other => panic!("trace result type mismatch for 'fstat': expected StatDir/StatFile/StatFifo/Err, got {other:?}"),
        }
    }
    fn trace_access(path: *const c_char, mode: c_int) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("access", &[ArgMatcher::Str(p), ArgMatcher::Int(mode as i64)]);
        apply_trace_result_int(&entry)
    }
    fn trace_chdir(path: *const c_char) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("chdir", &[ArgMatcher::Str(p)]);
        apply_trace_result_int(&entry)
    }
    fn trace_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
        let entry = trace_dispatch("getcwd", &[]);
        match &entry.result {
            TraceResult::CwdStr(s) => {
                let bytes = s.as_bytes();
                if bytes.len() + 1 > size {
                    super::set_errno(libc::ERANGE);
                    return std::ptr::null_mut();
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
                    *buf.add(bytes.len()) = 0;
                }
                buf
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!("trace result type mismatch for 'getcwd': expected CwdStr/Err, got {other:?}"),
        }
    }
    fn trace_opendir(path: *const c_char) -> *mut libc::DIR {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("opendir", &[ArgMatcher::Str(p)]);
        match &entry.result {
            TraceResult::Int(v) => *v as *mut libc::DIR,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!("trace result type mismatch for 'opendir': expected Int/Err, got {other:?}"),
        }
    }
    thread_local! {
        static FAKE_DIRENT: std::cell::RefCell<libc::dirent> = const { std::cell::RefCell::new(unsafe { std::mem::zeroed() }) };
    }
    fn trace_readdir(_dirp: *mut libc::DIR) -> *mut libc::dirent {
        let entry = trace_dispatch("readdir", &[ArgMatcher::Any]);
        match &entry.result {
            TraceResult::DirEntry(name) => {
                FAKE_DIRENT.with(|cell| {
                    let mut d = cell.borrow_mut();
                    d.d_name = unsafe { std::mem::zeroed() };
                    let bytes = name.as_bytes();
                    let len = bytes.len().min(d.d_name.len() - 1);
                    for (i, &b) in bytes[..len].iter().enumerate() {
                        d.d_name[i] = b as i8;
                    }
                    d.d_name[len] = 0;
                    &mut *d as *mut libc::dirent
                })
            }
            TraceResult::Int(0) => {
                super::set_errno(0);
                std::ptr::null_mut()
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!("trace result type mismatch for 'readdir': expected DirEntry/Int(0)/Err, got {other:?}"),
        }
    }
    fn trace_closedir(_dirp: *mut libc::DIR) -> c_int {
        let entry = trace_dispatch("closedir", &[ArgMatcher::Any]);
        apply_trace_result_int(&entry)
    }
    fn trace_realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("realpath", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::RealpathStr(s) => {
                let c_result = std::ffi::CString::new(s.as_str()).unwrap();
                if resolved.is_null() {
                    let ptr = unsafe { libc::malloc(c_result.as_bytes_with_nul().len()) } as *mut c_char;
                    unsafe {
                        std::ptr::copy_nonoverlapping(c_result.as_ptr(), ptr, c_result.as_bytes_with_nul().len());
                    }
                    ptr
                } else {
                    unsafe {
                        std::ptr::copy_nonoverlapping(c_result.as_ptr(), resolved, c_result.as_bytes_with_nul().len());
                    }
                    resolved
                }
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!("trace result type mismatch for 'realpath': expected RealpathStr/Err, got {other:?}"),
        }
    }
    fn trace_readlink(path: *const c_char, buf: *mut c_char, bufsiz: usize) -> isize {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("readlink", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::ReadlinkStr(s) => {
                let bytes = s.as_bytes();
                let n = bytes.len().min(bufsiz);
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, n); }
                n as isize
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            other => panic!("trace result type mismatch for 'readlink': expected ReadlinkStr/Err, got {other:?}"),
        }
    }
    fn trace_unlink(path: *const c_char) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }.to_string_lossy().to_string();
        let entry = trace_dispatch("unlink", &[ArgMatcher::Str(p)]);
        apply_trace_result_int(&entry)
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

    #[allow(dead_code)]
    pub(crate) struct ChildExitPanic(pub i32);

    pub(crate) fn trace_syscalls() -> Syscalls {
        Syscalls {
            getpid: trace_getpid,
            waitpid: trace_waitpid,
            kill: trace_kill,
            signal: trace_signal,
            isatty: trace_isatty,
            tcgetpgrp: trace_tcgetpgrp,
            tcsetpgrp: trace_tcsetpgrp,
            setpgid: trace_setpgid,
            pipe: trace_pipe,
            dup: trace_dup,
            dup2: trace_dup2,
            close: trace_close,
            fcntl: trace_fcntl,
            read: trace_read,
            umask: trace_umask,
            times: trace_times,
            sysconf: trace_sysconf,
            execvp: trace_execvp,
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
            readlink: trace_readlink,
            unlink: trace_unlink,
            fork: trace_fork,
            exit_process: trace_exit_process,
        }
    }

    pub(crate) fn no_syscalls_table() -> Syscalls {
        fn panic_getpid() -> Pid { panic!("unexpected syscall 'getpid' in pure-logic test") }
        fn panic_waitpid(_: Pid, _: *mut c_int, _: c_int) -> Pid { panic!("unexpected syscall 'waitpid' in pure-logic test") }
        fn panic_kill(_: Pid, _: c_int) -> c_int { panic!("unexpected syscall 'kill' in pure-logic test") }
        fn panic_signal(_: c_int, _: libc::sighandler_t) -> libc::sighandler_t { panic!("unexpected syscall 'signal' in pure-logic test") }
        fn panic_isatty(_: c_int) -> c_int { panic!("unexpected syscall 'isatty' in pure-logic test") }
        fn panic_tcgetpgrp(_: c_int) -> Pid { panic!("unexpected syscall 'tcgetpgrp' in pure-logic test") }
        fn panic_tcsetpgrp(_: c_int, _: Pid) -> c_int { panic!("unexpected syscall 'tcsetpgrp' in pure-logic test") }
        fn panic_setpgid(_: Pid, _: Pid) -> c_int { panic!("unexpected syscall 'setpgid' in pure-logic test") }
        fn panic_pipe(_: *mut c_int) -> c_int { panic!("unexpected syscall 'pipe' in pure-logic test") }
        fn panic_dup(_: c_int) -> c_int { panic!("unexpected syscall 'dup' in pure-logic test") }
        fn panic_dup2(_: c_int, _: c_int) -> c_int { panic!("unexpected syscall 'dup2' in pure-logic test") }
        fn panic_close(_: c_int) -> c_int { panic!("unexpected syscall 'close' in pure-logic test") }
        fn panic_fcntl(_: c_int, _: c_int, _: c_int) -> c_int { panic!("unexpected syscall 'fcntl' in pure-logic test") }
        fn panic_read(_: c_int, _: *mut u8, _: usize) -> isize { panic!("unexpected syscall 'read' in pure-logic test") }
        fn panic_umask(_: FileModeMask) -> FileModeMask { panic!("unexpected syscall 'umask' in pure-logic test") }
        fn panic_times(_: *mut libc::tms) -> ClockTicks { panic!("unexpected syscall 'times' in pure-logic test") }
        fn panic_sysconf(_: c_int) -> c_long { panic!("unexpected syscall 'sysconf' in pure-logic test") }
        fn panic_execvp(_: *const c_char, _: *const *const c_char) -> c_int { panic!("unexpected syscall 'execvp' in pure-logic test") }
        fn panic_open(_: *const c_char, _: c_int, _: mode_t) -> c_int { panic!("unexpected syscall 'open' in pure-logic test") }
        fn panic_write(_: c_int, _: *const u8, _: usize) -> isize { panic!("unexpected syscall 'write' in pure-logic test") }
        fn panic_stat(_: *const c_char, _: *mut libc::stat) -> c_int { panic!("unexpected syscall 'stat' in pure-logic test") }
        fn panic_lstat(_: *const c_char, _: *mut libc::stat) -> c_int { panic!("unexpected syscall 'lstat' in pure-logic test") }
        fn panic_fstat(_: c_int, _: *mut libc::stat) -> c_int { panic!("unexpected syscall 'fstat' in pure-logic test") }
        fn panic_access(_: *const c_char, _: c_int) -> c_int { panic!("unexpected syscall 'access' in pure-logic test") }
        fn panic_chdir(_: *const c_char) -> c_int { panic!("unexpected syscall 'chdir' in pure-logic test") }
        fn panic_getcwd(_: *mut c_char, _: usize) -> *mut c_char { panic!("unexpected syscall 'getcwd' in pure-logic test") }
        fn panic_opendir(_: *const c_char) -> *mut libc::DIR { panic!("unexpected syscall 'opendir' in pure-logic test") }
        fn panic_readdir(_: *mut libc::DIR) -> *mut libc::dirent { panic!("unexpected syscall 'readdir' in pure-logic test") }
        fn panic_closedir(_: *mut libc::DIR) -> c_int { panic!("unexpected syscall 'closedir' in pure-logic test") }
        fn panic_realpath(_: *const c_char, _: *mut c_char) -> *mut c_char { panic!("unexpected syscall 'realpath' in pure-logic test") }
        fn panic_readlink(_: *const c_char, _: *mut c_char, _: usize) -> isize { panic!("unexpected syscall 'readlink' in pure-logic test") }
        fn panic_unlink(_: *const c_char) -> c_int { panic!("unexpected syscall 'unlink' in pure-logic test") }
        fn panic_fork() -> Pid { panic!("unexpected syscall 'fork' in pure-logic test") }
        fn panic_exit_process(_: c_int) { panic!("unexpected syscall 'exit_process' in pure-logic test") }

        Syscalls {
            getpid: panic_getpid, waitpid: panic_waitpid, kill: panic_kill,
            signal: panic_signal, isatty: panic_isatty, tcgetpgrp: panic_tcgetpgrp,
            tcsetpgrp: panic_tcsetpgrp, setpgid: panic_setpgid, pipe: panic_pipe,
            dup: panic_dup, dup2: panic_dup2, close: panic_close, fcntl: panic_fcntl,
            read: panic_read, umask: panic_umask, times: panic_times, sysconf: panic_sysconf,
            execvp: panic_execvp, open: panic_open, write: panic_write, stat: panic_stat,
            lstat: panic_lstat, fstat: panic_fstat, access: panic_access, chdir: panic_chdir,
            getcwd: panic_getcwd, opendir: panic_opendir, readdir: panic_readdir,
            closedir: panic_closedir, realpath: panic_realpath, readlink: panic_readlink,
            unlink: panic_unlink, fork: panic_fork, exit_process: panic_exit_process,
        }
    }

    pub(crate) fn assert_no_syscalls<T>(f: impl FnOnce() -> T) -> T {
        with_test_syscalls(no_syscalls_table(), f)
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
            let syscalls = trace_syscalls();

            TRACE_LOG.with(|cell| {
                let prev_trace = cell.replace(Some(path.clone()));
                let prev_index = TRACE_INDEX.with(|idx| idx.replace(0));
                let prev_children = CHILD_TRACES.with(|c| std::mem::take(&mut *c.borrow_mut()));

                let result = std::panic::catch_unwind(
                    std::panic::AssertUnwindSafe(|| with_test_syscalls(syscalls, &f))
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
                let prefix: Vec<TraceEntry> = trace[..i].iter().map(|e| e.without_child_trace()).collect();
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
        TraceEntry { syscall, args, result, child_trace: None }
    }

    pub(crate) fn t_fork(result: TraceResult, child: Vec<TraceEntry>) -> TraceEntry {
        TraceEntry { syscall: "fork", args: vec![], result, child_trace: Some(child) }
    }

    #[allow(dead_code)]
    pub(crate) trait IntoArgMatcher {
        fn into_arg(self) -> ArgMatcher;
    }
    impl IntoArgMatcher for i32 {
        fn into_arg(self) -> ArgMatcher { ArgMatcher::Int(self as i64) }
    }
    impl IntoArgMatcher for i64 {
        fn into_arg(self) -> ArgMatcher { ArgMatcher::Int(self) }
    }
    impl IntoArgMatcher for &str {
        fn into_arg(self) -> ArgMatcher { ArgMatcher::Str(self.to_string()) }
    }
    impl IntoArgMatcher for &[u8] {
        fn into_arg(self) -> ArgMatcher { ArgMatcher::Bytes(self.to_vec()) }
    }
    impl IntoArgMatcher for &Vec<u8> {
        fn into_arg(self) -> ArgMatcher { ArgMatcher::Bytes(self.clone()) }
    }

    #[allow(dead_code)]
    pub(crate) fn arg_from(v: impl IntoArgMatcher) -> ArgMatcher {
        v.into_arg()
    }

}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitStatus {
    pub pid: Pid,
    pub status: c_int,
}

pub fn current_pid() -> Pid {
    (syscalls().getpid)()
}

pub fn is_interactive_fd(fd: c_int) -> bool {
    (syscalls().isatty)(fd) == 1
}

pub fn has_same_real_and_effective_ids() -> bool {
    #[cfg(test)]
    if let Some((uid, euid, gid, egid)) = test_support::current_process_ids() {
        return uid == euid && gid == egid;
    }
    unsafe { libc::getuid() == libc::geteuid() && libc::getgid() == libc::getegid() }
}

pub fn wait_pid(pid: Pid, nohang: bool) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let options = if nohang { WNOHANG } else { 0 };
    let result = (syscalls().waitpid)(pid, &mut status, options);
    if result > 0 {
        Ok(Some(WaitStatus { pid: result, status }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(last_error())
    }
}

pub fn send_signal(pid: Pid, signal: c_int) -> SysResult<()> {
    let result = (syscalls().kill)(pid, signal);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn install_shell_signal_handler(signal: c_int) -> SysResult<()> {
    let result = (syscalls().signal)(signal, record_signal as *const () as libc::sighandler_t);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn ignore_signal(signal: c_int) -> SysResult<()> {
    let result = (syscalls().signal)(signal, SIG_IGN_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn default_signal_action(signal: c_int) -> SysResult<()> {
    let result = (syscalls().signal)(signal, SIG_DFL_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn has_pending_signal() -> Option<c_int> {
    #[cfg(test)]
    let bits = test_support::test_pending_signal_bits();
    #[cfg(not(test))]
    let bits = PENDING_SIGNALS.load(Ordering::SeqCst);

    supported_trap_signals()
        .into_iter()
        .find(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
}

pub fn take_pending_signals() -> Vec<c_int> {
    #[cfg(test)]
    let bits = test_support::test_take_pending_signal_bits();
    #[cfg(not(test))]
    let bits = PENDING_SIGNALS.swap(0, Ordering::SeqCst);

    supported_trap_signals()
        .into_iter()
        .filter(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
        .collect()
}

pub fn supported_trap_signals() -> Vec<c_int> {
    vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]
}

pub fn interrupted(error: &SysError) -> bool {
    error.is_eintr()
}

pub fn current_foreground_pgrp(fd: c_int) -> SysResult<Pid> {
    let result = (syscalls().tcgetpgrp)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> SysResult<()> {
    let result = (syscalls().tcsetpgrp)(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn set_process_group(pid: Pid, pgid: Pid) -> SysResult<()> {
    let result = (syscalls().setpgid)(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn create_pipe() -> SysResult<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = (syscalls().pipe)(fds.as_mut_ptr());
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd(oldfd: c_int, newfd: c_int) -> SysResult<()> {
    let result = (syscalls().dup2)(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd_to_new(fd: c_int) -> SysResult<c_int> {
    let result = (syscalls().dup)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn close_fd(fd: c_int) -> SysResult<()> {
    let result = (syscalls().close)(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fd_status_flags(fd: c_int) -> SysResult<c_int> {
    let result = (syscalls().fcntl)(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> SysResult<()> {
    let result = (syscalls().fcntl)(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fifo_like_fd(fd: c_int) -> bool {
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().fstat)(fd, buf.as_mut_ptr());
    if result != 0 {
        return false;
    }
    let buf = unsafe { buf.assume_init() };
    (buf.st_mode & S_IFMT) == S_IFIFO
}

pub fn ensure_blocking_read_fd(fd: c_int) -> SysResult<()> {
    if !is_interactive_fd(fd) && !fifo_like_fd(fd) {
        return Ok(());
    }
    let flags = fd_status_flags(fd)?;
    if flags & O_NONBLOCK != 0 {
        set_fd_status_flags(fd, flags & !O_NONBLOCK)?;
    }
    Ok(())
}

pub fn read_fd(fd: c_int, buf: &mut [u8]) -> SysResult<usize> {
    let result = (syscalls().read)(fd, buf.as_mut_ptr(), buf.len());
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}

pub struct FdReader {
    fd: c_int,
}

impl FdReader {
    pub fn new(fd: c_int) -> Self {
        Self { fd }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> SysResult<usize> {
        read_fd(self.fd, buf)
    }
}

// --- Filesystem wrapper functions ---

#[derive(Clone, Debug)]
pub struct FileStat {
    pub mode: mode_t,
    pub size: u64,
}

impl FileStat {
    pub fn is_dir(&self) -> bool {
        (self.mode & S_IFMT) == S_IFDIR
    }

    pub fn is_regular_file(&self) -> bool {
        (self.mode & S_IFMT) == S_IFREG
    }

    pub fn is_executable(&self) -> bool {
        self.mode & (S_IXUSR | S_IXGRP | S_IXOTH) != 0
    }
}

fn to_cstring(path: &str) -> SysResult<CString> {
    CString::new(path).map_err(|_| SysError::NulInPath)
}

fn stat_raw(path: &str) -> SysResult<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().stat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(last_error())
    }
}

fn lstat_raw(path: &str) -> SysResult<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (syscalls().lstat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(last_error())
    }
}

pub fn open_file(path: &str, flags: c_int, mode: mode_t) -> SysResult<c_int> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().open)(c_path.as_ptr(), flags, mode);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn write_fd(fd: c_int, data: &[u8]) -> SysResult<usize> {
    let result = (syscalls().write)(fd, data.as_ptr(), data.len());
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(last_error())
    }
}

pub fn write_all_fd(fd: c_int, mut data: &[u8]) -> SysResult<()> {
    while !data.is_empty() {
        let n = write_fd(fd, data)?;
        if n == 0 {
            return Err(SysError::Errno(libc::EIO));
        }
        data = &data[n..];
    }
    Ok(())
}

pub fn stat_path(path: &str) -> SysResult<FileStat> {
    let raw = stat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
    })
}

pub fn lstat_path(path: &str) -> SysResult<FileStat> {
    let raw = lstat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
    })
}

pub fn access_path(path: &str, mode: c_int) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().access)(c_path.as_ptr(), mode);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn file_exists(path: &str) -> bool {
    access_path(path, F_OK).is_ok()
}

pub fn is_directory(path: &str) -> bool {
    stat_path(path).map(|s| s.is_dir()).unwrap_or(false)
}

pub fn is_regular_file(path: &str) -> bool {
    stat_path(path).map(|s| s.is_regular_file()).unwrap_or(false)
}

pub fn change_dir(path: &str) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().chdir)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn get_cwd() -> SysResult<String> {
    let mut buf = vec![0u8; 4096];
    let result = (syscalls().getcwd)(buf.as_mut_ptr().cast(), buf.len());
    if result.is_null() {
        Err(last_error())
    } else {
        let cstr = unsafe { CStr::from_ptr(result) };
        Ok(cstr.to_string_lossy().into_owned())
    }
}

pub fn read_dir_entries(path: &str) -> SysResult<Vec<String>> {
    let c_path = to_cstring(path)?;
    let dirp = (syscalls().opendir)(c_path.as_ptr());
    if dirp.is_null() {
        return Err(last_error());
    }

    let mut entries = Vec::new();
    loop {
        set_errno(0);
        let ent = (syscalls().readdir)(dirp);
        if ent.is_null() {
            let errno = last_error();
            (syscalls().closedir)(dirp);
            if errno.errno() == Some(0) {
                break;
            }
            return Err(errno);
        }
        let name = unsafe { CStr::from_ptr((*ent).d_name.as_ptr()) };
        let name = name.to_string_lossy().into_owned();
        if name != "." && name != ".." {
            entries.push(name);
        }
    }
    Ok(entries)
}

pub fn canonicalize(path: &str) -> SysResult<String> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().realpath)(c_path.as_ptr(), std::ptr::null_mut());
    if result.is_null() {
        Err(last_error())
    } else {
        let s = unsafe { CStr::from_ptr(result) }.to_string_lossy().into_owned();
        unsafe { libc::free(result.cast()) };
        Ok(s)
    }
}

pub fn read_link(path: &str) -> SysResult<String> {
    let c_path = to_cstring(path)?;
    let mut buf = vec![0u8; 4096];
    let result = (syscalls().readlink)(c_path.as_ptr(), buf.as_mut_ptr().cast(), buf.len());
    if result < 0 {
        Err(last_error())
    } else {
        buf.truncate(result as usize);
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }
}

pub fn unlink_file(path: &str) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (syscalls().unlink)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn read_file(path: &str) -> SysResult<String> {
    let fd = open_file(path, O_RDONLY | O_CLOEXEC, 0)?;
    let mut contents = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = read_fd(fd, &mut buf)?;
        if n == 0 {
            break;
        }
        contents.extend_from_slice(&buf[..n]);
    }
    close_fd(fd)?;
    Ok(String::from_utf8_lossy(&contents).into_owned())
}

pub fn open_for_redirect(path: &str, flags: c_int, mode: mode_t, noclobber: bool) -> SysResult<c_int> {
    let actual_flags = if noclobber && (flags & O_TRUNC != 0) {
        (flags & !O_TRUNC) | O_EXCL | O_CREAT
    } else {
        flags
    };
    open_file(path, actual_flags, mode)
}

// --- Process wrapper functions ---

#[derive(Clone, Debug)]
pub struct ChildHandle {
    pub pid: Pid,
    pub stdout_fd: Option<c_int>,
}

pub struct ChildOutput {
    pub status: ChildExitStatus,
    pub stdout: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub struct ChildExitStatus {
    code: i32,
}

impl ChildExitStatus {
    pub fn success(&self) -> bool {
        self.code == 0
    }

    pub fn code(&self) -> Option<i32> {
        Some(self.code)
    }
}

impl ChildHandle {
    pub fn wait_with_output(self) -> SysResult<ChildOutput> {
        let mut output = Vec::new();
        if let Some(fd) = self.stdout_fd {
            let mut buf = [0u8; 8192];
            loop {
                let n = read_fd(fd, &mut buf)?;
                if n == 0 {
                    break;
                }
                output.extend_from_slice(&buf[..n]);
            }
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildOutput {
            status: ChildExitStatus { code: decode_wait_status(ws.status) },
            stdout: output,
        })
    }

    pub fn wait(self) -> SysResult<ChildExitStatus> {
        if let Some(fd) = self.stdout_fd {
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildExitStatus { code: decode_wait_status(ws.status) })
    }
}

pub fn fork_process() -> SysResult<Pid> {
    let pid = (syscalls().fork)();
    if pid < 0 {
        Err(last_error())
    } else {
        Ok(pid)
    }
}

pub fn exit_process(status: c_int) -> ! {
    (syscalls().exit_process)(status);
    unreachable!()
}

pub fn spawn_child(
    program: &str,
    argv: &[&str],
    env_vars: Option<&[(&str, &str)]>,
    redirections: &[(c_int, c_int)],
    stdin_fd: Option<c_int>,
    pipe_stdout: bool,
    process_group: Option<Pid>,
) -> SysResult<ChildHandle> {
    let stdout_pipe = if pipe_stdout {
        let (r, w) = create_pipe()?;
        Some((r, w))
    } else {
        None
    };

    let pid = fork_process()?;
    if pid == 0 {
        // Child process
        if let Some(pgid) = process_group {
            let _ = set_process_group(0, pgid);
        }
        if let Some(fd) = stdin_fd {
            let _ = duplicate_fd(fd, STDIN_FILENO);
            let _ = close_fd(fd);
        }
        if let Some((r, w)) = stdout_pipe {
            let _ = close_fd(r);
            let _ = duplicate_fd(w, STDOUT_FILENO);
            let _ = close_fd(w);
        }
        for &(src, dst) in redirections {
            let _ = duplicate_fd(src, dst);
            if src != dst {
                let _ = close_fd(src);
            }
        }
        if let Some(vars) = env_vars {
            for &(key, value) in vars {
                env_set_var(key, value);
            }
        }
        let rest: Vec<String> = argv.get(1..).unwrap_or(&[]).iter().map(|s| s.to_string()).collect();
        let _ = exec_replace(program, &rest);
        exit_process(127);
    }

    // Parent process
    if let Some(fd) = stdin_fd {
        let _ = close_fd(fd);
    }
    let stdout_read = if let Some((r, w)) = stdout_pipe {
        let _ = close_fd(w);
        Some(r)
    } else {
        None
    };

    Ok(ChildHandle { pid, stdout_fd: stdout_read })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessTimes {
    pub user_ticks: u64,
    pub system_ticks: u64,
    pub child_user_ticks: u64,
    pub child_system_ticks: u64,
}

pub fn current_umask() -> FileModeMask {
    let mask = (syscalls().umask)(0);
    (syscalls().umask)(mask);
    mask & 0o777
}

pub fn set_umask(mask: FileModeMask) -> FileModeMask {
    (syscalls().umask)(mask & 0o777) & 0o777
}

pub fn process_times() -> SysResult<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = (syscalls().times)(raw.as_mut_ptr());
    if result == ClockTicks::MAX {
        return Err(last_error());
    }
    let raw = unsafe { raw.assume_init() };
    Ok(ProcessTimes {
        user_ticks: raw.tms_utime as u64,
        system_ticks: raw.tms_stime as u64,
        child_user_ticks: raw.tms_cutime as u64,
        child_system_ticks: raw.tms_cstime as u64,
    })
}

pub fn clock_ticks_per_second() -> SysResult<u64> {
    let result = (syscalls().sysconf)(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(last_error())
    }
}

/// Execute a program, replacing the current process image.
/// `program` is the file to exec and becomes argv[0].
/// `argv` contains the remaining arguments (argv[1..]).
pub fn exec_replace(program: &str, argv: &[String]) -> SysResult<()> {
    let mut owned = Vec::with_capacity(argv.len() + 1);
    owned.push(CString::new(program).map_err(|_| SysError::NulInPath)?);
    for arg in argv {
        owned.push(CString::new(arg.as_str()).map_err(|_| SysError::NulInPath)?);
    }

    let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());

    let result = (syscalls().execvp)(owned[0].as_ptr(), pointers.as_ptr());
    if result == -1 {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn decode_wait_status(status: c_int) -> i32 {
    if wifexited(status) {
        wexitstatus(status)
    } else if wifsignaled(status) {
        128 + wtermsig(status)
    } else {
        status
    }
}

pub fn format_signal_exit(status: c_int) -> Option<String> {
    if wifsignaled(status) {
        Some(format!("terminated by signal {}", wtermsig(status)))
    } else {
        None
    }
}

fn signal_mask(signal: c_int) -> Option<usize> {
    let bit = match signal {
        SIGHUP => 0,
        SIGINT => 1,
        SIGQUIT => 2,
        SIGABRT => 3,
        SIGALRM => 4,
        SIGTERM => 5,
        _ => return None,
    };
    Some(1usize << bit)
}

fn wifexited(status: c_int) -> bool {
    (status & 0x7f) == 0
}

fn wexitstatus(status: c_int) -> i32 {
    (status >> 8) & 0xff
}

fn wifsignaled(status: c_int) -> bool {
    (status & 0x7f) != 0 && (status & 0x7f) != 0x7f
}

fn wtermsig(status: c_int) -> i32 {
    status & 0x7f
}

pub fn shell_name_from_args(args: &[String]) -> &str {
    args.first().map(String::as_str).unwrap_or("meiksh")
}

pub fn cstr_lossy(bytes: &[u8]) -> String {
    CStr::from_bytes_until_nul(bytes)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

#[allow(clippy::disallowed_methods)]
pub fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

#[allow(clippy::disallowed_methods)]
pub fn env_vars() -> std::collections::HashMap<String, String> {
    std::env::vars().collect()
}

#[allow(clippy::disallowed_methods)]
pub fn env_args_os() -> Vec<std::ffi::OsString> {
    std::env::args_os().collect()
}

pub fn env_set_var(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_roundtrip() {
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe { *fds.add(0) = 10; *fds.add(1) = 11; }
            0
        }
        fn fake_close(_fd: c_int) -> c_int { 0 }

        let fake = Syscalls { pipe: fake_pipe, close: fake_close, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            assert_eq!(read_fd, 10);
            assert_eq!(write_fd, 11);
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn decodes_wait_status_shapes() {
        assert_eq!(decode_wait_status(0), 0);
        assert_eq!(decode_wait_status(7 << 8), 7);
        assert_eq!(format_signal_exit(9), Some("terminated by signal 9".to_string()));
        assert_eq!(format_signal_exit(0), None);
    }

    #[test]
    fn shell_name_from_args_returns_first_arg_or_default() {
        assert_eq!(shell_name_from_args(&["meiksh".to_string(), "-c".to_string()]), "meiksh");
        assert_eq!(shell_name_from_args(&[]), "meiksh");
    }

    #[test]
    fn cstr_lossy_handles_nul_terminated_and_plain_bytes() {
        assert_eq!(cstr_lossy(b"abc\0rest"), "abc".to_string());
        assert_eq!(cstr_lossy(b"plain-bytes"), "plain-bytes".to_string());
    }

    #[test]
    fn execvp_failure_returns_minus_one() {
        fn fail_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int { -1 }
        let fake = Syscalls { execvp: fail_execvp, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let program = CString::new("meiksh-command-that-does-not-exist").expect("cstring");
            let argv = [program.as_ptr(), std::ptr::null()];
            assert_eq!((syscalls().execvp)(program.as_ptr(), argv.as_ptr()), -1);
        });
    }

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        fn fail_isatty(_fd: c_int) -> c_int { 0 }
        fn fail_dup2(_old: c_int, _new: c_int) -> c_int { -1 }
        fn fail_close(_fd: c_int) -> c_int { -1 }
        fn fail_tcgetpgrp(_fd: c_int) -> Pid { -1 }
        fn fail_tcsetpgrp(_fd: c_int, _pgid: Pid) -> c_int { -1 }
        fn fail_setpgid(_pid: Pid, _pgid: Pid) -> c_int { -1 }

        let fake = Syscalls {
            isatty: fail_isatty, dup2: fail_dup2, close: fail_close,
            tcgetpgrp: fail_tcgetpgrp, tcsetpgrp: fail_tcsetpgrp, setpgid: fail_setpgid,
            ..default_syscalls()
        };
        test_support::with_test_syscalls(fake, || {
            assert!(!is_interactive_fd(-1));
            assert!(duplicate_fd(-1, -1).is_err());
            assert!(close_fd(-1).is_err());
            assert!(current_foreground_pgrp(-1).is_err());
            assert!(set_foreground_pgrp(-1, 0).is_err());
            assert!(set_process_group(999_999, 999_999).is_err());
        });
    }

    #[test]
    fn wait_pid_error_surfaces_errno() {
        fn fail_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid { -1 }
        let fake = Syscalls { waitpid: fail_waitpid, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            assert!(wait_pid(999_999, false).is_err());
        });
    }

    #[test]
    fn exec_replace_rejects_nul_in_program_and_args() {
        let err = exec_replace("bad\0program", &[]).unwrap_err();
        assert_eq!(err, SysError::NulInPath);
        assert!(err.errno().is_none());
        assert!(!err.is_enoent());
        assert!(format!("{err}").contains("null"));

        let err = exec_replace("ok", &["bad\0arg".to_string()]).unwrap_err();
        assert_eq!(err, SysError::NulInPath);
    }

    #[test]
    fn sys_error_helper_methods_report_correct_variants() {
        let errno_err = SysError::Errno(libc::ENOENT);
        assert_eq!(errno_err.errno(), Some(libc::ENOENT));
        assert!(errno_err.is_enoent());
        assert!(!errno_err.is_ebadf());
        assert!(!errno_err.is_enoexec());
        assert!(!errno_err.is_eintr());
        assert!(!format!("{errno_err}").is_empty());

        let ebadf = SysError::Errno(libc::EBADF);
        assert!(ebadf.is_ebadf());
        let enoexec = SysError::Errno(libc::ENOEXEC);
        assert!(enoexec.is_enoexec());
        let eintr = SysError::Errno(EINTR);
        assert!(eintr.is_eintr());
    }

    #[test]
    fn sys_success_branches_cover_fd_helpers() {
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe { *fds.add(0) = 20; *fds.add(1) = 21; }
            0
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int { oldfd }
        fn fake_close(_fd: c_int) -> c_int { 0 }

        let fake = Syscalls { pipe: fake_pipe, dup2: fake_dup2, close: fake_close, ..default_syscalls() };
        test_support::with_test_syscalls(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            duplicate_fd(read_fd, read_fd).expect("dup self");
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn process_identity_helper_covers_mismatch_branch() {
        assert!(!test_support::with_process_ids_for_test((1, 2, 3, 3), has_same_real_and_effective_ids));
        assert!(!test_support::with_process_ids_for_test((1, 1, 3, 4), has_same_real_and_effective_ids));
    }

    #[test]
    fn success_process_identity() {
        fn fake_getpid() -> Pid {
            4242
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 4242);
        });
    }

    #[test]
    fn success_wait_and_signal() {
        fn fake_waitpid(_pid: Pid, status: *mut c_int, _options: c_int) -> Pid {
            unsafe {
                *status = 9 << 8;
            }
            99
        }
        fn fake_kill(_pid: Pid, _sig: c_int) -> c_int {
            0
        }
        fn fake_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            0
        }

        let fake = Syscalls {
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(
                wait_pid(1, false).expect("wait").expect("status"),
                WaitStatus { pid: 99, status: 9 << 8 }
            );
            assert!(send_signal(1, 0).is_ok());
        });
    }

    #[test]
    fn success_terminal_control() {
        fn fake_isatty(_fd: c_int) -> c_int {
            1
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            77
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            0
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            0
        }

        let fake = Syscalls {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(is_interactive_fd(0));
            assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
            assert!(set_foreground_pgrp(0, 77).is_ok());
            assert!(set_process_group(1, 1).is_ok());
        });
    }

    #[test]
    fn success_pipe_and_fd() {
        fn fake_pipe(fds: *mut c_int) -> c_int {
            unsafe {
                *fds.add(0) = 10;
                *fds.add(1) = 11;
            }
            0
        }
        fn fake_dup(fd: c_int) -> c_int {
            fd + 100
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int {
            oldfd
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = Syscalls {
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(create_pipe().expect("pipe"), (10, 11));
            assert_eq!(duplicate_fd_to_new(4).expect("dup"), 104);
            assert!(duplicate_fd(4, 5).is_ok());
            assert!(close_fd(4).is_ok());
        });
    }

    #[test]
    fn success_file_io() {
        fn fake_fcntl(_fd: c_int, cmd: c_int, arg: c_int) -> c_int {
            match cmd {
                F_GETFL => arg,
                F_SETFL => 0,
                _ => -1,
            }
        }
        fn fake_read(_fd: c_int, buf: *mut u8, count: usize) -> isize {
            if count == 0 {
                return 0;
            }
            unsafe {
                *buf = b'X';
            }
            1
        }

        let fake = Syscalls {
            fcntl: fake_fcntl,
            read: fake_read,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            let mut buffer = [0u8; 1];
            assert_eq!(read_fd(0, &mut buffer).expect("read"), 1);
            assert_eq!(buffer, [b'X']);
            let mut reader = FdReader::new(0);
            assert_eq!(reader.read(&mut buffer).expect("reader read"), 1);
            assert_eq!(buffer, [b'X']);
        });
    }

    #[test]
    fn success_umask_times_sysconf() {
        fn fake_umask(mask: FileModeMask) -> FileModeMask {
            mask
        }
        fn fake_times(buffer: *mut libc::tms) -> ClockTicks {
            unsafe {
                (*buffer).tms_utime = 10;
                (*buffer).tms_stime = 20;
                (*buffer).tms_cutime = 30;
                (*buffer).tms_cstime = 40;
            }
            99
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            60
        }

        let fake = Syscalls {
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_umask(), 0);
            assert_eq!(set_umask(0o027), 0o027);
            assert_eq!(
                process_times().expect("times"),
                ProcessTimes {
                    user_ticks: 10,
                    system_ticks: 20,
                    child_user_ticks: 30,
                    child_system_ticks: 40,
                }
            );
            assert_eq!(clock_ticks_per_second().expect("ticks"), 60);
        });
    }

    #[test]
    fn success_exec() {
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            0
        }

        let fake = Syscalls {
            execvp: fake_execvp,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(exec_replace("echo", &["hello".to_string(), "world".to_string()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        assert_eq!(decode_wait_status(0x7f), 0x7f);
    }

    #[test]
    fn signal_handler_installation_succeeds() {
        use test_support::{run_trace, t, TraceResult, ArgMatcher};

        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(SIGINT as i64), ArgMatcher::Any], TraceResult::Int(0)),
            t("signal", vec![ArgMatcher::Int(SIGTERM as i64), ArgMatcher::Any], TraceResult::Int(0)),
            t("signal", vec![ArgMatcher::Int(SIGQUIT as i64), ArgMatcher::Any], TraceResult::Int(0)),
        ], || {
            install_shell_signal_handler(SIGINT).expect("install");
            ignore_signal(SIGTERM).expect("ignore");
            default_signal_action(SIGQUIT).expect("default");
        });
    }

    #[test]
    fn signal_handler_error_paths() {
        use test_support::{run_trace, t, TraceResult, ArgMatcher};

        run_trace(vec![
            t("signal", vec![ArgMatcher::Int(SIGINT as i64), ArgMatcher::Any], TraceResult::Err(libc::EINVAL)),
            t("signal", vec![ArgMatcher::Int(SIGTERM as i64), ArgMatcher::Any], TraceResult::Err(libc::EINVAL)),
            t("signal", vec![ArgMatcher::Int(SIGQUIT as i64), ArgMatcher::Any], TraceResult::Err(libc::EINVAL)),
        ], || {
            assert!(install_shell_signal_handler(SIGINT).is_err());
            assert!(ignore_signal(SIGTERM).is_err());
            assert!(default_signal_action(SIGQUIT).is_err());
        });
    }

    #[test]
    fn pending_signal_tracking() {
        test_support::with_pending_signals_for_test(&[SIGINT], || {
            assert_eq!(has_pending_signal(), Some(SIGINT));
            assert_eq!(take_pending_signals(), vec![SIGINT]);
        });
        test_support::with_pending_signals_for_test(&[99], || {
            assert_eq!(has_pending_signal(), None);
        });
    }

    #[test]
    fn signal_utility_helpers() {
        let interrupted_error = SysError::Errno(EINTR);
        assert!(interrupted(&interrupted_error));
        assert_eq!(supported_trap_signals(), vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]);
    }

    #[test]
    fn error_process_identity() {
        fn fake_getpid() -> Pid {
            1
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 1);
        });
    }

    #[test]
    fn error_wait_and_signal() {
        fn fake_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid {
            -1
        }
        fn fake_kill(_pid: Pid, _sig: c_int) -> c_int {
            -1
        }

        let fake = Syscalls {
            waitpid: fake_waitpid,
            kill: fake_kill,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(send_signal(1, 0).is_err());
            assert!(wait_pid(1, false).is_err());
        });
    }

    #[test]
    fn error_terminal_control() {
        fn fake_isatty(_fd: c_int) -> c_int {
            0
        }
        fn fake_tcgetpgrp(_fd: c_int) -> Pid {
            -1
        }
        fn fake_tcsetpgrp(_fd: c_int, _pgrp: Pid) -> c_int {
            -1
        }
        fn fake_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            -1
        }

        let fake = Syscalls {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(!is_interactive_fd(0));
            assert!(current_foreground_pgrp(0).is_err());
            assert!(set_foreground_pgrp(0, 1).is_err());
            assert!(set_process_group(1, 1).is_err());
        });
    }

    #[test]
    fn error_pipe_and_fd() {
        fn fake_pipe(_fds: *mut c_int) -> c_int {
            -1
        }
        fn fake_dup(_fd: c_int) -> c_int {
            -1
        }
        fn fake_dup2(_oldfd: c_int, _newfd: c_int) -> c_int {
            -1
        }
        fn fake_close(_fd: c_int) -> c_int {
            -1
        }

        let fake = Syscalls {
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(create_pipe().is_err());
            assert!(duplicate_fd_to_new(1).is_err());
            assert!(duplicate_fd(1, 2).is_err());
            assert!(close_fd(1).is_err());
        });
    }

    #[test]
    fn error_file_io() {
        fn fake_read(_fd: c_int, _buf: *mut u8, _count: usize) -> isize {
            -1
        }

        let fake = Syscalls {
            read: fake_read,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(read_fd(0, &mut [0u8; 1]).is_err());
        });
    }

    #[test]
    fn error_times_sysconf() {
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            -1
        }

        let fake = Syscalls {
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(process_times().is_err());
            assert!(clock_ticks_per_second().is_err());
        });
    }

    #[test]
    fn error_exec() {
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }

        let fake = Syscalls {
            execvp: fake_execvp,
            ..default_syscalls()
        };

        test_support::with_test_syscalls(fake, || {
            assert!(exec_replace("echo", &["hi".to_string()]).is_err());
        });
    }

    #[test]
    fn decode_wait_status_signal_terminated() {
        assert_eq!(decode_wait_status(9), 137);
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_tty() {
        use test_support::{run_trace, t, TraceResult, ArgMatcher};

        // TTY path: isatty→1, fcntl F_GETFL→O_NONBLOCK|2, fcntl F_SETFL→0
        run_trace(vec![
            t("isatty", vec![ArgMatcher::Fd(STDIN_FILENO)], TraceResult::Int(1)),
            t("fcntl", vec![ArgMatcher::Fd(STDIN_FILENO), ArgMatcher::Int(F_GETFL as i64), ArgMatcher::Int(0)], TraceResult::Int((O_NONBLOCK | 0o2) as i64)),
            t("fcntl", vec![ArgMatcher::Fd(STDIN_FILENO), ArgMatcher::Int(F_SETFL as i64), ArgMatcher::Int(0o2)], TraceResult::Int(0)),
        ], || {
            ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
        });
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_fifo() {
        use test_support::{run_trace, t, TraceResult, ArgMatcher};

        // FIFO path: isatty→0, fstat→S_IFIFO, fcntl F_GETFL→O_NONBLOCK|2, fcntl F_SETFL→0
        run_trace(vec![
            t("isatty", vec![ArgMatcher::Fd(42)], TraceResult::Int(0)),
            t("fstat", vec![ArgMatcher::Fd(42), ArgMatcher::Any], TraceResult::StatFifo),
            t("fcntl", vec![ArgMatcher::Fd(42), ArgMatcher::Int(F_GETFL as i64), ArgMatcher::Int(0)], TraceResult::Int((O_NONBLOCK | 0o2) as i64)),
            t("fcntl", vec![ArgMatcher::Fd(42), ArgMatcher::Int(F_SETFL as i64), ArgMatcher::Int(0o2)], TraceResult::Int(0)),
        ], || {
            ensure_blocking_read_fd(42).expect("fifo blocking");
        });
    }

    #[test]
    fn ensure_blocking_read_fd_surfaces_fcntl_errors() {
        use test_support::{run_trace, t, TraceResult, ArgMatcher};

        run_trace(vec![
            t("isatty", vec![ArgMatcher::Fd(STDIN_FILENO)], TraceResult::Int(1)),
            t("fcntl", vec![ArgMatcher::Fd(STDIN_FILENO), ArgMatcher::Int(F_GETFL as i64), ArgMatcher::Int(0)], TraceResult::Err(libc::EIO)),
        ], || {
            assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
        });
    }
}
