macro_rules! sys_println {
    () => {
        let _ = $crate::sys::write_all_fd($crate::sys::STDOUT_FILENO, b"\n");
    };
    ($($arg:tt)*) => {{
        let msg = format!("{}\n", format_args!($($arg)*));
        let _ = $crate::sys::write_all_fd($crate::sys::STDOUT_FILENO, msg.as_bytes());
    }};
}

macro_rules! sys_eprintln {
    ($($arg:tt)*) => {{
        let msg = format!("{}\n", format_args!($($arg)*));
        let _ = $crate::sys::write_all_fd($crate::sys::STDERR_FILENO, msg.as_bytes());
    }};
}

use libc::{self, c_char, c_int, c_long, mode_t};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicUsize, Ordering};

pub type Pid = libc::pid_t;
pub type RawFd = c_int;
pub type FileModeMask = libc::mode_t;
type ClockTicks = libc::clock_t;

const SC_CLK_TCK: c_int = libc::_SC_CLK_TCK;
const F_GETFL: c_int = libc::F_GETFL;
const F_SETFL: c_int = libc::F_SETFL;
const F_DUPFD_CLOEXEC: c_int = libc::F_DUPFD_CLOEXEC;
const O_NONBLOCK: c_int = libc::O_NONBLOCK;

pub const STDIN_FILENO: c_int = libc::STDIN_FILENO;
pub const STDOUT_FILENO: c_int = libc::STDOUT_FILENO;
pub const STDERR_FILENO: c_int = libc::STDERR_FILENO;
pub const SIGHUP: c_int = libc::SIGHUP;
pub const SIGINT: c_int = libc::SIGINT;
pub const SIGQUIT: c_int = libc::SIGQUIT;
pub const SIGILL: c_int = libc::SIGILL;
pub const SIGABRT: c_int = libc::SIGABRT;
pub const SIGFPE: c_int = libc::SIGFPE;
pub const SIGKILL: c_int = libc::SIGKILL;
pub const SIGUSR1: c_int = libc::SIGUSR1;
pub const SIGSEGV: c_int = libc::SIGSEGV;
pub const SIGUSR2: c_int = libc::SIGUSR2;
pub const SIGPIPE: c_int = libc::SIGPIPE;
pub const SIGALRM: c_int = libc::SIGALRM;
pub const SIGSTOP: c_int = libc::SIGSTOP;
pub const SIGCONT: c_int = libc::SIGCONT;
pub const SIGTERM: c_int = libc::SIGTERM;
pub const SIGTRAP: c_int = libc::SIGTRAP;
pub const SIGCHLD: c_int = libc::SIGCHLD;
pub const SIGTSTP: c_int = libc::SIGTSTP;
pub const SIGTTIN: c_int = libc::SIGTTIN;
pub const SIGTTOU: c_int = libc::SIGTTOU;
pub const SIGBUS: c_int = libc::SIGBUS;
pub const SIGSYS: c_int = libc::SIGSYS;
pub const WNOHANG: c_int = libc::WNOHANG;
pub const WUNTRACED: c_int = libc::WUNTRACED;
pub const WCONTINUED: c_int = libc::WCONTINUED;
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
pub const ESRCH: c_int = libc::ESRCH;

const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

pub const TCSADRAIN: c_int = libc::TCSADRAIN;

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

    pub fn is_eacces(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::EACCES)
    }

    pub fn is_enoexec(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::ENOEXEC)
    }

    pub fn is_eintr(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == EINTR)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SystemInterface {
    getpid: fn() -> Pid,
    getppid: fn() -> Pid,
    waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    kill: fn(Pid, c_int) -> c_int,
    signal: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
    isatty: fn(c_int) -> c_int,
    tcgetpgrp: fn(c_int) -> Pid,
    tcsetpgrp: fn(c_int, Pid) -> c_int,
    setpgid: fn(Pid, Pid) -> c_int,
    pipe: fn(&mut [c_int; 2]) -> c_int,
    dup2: fn(c_int, c_int) -> c_int,
    close: fn(c_int) -> c_int,
    fcntl: fn(c_int, c_int, c_int) -> c_int,
    read: fn(c_int, &mut [u8]) -> isize,
    umask: fn(FileModeMask) -> FileModeMask,
    times: fn(*mut libc::tms) -> ClockTicks,
    sysconf: fn(c_int) -> c_long,
    execvp: fn(*const c_char, *const *const c_char) -> c_int,
    execve: fn(*const c_char, *const *const c_char, *const *const c_char) -> c_int,
    // Filesystem
    open: fn(*const c_char, c_int, mode_t) -> c_int,
    write: fn(c_int, &[u8]) -> isize,
    stat: fn(*const c_char, *mut libc::stat) -> c_int,
    fstat: fn(c_int, *mut libc::stat) -> c_int,
    access: fn(*const c_char, c_int) -> c_int,
    chdir: fn(*const c_char) -> c_int,
    getcwd: fn(*mut c_char, usize) -> *mut c_char,
    opendir: fn(*const c_char) -> *mut libc::DIR,
    readdir: fn(*mut libc::DIR) -> *mut libc::dirent,
    closedir: fn(*mut libc::DIR) -> c_int,
    realpath: fn(*const c_char, *mut c_char) -> *mut c_char,
    // Process
    fork: fn() -> Pid,
    exit_process: fn(c_int),
    // Environment
    setenv: fn(&str, &str) -> SysResult<()>,
    unsetenv: fn(&str) -> SysResult<()>,
    getenv: fn(&str) -> Option<String>,
    get_environ: fn() -> HashMap<String, String>,
    // Terminal attributes
    tcgetattr: fn(c_int, *mut libc::termios) -> c_int,
    tcsetattr: fn(c_int, c_int, *const libc::termios) -> c_int,
    // User database
    getpwnam: fn(&str) -> Option<String>,
    // Signal state
    pending_signal_bits: fn() -> usize,
    take_pending_signal_bits: fn() -> usize,
    monotonic_clock_ns: fn() -> u64,
    // Locale
    setup_locale: fn(),
    classify_char: fn(&str, char) -> bool,
}

pub(crate) fn default_interface() -> SystemInterface {
    SystemInterface {
        getpid: || unsafe { libc::getpid() },
        getppid: || unsafe { libc::getppid() },
        waitpid: |pid, status, options| unsafe { libc::waitpid(pid, status, options) },
        kill: |pid, sig| unsafe { libc::kill(pid, sig) },
        signal: |sig, handler| unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handler;
            libc::sigemptyset(&mut sa.sa_mask);
            let mut old_sa: libc::sigaction = std::mem::zeroed();
            let rc = libc::sigaction(sig, &sa, &mut old_sa);
            [old_sa.sa_sigaction, libc::SIG_ERR][(rc < 0) as usize]
        },
        isatty: |fd| unsafe { libc::isatty(fd) },
        tcgetpgrp: |fd| unsafe { libc::tcgetpgrp(fd) },
        tcsetpgrp: |fd, pgrp| unsafe { libc::tcsetpgrp(fd, pgrp) },
        setpgid: |pid, pgid| unsafe { libc::setpgid(pid, pgid) },
        pipe: |fds| unsafe { libc::pipe(fds.as_mut_ptr()) },
        dup2: |oldfd, newfd| unsafe { libc::dup2(oldfd, newfd) },
        close: |fd| unsafe { libc::close(fd) },
        fcntl: |fd, cmd, arg| unsafe { libc::fcntl(fd, cmd, arg) },
        read: |fd, buf| unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) },
        umask: |cmask| unsafe { libc::umask(cmask) },
        times: |buffer| unsafe { libc::times(buffer) },
        sysconf: |name| unsafe { libc::sysconf(name) },
        execvp: |file, argv| unsafe { libc::execvp(file, argv) },
        execve: |file, argv, envp| unsafe { libc::execve(file, argv, envp) },
        open: |path, flags, mode| unsafe { libc::open(path, flags, mode as c_int) },
        write: |fd, data| unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) },
        stat: |path, buf| unsafe { libc::stat(path, buf) },
        fstat: |fd, buf| unsafe { libc::fstat(fd, buf) },
        access: |path, mode| unsafe { libc::access(path, mode) },
        chdir: |path| unsafe { libc::chdir(path) },
        getcwd: |buf, size| unsafe { libc::getcwd(buf, size) },
        opendir: |path| unsafe { libc::opendir(path) },
        readdir: |dirp| unsafe { libc::readdir(dirp) },
        closedir: |dirp| unsafe { libc::closedir(dirp) },
        realpath: |path, resolved| unsafe { libc::realpath(path, resolved) },
        fork: || unsafe { libc::fork() },
        exit_process: |status| {
            #[cfg(coverage)]
            flush_coverage();
            unsafe { libc::_exit(status) }
        },
        setenv: |key, value| {
            let c_key = CString::new(key).map_err(|_| SysError::NulInPath)?;
            let c_value = CString::new(value).map_err(|_| SysError::NulInPath)?;
            let result = unsafe { libc::setenv(c_key.as_ptr(), c_value.as_ptr(), 1) };
            if result == 0 {
                Ok(())
            } else {
                Err(last_error())
            }
        },
        unsetenv: |key| {
            let c_key = CString::new(key).map_err(|_| SysError::NulInPath)?;
            let result = unsafe { libc::unsetenv(c_key.as_ptr()) };
            if result == 0 {
                Ok(())
            } else {
                Err(last_error())
            }
        },
        getenv: |key| {
            let c_key = CString::new(key).ok()?;
            let ptr = unsafe { libc::getenv(c_key.as_ptr()) };
            if ptr.is_null() {
                None
            } else {
                Some(
                    unsafe { CStr::from_ptr(ptr) }
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        },
        get_environ: || {
            unsafe extern "C" {
                static environ: *const *const c_char;
            }
            let mut map = HashMap::new();
            unsafe {
                let mut ptr = environ;
                while !(*ptr).is_null() {
                    let entry = CStr::from_ptr(*ptr).to_string_lossy();
                    if let Some((key, value)) = entry.split_once('=') {
                        map.insert(key.to_string(), value.to_string());
                    }
                    ptr = ptr.add(1);
                }
            }
            map
        },
        tcgetattr: |fd, termios_p| unsafe { libc::tcgetattr(fd, termios_p) },
        tcsetattr: |fd, action, termios_p| unsafe { libc::tcsetattr(fd, action, termios_p) },
        getpwnam: |name| {
            let c_name = CString::new(name).ok()?;
            let pw = unsafe { libc::getpwnam(c_name.as_ptr()) };
            if pw.is_null() {
                return None;
            }
            let dir = unsafe { CStr::from_ptr((*pw).pw_dir) };
            Some(dir.to_string_lossy().into_owned())
        },
        pending_signal_bits: || PENDING_SIGNALS.load(Ordering::SeqCst),
        take_pending_signal_bits: || PENDING_SIGNALS.swap(0, Ordering::SeqCst),
        monotonic_clock_ns: || {
            let mut ts = std::mem::MaybeUninit::<libc::timespec>::zeroed();
            unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, ts.as_mut_ptr()) };
            let ts = unsafe { ts.assume_init() };
            ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
        },
        setup_locale: || unsafe {
            libc::setlocale(libc::LC_ALL, b"\0".as_ptr().cast());
        },
        classify_char: |class, ch| {
            unsafe extern "C" {
                fn iswalnum(wc: u32) -> c_int;
                fn iswalpha(wc: u32) -> c_int;
                fn iswblank(wc: u32) -> c_int;
                fn iswcntrl(wc: u32) -> c_int;
                fn iswdigit(wc: u32) -> c_int;
                fn iswgraph(wc: u32) -> c_int;
                fn iswlower(wc: u32) -> c_int;
                fn iswprint(wc: u32) -> c_int;
                fn iswpunct(wc: u32) -> c_int;
                fn iswspace(wc: u32) -> c_int;
                fn iswupper(wc: u32) -> c_int;
                fn iswxdigit(wc: u32) -> c_int;
            }
            let wc = ch as u32;
            unsafe {
                match class {
                    "alnum" => iswalnum(wc) != 0,
                    "alpha" => iswalpha(wc) != 0,
                    "blank" => iswblank(wc) != 0,
                    "cntrl" => iswcntrl(wc) != 0,
                    "digit" => iswdigit(wc) != 0,
                    "graph" => iswgraph(wc) != 0,
                    "lower" => iswlower(wc) != 0,
                    "print" => iswprint(wc) != 0,
                    "punct" => iswpunct(wc) != 0,
                    "space" => iswspace(wc) != 0,
                    "upper" => iswupper(wc) != 0,
                    "xdigit" => iswxdigit(wc) != 0,
                    _ => false,
                }
            }
        },
    }
}

static PENDING_SIGNALS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn record_signal(sig: c_int) {
    if let Some(mask) = signal_mask(sig) {
        PENDING_SIGNALS.fetch_or(mask, Ordering::SeqCst);
    }
}

fn sys_interface() -> SystemInterface {
    #[cfg(test)]
    {
        return test_support::current_interface()
            .expect("sys call without run_trace or assert_no_syscalls");
    }

    #[cfg(not(test))]
    {
        default_interface()
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
}

fn last_error() -> SysError {
    #[cfg(test)]
    {
        return test_support::take_test_error();
    }

    #[cfg(not(test))]
    SysError::Errno(unsafe { *errno_ptr() })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    thread_local! {
        static TEST_INTERFACE: RefCell<Option<SystemInterface>> = const { RefCell::new(None) };
        static TEST_ERRNO: RefCell<c_int> = const { RefCell::new(0) };
        static TEST_PENDING_SIGNALS: RefCell<usize> = const { RefCell::new(0) };
        static TEST_PROCESS_IDS: RefCell<Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)>> =
            const { RefCell::new(None) };
    }

    fn syscall_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    pub(crate) fn current_interface() -> Option<SystemInterface> {
        TEST_INTERFACE.with(|cell| *cell.borrow())
    }

    pub(crate) fn current_process_ids()
    -> Option<(libc::uid_t, libc::uid_t, libc::gid_t, libc::gid_t)> {
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

    pub(crate) fn with_test_interface<T>(iface: SystemInterface, f: impl FnOnce() -> T) -> T {
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
        Str(String),
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
        CwdStr(String),
        RealpathStr(String),
        StatDir,
        StatFile(mode_t),
        StatFifo,
        DirEntry(String),
        Str(String),
        NullStr,
        EnvMap(HashMap<String, String>),
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
        super::set_errno(super::EINTR);
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
                super::set_errno(*errno);
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
                super::set_errno(*errno);
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
                super::set_errno(*errno);
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
                super::set_errno(*errno);
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
                super::set_errno(*errno);
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
    fn trace_setup_locale() {}
    fn trace_classify_char(class: &str, ch: char) -> bool {
        ch.is_ascii_alphabetic() && class == "alpha"
            || ch.is_ascii_alphanumeric() && class == "alnum"
            || ch.is_ascii_digit() && class == "digit"
            || ch.is_ascii_lowercase() && class == "lower"
            || ch.is_ascii_uppercase() && class == "upper"
            || (ch == ' ' || ch == '\t') && class == "blank"
            || ch.is_ascii_whitespace() && class == "space"
            || ch.is_ascii_hexdigit() && class == "xdigit"
            || ch.is_ascii_punctuation() && class == "punct"
            || ch.is_ascii_graphic() && class == "graph"
            || (ch.is_ascii_graphic() || ch == ' ') && class == "print"
            || ch.is_ascii_control() && class == "cntrl"
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
        let name = unsafe { CStr::from_ptr(file) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch("execvp", &[ArgMatcher::Str(name), ArgMatcher::Any]);
        apply_trace_result_int(&entry)
    }
    fn trace_execve(
        file: *const c_char,
        _argv: *const *const c_char,
        _envp: *const *const c_char,
    ) -> c_int {
        let name = unsafe { CStr::from_ptr(file) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch("execve", &[ArgMatcher::Str(name), ArgMatcher::Any]);
        apply_trace_result_int(&entry)
    }
    fn trace_open(path: *const c_char, flags: c_int, mode: mode_t) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
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
    fn trace_stat(path: *const c_char, buf: *mut libc::stat) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch("stat", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::StatDir => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFDIR | 0o755;
                }
                0
            }
            TraceResult::StatFile(mode) => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFREG | mode;
                }
                0
            }
            TraceResult::StatFifo => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFIFO | 0o644;
                }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            TraceResult::Int(v) => *v as c_int,
            other => panic!(
                "trace result type mismatch for 'stat': expected StatDir/StatFile/Err, got {other:?}"
            ),
        }
    }
    fn trace_fstat(fd: c_int, buf: *mut libc::stat) -> c_int {
        let entry = trace_dispatch("fstat", &[ArgMatcher::Fd(fd), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::StatDir => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFDIR | 0o755;
                }
                0
            }
            TraceResult::StatFile(mode) => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFREG | mode;
                }
                0
            }
            TraceResult::StatFifo => {
                unsafe {
                    std::ptr::write_bytes(buf, 0, 1);
                    (*buf).st_mode = libc::S_IFIFO | 0o644;
                }
                0
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                -1
            }
            TraceResult::Int(v) => *v as c_int,
            other => panic!(
                "trace result type mismatch for 'fstat': expected StatDir/StatFile/StatFifo/Err, got {other:?}"
            ),
        }
    }
    fn trace_access(path: *const c_char, mode: c_int) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch(
            "access",
            &[ArgMatcher::Str(p), ArgMatcher::Int(mode as i64)],
        );
        apply_trace_result_int(&entry)
    }
    fn trace_chdir(path: *const c_char) -> c_int {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
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
            other => panic!(
                "trace result type mismatch for 'getcwd': expected CwdStr/Err, got {other:?}"
            ),
        }
    }
    fn trace_opendir(path: *const c_char) -> *mut libc::DIR {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch("opendir", &[ArgMatcher::Str(p)]);
        match &entry.result {
            TraceResult::Int(v) => *v as *mut libc::DIR,
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
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
            TraceResult::DirEntry(name) => FAKE_DIRENT.with(|cell| {
                let mut d = cell.borrow_mut();
                d.d_name = unsafe { std::mem::zeroed() };
                let bytes = name.as_bytes();
                let len = bytes.len().min(d.d_name.len() - 1);
                for (i, &b) in bytes[..len].iter().enumerate() {
                    d.d_name[i] = b as i8;
                }
                d.d_name[len] = 0;
                &mut *d as *mut libc::dirent
            }),
            TraceResult::Int(0) => {
                super::set_errno(0);
                std::ptr::null_mut()
            }
            TraceResult::Err(errno) => {
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!(
                "trace result type mismatch for 'readdir': expected DirEntry/Int(0)/Err, got {other:?}"
            ),
        }
    }
    fn trace_closedir(_dirp: *mut libc::DIR) -> c_int {
        let entry = trace_dispatch("closedir", &[ArgMatcher::Any]);
        apply_trace_result_int(&entry)
    }
    fn trace_realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
        let p = unsafe { CStr::from_ptr(path) }
            .to_string_lossy()
            .to_string();
        let entry = trace_dispatch("realpath", &[ArgMatcher::Str(p), ArgMatcher::Any]);
        match &entry.result {
            TraceResult::RealpathStr(s) => {
                let c_result = std::ffi::CString::new(s.as_str()).unwrap();
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
                super::set_errno(*errno);
                std::ptr::null_mut()
            }
            other => panic!(
                "trace result type mismatch for 'realpath': expected RealpathStr/Err, got {other:?}"
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

    fn trace_setenv(key: &str, value: &str) -> SysResult<()> {
        let entry = trace_dispatch(
            "setenv",
            &[
                ArgMatcher::Str(key.to_string()),
                ArgMatcher::Str(value.to_string()),
            ],
        );
        match entry.result {
            TraceResult::Int(0) => Ok(()),
            TraceResult::Err(errno) => Err(SysError::Errno(errno)),
            other => panic!("setenv trace: unexpected result {other:?}"),
        }
    }

    fn trace_unsetenv(key: &str) -> SysResult<()> {
        let entry = trace_dispatch("unsetenv", &[ArgMatcher::Str(key.to_string())]);
        match entry.result {
            TraceResult::Int(0) => Ok(()),
            TraceResult::Err(errno) => Err(SysError::Errno(errno)),
            other => panic!("unsetenv trace: unexpected result {other:?}"),
        }
    }

    fn trace_getenv(key: &str) -> Option<String> {
        let entry = trace_dispatch("getenv", &[ArgMatcher::Str(key.to_string())]);
        match entry.result {
            TraceResult::Str(s) => Some(s),
            TraceResult::NullStr => None,
            other => panic!("getenv trace: unexpected result {other:?}"),
        }
    }

    fn trace_get_environ() -> HashMap<String, String> {
        let entry = trace_dispatch("get_environ", &[]);
        match entry.result {
            TraceResult::EnvMap(map) => map,
            other => panic!("get_environ trace: unexpected result {other:?}"),
        }
    }

    fn trace_getpwnam(name: &str) -> Option<String> {
        let entry = trace_dispatch("getpwnam", &[ArgMatcher::Str(name.to_string())]);
        match entry.result {
            TraceResult::Str(s) => Some(s),
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

    pub(crate) fn trace_interface() -> SystemInterface {
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
            fstat: trace_fstat,
            access: trace_access,
            chdir: trace_chdir,
            getcwd: trace_getcwd,
            opendir: trace_opendir,
            readdir: trace_readdir,
            closedir: trace_closedir,
            realpath: trace_realpath,
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
            setup_locale: trace_setup_locale,
            classify_char: trace_classify_char,
        }
    }

    pub(crate) fn no_interface_table() -> SystemInterface {
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
        fn panic_execve(
            _: *const c_char,
            _: *const *const c_char,
            _: *const *const c_char,
        ) -> c_int {
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
        fn panic_realpath(_: *const c_char, _: *mut c_char) -> *mut c_char {
            panic!("unexpected syscall 'realpath' in pure-logic test")
        }
        fn panic_fork() -> Pid {
            panic!("unexpected syscall 'fork' in pure-logic test")
        }
        fn panic_exit_process(_: c_int) {
            panic!("unexpected syscall 'exit_process' in pure-logic test")
        }
        fn panic_setenv(_: &str, _: &str) -> SysResult<()> {
            panic!("unexpected call 'setenv' in pure-logic test")
        }
        fn panic_unsetenv(_: &str) -> SysResult<()> {
            panic!("unexpected call 'unsetenv' in pure-logic test")
        }
        fn panic_getenv(_: &str) -> Option<String> {
            panic!("unexpected call 'getenv' in pure-logic test")
        }
        fn panic_get_environ() -> HashMap<String, String> {
            panic!("unexpected call 'get_environ' in pure-logic test")
        }
        fn panic_tcgetattr(_: c_int, _: *mut libc::termios) -> c_int {
            panic!("unexpected syscall 'tcgetattr' in pure-logic test")
        }
        fn panic_tcsetattr(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
            panic!("unexpected syscall 'tcsetattr' in pure-logic test")
        }
        fn panic_getpwnam(_: &str) -> Option<String> {
            panic!("unexpected call 'getpwnam' in pure-logic test")
        }
        fn panic_monotonic_clock_ns() -> u64 {
            panic!("unexpected call 'monotonic_clock_ns' in pure-logic test")
        }
        fn panic_setup_locale() {
            panic!("unexpected call 'setup_locale' in pure-logic test")
        }
        fn ascii_classify_char(class: &str, ch: char) -> bool {
            match class {
                "alnum" => ch.is_ascii_alphanumeric(),
                "alpha" => ch.is_ascii_alphabetic(),
                "blank" => ch == ' ' || ch == '\t',
                "cntrl" => ch.is_ascii_control(),
                "digit" => ch.is_ascii_digit(),
                "graph" => ch.is_ascii_graphic(),
                "lower" => ch.is_ascii_lowercase(),
                "print" => ch.is_ascii_graphic() || ch == ' ',
                "punct" => ch.is_ascii_punctuation(),
                "space" => ch.is_ascii_whitespace(),
                "upper" => ch.is_ascii_uppercase(),
                "xdigit" => ch.is_ascii_hexdigit(),
                _ => false,
            }
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
            fstat: panic_fstat,
            access: panic_access,
            chdir: panic_chdir,
            getcwd: panic_getcwd,
            opendir: panic_opendir,
            readdir: panic_readdir,
            closedir: panic_closedir,
            realpath: panic_realpath,
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
            setup_locale: panic_setup_locale,
            classify_char: ascii_classify_char,
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
    pub(crate) fn t(
        syscall: &'static str,
        args: Vec<ArgMatcher>,
        result: TraceResult,
    ) -> TraceEntry {
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
            ArgMatcher::Str(self.to_string())
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitStatus {
    pub pid: Pid,
    pub status: c_int,
}

pub fn current_pid() -> Pid {
    (sys_interface().getpid)()
}

pub fn parent_pid() -> Pid {
    (sys_interface().getppid)()
}

pub fn setup_locale() {
    (sys_interface().setup_locale)()
}

pub fn classify_char(class: &str, ch: char) -> bool {
    (sys_interface().classify_char)(class, ch)
}

pub fn is_interactive_fd(fd: c_int) -> bool {
    (sys_interface().isatty)(fd) == 1
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
    let result = (sys_interface().waitpid)(pid, &mut status, options);
    if result > 0 {
        Ok(Some(WaitStatus {
            pid: result,
            status,
        }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(last_error())
    }
}

pub fn wait_pid_untraced(pid: Pid, _nohang: bool) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let result = (sys_interface().waitpid)(pid, &mut status, WUNTRACED);
    if result > 0 {
        Ok(Some(WaitStatus {
            pid: result,
            status,
        }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(last_error())
    }
}

pub fn wait_pid_job_status(pid: Pid) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let options = WUNTRACED | WCONTINUED | WNOHANG;
    let result = (sys_interface().waitpid)(pid, &mut status, options);
    if result > 0 {
        Ok(Some(WaitStatus {
            pid: result,
            status,
        }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(last_error())
    }
}

pub fn send_signal(pid: Pid, signal: c_int) -> SysResult<()> {
    let result = (sys_interface().kill)(pid, signal);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn install_shell_signal_handler(signal: c_int) -> SysResult<()> {
    let result = (sys_interface().signal)(signal, record_signal as *const () as libc::sighandler_t);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn ignore_signal(signal: c_int) -> SysResult<()> {
    let result = (sys_interface().signal)(signal, SIG_IGN_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn default_signal_action(signal: c_int) -> SysResult<()> {
    let result = (sys_interface().signal)(signal, SIG_DFL_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub fn has_pending_signal() -> Option<c_int> {
    let bits = (sys_interface().pending_signal_bits)();

    supported_trap_signals().into_iter().find(|signal| {
        signal_mask(*signal)
            .map(|mask| bits & mask != 0)
            .unwrap_or(false)
    })
}

pub fn take_pending_signals() -> Vec<c_int> {
    let bits = (sys_interface().take_pending_signal_bits)();

    supported_trap_signals()
        .into_iter()
        .filter(|signal| {
            signal_mask(*signal)
                .map(|mask| bits & mask != 0)
                .unwrap_or(false)
        })
        .collect()
}

pub fn supported_trap_signals() -> Vec<c_int> {
    vec![
        SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGABRT, SIGFPE, SIGBUS, SIGUSR1, SIGSEGV, SIGUSR2,
        SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT, SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGSYS,
    ]
}

pub fn query_signal_disposition(signal: c_int) -> SysResult<bool> {
    let prev = (sys_interface().signal)(signal, SIG_IGN_HANDLER);
    if prev == SIG_ERR_HANDLER {
        return Err(last_error());
    }
    let _ = (sys_interface().signal)(signal, prev);
    Ok(prev == SIG_IGN_HANDLER)
}

pub fn interrupted(error: &SysError) -> bool {
    error.is_eintr()
}

pub fn current_foreground_pgrp(fd: c_int) -> SysResult<Pid> {
    let result = (sys_interface().tcgetpgrp)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> SysResult<()> {
    let result = (sys_interface().tcsetpgrp)(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn set_process_group(pid: Pid, pgid: Pid) -> SysResult<()> {
    let result = (sys_interface().setpgid)(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn get_terminal_attrs(fd: c_int) -> SysResult<libc::termios> {
    let mut termios = unsafe { std::mem::zeroed::<libc::termios>() };
    let result = (sys_interface().tcgetattr)(fd, &mut termios);
    if result == 0 {
        Ok(termios)
    } else {
        Err(last_error())
    }
}

pub fn set_terminal_attrs(fd: c_int, termios: &libc::termios) -> SysResult<()> {
    let result = (sys_interface().tcsetattr)(fd, TCSADRAIN, termios);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn create_pipe() -> SysResult<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = (sys_interface().pipe)(&mut fds);
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd(oldfd: c_int, newfd: c_int) -> SysResult<()> {
    let result = (sys_interface().dup2)(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn duplicate_fd_to_new(fd: c_int) -> SysResult<c_int> {
    let result = (sys_interface().fcntl)(fd, F_DUPFD_CLOEXEC, 10);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn close_fd(fd: c_int) -> SysResult<()> {
    let result = (sys_interface().close)(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fd_status_flags(fd: c_int) -> SysResult<c_int> {
    let result = (sys_interface().fcntl)(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> SysResult<()> {
    let result = (sys_interface().fcntl)(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

fn fifo_like_fd(fd: c_int) -> bool {
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (sys_interface().fstat)(fd, buf.as_mut_ptr());
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
    let result = (sys_interface().read)(fd, buf);
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
    let result = (sys_interface().stat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(last_error())
    }
}

pub fn open_file(path: &str, flags: c_int, mode: mode_t) -> SysResult<c_int> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().open)(c_path.as_ptr(), flags, mode);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn write_fd(fd: c_int, data: &[u8]) -> SysResult<usize> {
    let result = (sys_interface().write)(fd, data);
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

pub fn access_path(path: &str, mode: c_int) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().access)(c_path.as_ptr(), mode);
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
    stat_path(path)
        .map(|s| s.is_regular_file())
        .unwrap_or(false)
}

pub fn change_dir(path: &str) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().chdir)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn get_cwd() -> SysResult<String> {
    let mut buf = vec![0u8; 4096];
    let result = (sys_interface().getcwd)(buf.as_mut_ptr().cast(), buf.len());
    if result.is_null() {
        Err(last_error())
    } else {
        let cstr = unsafe { CStr::from_ptr(result) };
        Ok(cstr.to_string_lossy().into_owned())
    }
}

pub fn read_dir_entries(path: &str) -> SysResult<Vec<String>> {
    let c_path = to_cstring(path)?;
    let dirp = (sys_interface().opendir)(c_path.as_ptr());
    if dirp.is_null() {
        return Err(last_error());
    }

    let mut entries = Vec::new();
    loop {
        set_errno(0);
        let ent = (sys_interface().readdir)(dirp);
        if ent.is_null() {
            let errno = last_error();
            (sys_interface().closedir)(dirp);
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
    let result = (sys_interface().realpath)(c_path.as_ptr(), std::ptr::null_mut());
    if result.is_null() {
        Err(last_error())
    } else {
        let s = unsafe { CStr::from_ptr(result) }
            .to_string_lossy()
            .into_owned();
        unsafe { libc::free(result.cast()) };
        Ok(s)
    }
}

pub fn read_file_bytes(path: &str) -> SysResult<Vec<u8>> {
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
    Ok(contents)
}

pub fn read_file(path: &str) -> SysResult<String> {
    let contents = read_file_bytes(path)?;
    Ok(String::from_utf8_lossy(&contents).into_owned())
}

pub fn open_for_redirect(
    path: &str,
    flags: c_int,
    mode: mode_t,
    noclobber: bool,
) -> SysResult<c_int> {
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
            status: ChildExitStatus {
                code: decode_wait_status(ws.status),
            },
            stdout: output,
        })
    }

    pub fn wait(self) -> SysResult<ChildExitStatus> {
        if let Some(fd) = self.stdout_fd {
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildExitStatus {
            code: decode_wait_status(ws.status),
        })
    }
}

pub fn fork_process() -> SysResult<Pid> {
    let pid = (sys_interface().fork)();
    if pid < 0 { Err(last_error()) } else { Ok(pid) }
}

pub fn exit_process(status: c_int) -> ! {
    (sys_interface().exit_process)(status);
    unreachable!()
}

#[cfg(coverage)]
fn flush_coverage() {
    unsafe {
        unsafe extern "C" {
            fn __llvm_profile_write_file() -> c_int;
        }
        __llvm_profile_write_file();
    }
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
        for &(key, value) in env_vars.unwrap_or(&[]) {
            let _ = env_set_var(key, value);
        }
        let argv_owned: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        let _ = exec_replace(program, &argv_owned);
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

    Ok(ChildHandle {
        pid,
        stdout_fd: stdout_read,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessTimes {
    pub user_ticks: u64,
    pub system_ticks: u64,
    pub child_user_ticks: u64,
    pub child_system_ticks: u64,
}

pub fn current_umask() -> FileModeMask {
    let mask = (sys_interface().umask)(0);
    (sys_interface().umask)(mask);
    mask & 0o777
}

pub fn set_umask(mask: FileModeMask) -> FileModeMask {
    (sys_interface().umask)(mask & 0o777) & 0o777
}

pub fn process_times() -> SysResult<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = (sys_interface().times)(raw.as_mut_ptr());
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

pub fn monotonic_clock_ns() -> u64 {
    (sys_interface().monotonic_clock_ns)()
}

pub fn clock_ticks_per_second() -> SysResult<u64> {
    let result = (sys_interface().sysconf)(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(last_error())
    }
}

/// Execute a program, replacing the current process image.
/// `file` is the pathname to exec (passed to `execvp`).
/// `argv` is the full argument vector: `argv[0]` is the command name
/// as typed by the user, `argv[1..]` are the remaining arguments.
pub fn string_to_bytes(s: &str) -> Vec<u8> {
    if s.is_ascii() {
        return s.as_bytes().to_vec();
    }
    s.chars()
        .flat_map(|ch| {
            if (ch as u32) < 256 {
                vec![ch as u8]
            } else {
                let mut buf = [0u8; 4];
                ch.encode_utf8(&mut buf);
                buf[..ch.len_utf8()].to_vec()
            }
        })
        .collect()
}

pub fn exec_replace<S: AsRef<str>>(file: &str, argv: &[S]) -> SysResult<()> {
    let c_file = CString::new(string_to_bytes(file)).map_err(|_| SysError::NulInPath)?;
    let mut owned = Vec::with_capacity(argv.len());
    for arg in argv {
        owned.push(CString::new(string_to_bytes(arg.as_ref())).map_err(|_| SysError::NulInPath)?);
    }

    let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());

    #[cfg(coverage)]
    flush_coverage();
    let result = (sys_interface().execvp)(c_file.as_ptr(), pointers.as_ptr());
    if result == -1 {
        Err(last_error())
    } else {
        Ok(())
    }
}

/// Replace the current process, using an explicit environment (as with `execve`).
pub fn exec_replace_with_env(
    file: &str,
    argv: &[String],
    env: &[(String, String)],
) -> SysResult<()> {
    let c_file = CString::new(string_to_bytes(file)).map_err(|_| SysError::NulInPath)?;
    let mut argv_owned = Vec::with_capacity(argv.len());
    for arg in argv {
        argv_owned.push(CString::new(string_to_bytes(arg)).map_err(|_| SysError::NulInPath)?);
    }
    let mut argp: Vec<*const c_char> = argv_owned.iter().map(|a| a.as_ptr()).collect();
    argp.push(std::ptr::null());

    let mut env_owned = Vec::with_capacity(env.len());
    for (k, v) in env {
        let pair = format!("{k}={v}");
        env_owned.push(CString::new(string_to_bytes(&pair)).map_err(|_| SysError::NulInPath)?);
    }
    let mut envp: Vec<*const c_char> = env_owned.iter().map(|e| e.as_ptr()).collect();
    envp.push(std::ptr::null());

    #[cfg(coverage)]
    flush_coverage();
    let result = (sys_interface().execve)(c_file.as_ptr(), argp.as_ptr(), envp.as_ptr());
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

pub fn signal_name(sig: c_int) -> &'static str {
    match sig {
        SIGHUP => "SIGHUP",
        SIGINT => "SIGINT",
        SIGQUIT => "SIGQUIT",
        SIGILL => "SIGILL",
        SIGABRT => "SIGABRT",
        SIGFPE => "SIGFPE",
        SIGKILL => "SIGKILL",
        SIGBUS => "SIGBUS",
        SIGUSR1 => "SIGUSR1",
        SIGSEGV => "SIGSEGV",
        SIGUSR2 => "SIGUSR2",
        SIGPIPE => "SIGPIPE",
        SIGALRM => "SIGALRM",
        SIGTERM => "SIGTERM",
        SIGCHLD => "SIGCHLD",
        SIGSTOP => "SIGSTOP",
        SIGCONT => "SIGCONT",
        SIGTRAP => "SIGTRAP",
        SIGTSTP => "SIGTSTP",
        SIGTTIN => "SIGTTIN",
        SIGTTOU => "SIGTTOU",
        SIGSYS => "SIGSYS",
        _ => "UNKNOWN",
    }
}

pub fn all_signal_names() -> &'static [(&'static str, c_int)] {
    &[
        ("HUP", SIGHUP),
        ("INT", SIGINT),
        ("QUIT", SIGQUIT),
        ("ILL", SIGILL),
        ("ABRT", SIGABRT),
        ("FPE", SIGFPE),
        ("KILL", SIGKILL),
        ("BUS", SIGBUS),
        ("USR1", SIGUSR1),
        ("SEGV", SIGSEGV),
        ("USR2", SIGUSR2),
        ("PIPE", SIGPIPE),
        ("ALRM", SIGALRM),
        ("TERM", SIGTERM),
        ("CHLD", SIGCHLD),
        ("STOP", SIGSTOP),
        ("CONT", SIGCONT),
        ("TRAP", SIGTRAP),
        ("TSTP", SIGTSTP),
        ("TTIN", SIGTTIN),
        ("TTOU", SIGTTOU),
        ("SYS", SIGSYS),
    ]
}

fn signal_mask(signal: c_int) -> Option<usize> {
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

fn wifexited(status: c_int) -> bool {
    (status & 0x7f) == 0
}

pub fn wexitstatus(status: c_int) -> i32 {
    (status >> 8) & 0xff
}

pub fn wifsignaled(status: c_int) -> bool {
    (status & 0x7f) != 0 && (status & 0x7f) != 0x7f
}

pub fn wtermsig(status: c_int) -> i32 {
    status & 0x7f
}

pub fn wifstopped(status: c_int) -> bool {
    (status & 0xff) == 0x7f
}

pub fn wifcontinued(status: c_int) -> bool {
    status == 0xffff
}

pub fn wstopsig(status: c_int) -> i32 {
    (status >> 8) & 0xff
}

pub fn shell_name_from_args(args: &[String]) -> &str {
    args.first().map(String::as_str).unwrap_or("meiksh")
}

pub fn cstr_lossy(bytes: &[u8]) -> String {
    CStr::from_bytes_until_nul(bytes)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

pub fn env_set_var(key: &str, value: &str) -> SysResult<()> {
    (sys_interface().setenv)(key, value)
}

pub fn env_unset_var(key: &str) -> SysResult<()> {
    (sys_interface().unsetenv)(key)
}

pub fn env_var(key: &str) -> Option<String> {
    (sys_interface().getenv)(key)
}

pub fn env_vars() -> HashMap<String, String> {
    (sys_interface().get_environ)()
}

pub fn home_dir_for_user(name: &str) -> Option<String> {
    (sys_interface().getpwnam)(name)
}

#[allow(clippy::disallowed_methods)]
pub fn env_args_os() -> Vec<std::ffi::OsString> {
    std::env::args_os().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_roundtrip() {
        fn fake_pipe(fds: &mut [c_int; 2]) -> c_int {
            fds[0] = 10;
            fds[1] = 11;
            0
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            close: fake_close,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
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
        assert_eq!(
            format_signal_exit(9),
            Some("terminated by signal 9".to_string())
        );
        assert_eq!(format_signal_exit(0), None);
    }

    #[test]
    fn shell_name_from_args_returns_first_arg_or_default() {
        assert_eq!(
            shell_name_from_args(&["meiksh".to_string(), "-c".to_string()]),
            "meiksh"
        );
        assert_eq!(shell_name_from_args(&[]), "meiksh");
    }

    #[test]
    fn cstr_lossy_handles_nul_terminated_and_plain_bytes() {
        assert_eq!(cstr_lossy(b"abc\0rest"), "abc".to_string());
        assert_eq!(cstr_lossy(b"plain-bytes"), "plain-bytes".to_string());
    }

    #[test]
    fn execvp_failure_returns_minus_one() {
        fn fail_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }
        let fake = SystemInterface {
            execvp: fail_execvp,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let program = CString::new("meiksh-command-that-does-not-exist").expect("cstring");
            let argv = [program.as_ptr(), std::ptr::null()];
            assert_eq!(
                (sys_interface().execvp)(program.as_ptr(), argv.as_ptr()),
                -1
            );
        });
    }

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        fn fail_isatty(_fd: c_int) -> c_int {
            0
        }
        fn fail_dup2(_old: c_int, _new: c_int) -> c_int {
            -1
        }
        fn fail_close(_fd: c_int) -> c_int {
            -1
        }
        fn fail_tcgetpgrp(_fd: c_int) -> Pid {
            -1
        }
        fn fail_tcsetpgrp(_fd: c_int, _pgid: Pid) -> c_int {
            -1
        }
        fn fail_setpgid(_pid: Pid, _pgid: Pid) -> c_int {
            -1
        }

        let fake = SystemInterface {
            isatty: fail_isatty,
            dup2: fail_dup2,
            close: fail_close,
            tcgetpgrp: fail_tcgetpgrp,
            tcsetpgrp: fail_tcsetpgrp,
            setpgid: fail_setpgid,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
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
        fn fail_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid {
            -1
        }
        let fake = SystemInterface {
            waitpid: fail_waitpid,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(wait_pid(999_999, false).is_err());
        });
    }

    #[test]
    fn exec_replace_rejects_nul_in_program_and_args() {
        let err = exec_replace::<String>("bad\0program", &[]).unwrap_err();
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
        let eacces = SysError::Errno(libc::EACCES);
        assert!(eacces.is_eacces());
        assert!(!errno_err.is_eacces());
        let enoexec = SysError::Errno(libc::ENOEXEC);
        assert!(enoexec.is_enoexec());
        let eintr = SysError::Errno(EINTR);
        assert!(eintr.is_eintr());
    }

    #[test]
    fn sys_success_branches_cover_fd_helpers() {
        fn fake_pipe(fds: &mut [c_int; 2]) -> c_int {
            fds[0] = 20;
            fds[1] = 21;
            0
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int {
            oldfd
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            dup2: fake_dup2,
            close: fake_close,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let (read_fd, write_fd) = create_pipe().expect("pipe");
            duplicate_fd(read_fd, read_fd).expect("dup self");
            close_fd(read_fd).expect("close read");
            close_fd(write_fd).expect("close write");
        });
    }

    #[test]
    fn process_identity_helper_covers_mismatch_branch() {
        assert!(!test_support::with_process_ids_for_test(
            (1, 2, 3, 3),
            has_same_real_and_effective_ids
        ));
        assert!(!test_support::with_process_ids_for_test(
            (1, 1, 3, 4),
            has_same_real_and_effective_ids
        ));
    }

    #[test]
    fn success_process_identity() {
        fn fake_getpid() -> Pid {
            4242
        }

        let fake = SystemInterface {
            getpid: fake_getpid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert_eq!(
                wait_pid(1, false).expect("wait").expect("status"),
                WaitStatus {
                    pid: 99,
                    status: 9 << 8
                }
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

        let fake = SystemInterface {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(is_interactive_fd(0));
            assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
            assert!(set_foreground_pgrp(0, 77).is_ok());
            assert!(set_process_group(1, 1).is_ok());
        });
    }

    #[test]
    fn success_pipe_and_fd() {
        fn fake_pipe(fds: &mut [c_int; 2]) -> c_int {
            fds[0] = 10;
            fds[1] = 11;
            0
        }
        fn fake_fcntl(fd: c_int, cmd: c_int, _arg: c_int) -> c_int {
            if cmd == F_DUPFD_CLOEXEC { fd + 100 } else { -1 }
        }
        fn fake_dup2(oldfd: c_int, _newfd: c_int) -> c_int {
            oldfd
        }
        fn fake_close(_fd: c_int) -> c_int {
            0
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            fcntl: fake_fcntl,
            dup2: fake_dup2,
            close: fake_close,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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
        fn fake_read(_fd: c_int, buf: &mut [u8]) -> isize {
            if buf.is_empty() {
                return 0;
            }
            buf[0] = b'X';
            1
        }

        let fake = SystemInterface {
            fcntl: fake_fcntl,
            read: fake_read,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            execvp: fake_execvp,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(exec_replace("echo", &["hello".to_string(), "world".to_string()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        assert_eq!(decode_wait_status(0x7f), 0x7f);
    }

    #[test]
    fn signal_handler_installation_succeeds() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGQUIT as i64), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
            ],
            || {
                install_shell_signal_handler(SIGINT).expect("install");
                ignore_signal(SIGTERM).expect("ignore");
                default_signal_action(SIGQUIT).expect("default");
            },
        );
    }

    #[test]
    fn signal_handler_error_paths() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGINT as i64), ArgMatcher::Any],
                    TraceResult::Err(libc::EINVAL),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGTERM as i64), ArgMatcher::Any],
                    TraceResult::Err(libc::EINVAL),
                ),
                t(
                    "signal",
                    vec![ArgMatcher::Int(SIGQUIT as i64), ArgMatcher::Any],
                    TraceResult::Err(libc::EINVAL),
                ),
            ],
            || {
                assert!(install_shell_signal_handler(SIGINT).is_err());
                assert!(ignore_signal(SIGTERM).is_err());
                assert!(default_signal_action(SIGQUIT).is_err());
            },
        );
    }

    #[test]
    fn pending_signal_tracking() {
        test_support::assert_no_syscalls(|| {
            test_support::with_pending_signals_for_test(&[SIGINT], || {
                assert_eq!(has_pending_signal(), Some(SIGINT));
                assert_eq!(take_pending_signals(), vec![SIGINT]);
            });
            test_support::with_pending_signals_for_test(&[99], || {
                assert_eq!(has_pending_signal(), None);
            });
        });
    }

    #[test]
    fn signal_utility_helpers() {
        let interrupted_error = SysError::Errno(EINTR);
        assert!(interrupted(&interrupted_error));
        let trap_sigs = supported_trap_signals();
        assert!(trap_sigs.contains(&SIGHUP));
        assert!(trap_sigs.contains(&SIGINT));
        assert!(trap_sigs.contains(&SIGQUIT));
        assert!(trap_sigs.contains(&SIGABRT));
        assert!(trap_sigs.contains(&SIGALRM));
        assert!(trap_sigs.contains(&SIGTERM));
        assert!(trap_sigs.contains(&SIGUSR1));
        assert!(trap_sigs.contains(&SIGUSR2));
        assert!(trap_sigs.contains(&SIGPIPE));
        assert!(trap_sigs.contains(&SIGCHLD));
        assert!(trap_sigs.contains(&SIGCONT));
        assert!(trap_sigs.contains(&SIGTRAP));
        assert_eq!(trap_sigs.len(), 20);
    }

    #[test]
    fn error_process_identity() {
        fn fake_getpid() -> Pid {
            1
        }

        let fake = SystemInterface {
            getpid: fake_getpid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            waitpid: fake_waitpid,
            kill: fake_kill,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(!is_interactive_fd(0));
            assert!(current_foreground_pgrp(0).is_err());
            assert!(set_foreground_pgrp(0, 1).is_err());
            assert!(set_process_group(1, 1).is_err());
        });
    }

    #[test]
    fn error_pipe_and_fd() {
        fn fake_pipe(_fds: &mut [c_int; 2]) -> c_int {
            -1
        }
        fn fake_fcntl(_fd: c_int, _cmd: c_int, _arg: c_int) -> c_int {
            -1
        }
        fn fake_dup2(_oldfd: c_int, _newfd: c_int) -> c_int {
            -1
        }
        fn fake_close(_fd: c_int) -> c_int {
            -1
        }

        let fake = SystemInterface {
            pipe: fake_pipe,
            fcntl: fake_fcntl,
            dup2: fake_dup2,
            close: fake_close,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(create_pipe().is_err());
            assert!(duplicate_fd_to_new(1).is_err());
            assert!(duplicate_fd(1, 2).is_err());
            assert!(close_fd(1).is_err());
        });
    }

    #[test]
    fn error_file_io() {
        fn fake_read(_fd: c_int, _buf: &mut [u8]) -> isize {
            -1
        }

        let fake = SystemInterface {
            read: fake_read,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
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

        let fake = SystemInterface {
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(process_times().is_err());
            assert!(clock_ticks_per_second().is_err());
        });
    }

    #[test]
    fn error_exec() {
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }

        let fake = SystemInterface {
            execvp: fake_execvp,
            ..default_interface()
        };

        test_support::with_test_interface(fake, || {
            assert!(exec_replace("echo", &["hi".to_string()]).is_err());
        });
    }

    #[test]
    fn decode_wait_status_signal_terminated() {
        assert_eq!(decode_wait_status(9), 137);
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_tty() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        // TTY path: isatty→1, fcntl F_GETFL→O_NONBLOCK|2, fcntl F_SETFL→0
        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int((O_NONBLOCK | 0o2) as i64),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_SETFL as i64),
                        ArgMatcher::Int(0o2),
                    ],
                    TraceResult::Int(0),
                ),
            ],
            || {
                ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_fifo() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        // FIFO path: isatty→0, fstat→S_IFIFO, fcntl F_GETFL→O_NONBLOCK|2, fcntl F_SETFL→0
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(42)], TraceResult::Int(0)),
                t(
                    "fstat",
                    vec![ArgMatcher::Fd(42), ArgMatcher::Any],
                    TraceResult::StatFifo,
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int((O_NONBLOCK | 0o2) as i64),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(42),
                        ArgMatcher::Int(F_SETFL as i64),
                        ArgMatcher::Int(0o2),
                    ],
                    TraceResult::Int(0),
                ),
            ],
            || {
                ensure_blocking_read_fd(42).expect("fifo blocking");
            },
        );
    }

    #[test]
    fn ensure_blocking_read_fd_surfaces_fcntl_errors() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Err(libc::EIO),
                ),
            ],
            || {
                assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
            },
        );
    }

    #[test]
    fn setenv_success() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "setenv",
                vec![
                    ArgMatcher::Str("MY_KEY".into()),
                    ArgMatcher::Str("my_val".into()),
                ],
                TraceResult::Int(0),
            )],
            || {
                let result = (sys_interface().setenv)("MY_KEY", "my_val");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    fn setenv_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "setenv",
                vec![ArgMatcher::Str("K".into()), ArgMatcher::Str("V".into())],
                TraceResult::Err(libc::ENOMEM),
            )],
            || {
                let result = (sys_interface().setenv)("K", "V");
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn unsetenv_success() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "unsetenv",
                vec![ArgMatcher::Str("MY_KEY".into())],
                TraceResult::Int(0),
            )],
            || {
                let result = (sys_interface().unsetenv)("MY_KEY");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    fn unsetenv_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "unsetenv",
                vec![ArgMatcher::Str("K".into())],
                TraceResult::Err(libc::EINVAL),
            )],
            || {
                let result = (sys_interface().unsetenv)("K");
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn getenv_found() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "getenv",
                vec![ArgMatcher::Str("HOME".into())],
                TraceResult::Str("/home/user".into()),
            )],
            || {
                let val = (sys_interface().getenv)("HOME");
                assert_eq!(val, Some("/home/user".to_string()));
            },
        );
    }

    #[test]
    fn getenv_not_found() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};

        run_trace(
            vec![t(
                "getenv",
                vec![ArgMatcher::Str("MISSING".into())],
                TraceResult::NullStr,
            )],
            || {
                let val = (sys_interface().getenv)("MISSING");
                assert_eq!(val, None);
            },
        );
    }

    #[test]
    fn get_environ_returns_map() {
        use test_support::{TraceResult, run_trace, t};

        let mut expected = HashMap::new();
        expected.insert("HOME".to_string(), "/home/user".to_string());
        expected.insert("PATH".to_string(), "/usr/bin".to_string());

        run_trace(
            vec![t(
                "get_environ",
                vec![],
                TraceResult::EnvMap(expected.clone()),
            )],
            || {
                let map = (sys_interface().get_environ)();
                assert_eq!(map.len(), 2);
                assert_eq!(map.get("HOME"), Some(&"/home/user".to_string()));
                assert_eq!(map.get("PATH"), Some(&"/usr/bin".to_string()));
            },
        );
    }

    #[test]
    fn default_env_functions_roundtrip() {
        let iface = default_interface();
        let key = "MEIKSH_TEST_ROUNDTRIP_878c2a";

        (iface.setenv)(key, "hello").expect("setenv");
        assert_eq!((iface.getenv)(key), Some("hello".to_string()));

        (iface.unsetenv)(key).expect("unsetenv");
        assert_eq!((iface.getenv)(key), None);
    }

    #[test]
    fn default_setenv_rejects_key_with_equals() {
        let iface = default_interface();
        assert!((iface.setenv)("BAD=KEY", "val").is_err());
    }

    #[test]
    fn default_unsetenv_rejects_key_with_equals() {
        let iface = default_interface();
        assert!((iface.unsetenv)("BAD=KEY").is_err());
    }

    #[test]
    fn default_interface_pending_signal_bits() {
        let iface = default_interface();
        let _bits = (iface.pending_signal_bits)();
    }

    #[test]
    fn default_interface_tcgetattr_tcsetattr() {
        let iface = default_interface();
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        let _ = (iface.tcgetattr)(STDIN_FILENO, &mut termios);
        let _ = (iface.tcsetattr)(STDIN_FILENO, 0, &termios);
    }

    #[test]
    fn default_interface_execvp_nonexistent() {
        let iface = default_interface();
        let prog = CString::new("/nonexistent_meiksh_test_binary_xyz").unwrap();
        let argv = [prog.as_ptr(), std::ptr::null()];
        let rc = (iface.execvp)(prog.as_ptr(), argv.as_ptr());
        assert!(rc < 0);
    }

    #[test]
    fn read_dir_entries_readdir_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t("opendir", vec![ArgMatcher::Any], TraceResult::Int(1)),
                t(
                    "readdir",
                    vec![ArgMatcher::Any],
                    TraceResult::Err(libc::EIO),
                ),
                t("closedir", vec![ArgMatcher::Any], TraceResult::Int(0)),
            ],
            || {
                assert!(read_dir_entries("/tmp").is_err());
            },
        );
    }

    #[test]
    fn setenv_rejects_nul() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "setenv",
                    vec![ArgMatcher::Str("K".into()), ArgMatcher::Str("V".into())],
                    TraceResult::Int(0),
                ),
                t(
                    "unsetenv",
                    vec![ArgMatcher::Str("K".into())],
                    TraceResult::Int(0),
                ),
            ],
            || {
                env_set_var("K", "V").expect("setenv ok");
                env_unset_var("K").expect("unsetenv ok");
            },
        );
    }

    #[test]
    fn signal_name_covers_all_branches() {
        assert_eq!(signal_name(SIGHUP), "SIGHUP");
        assert_eq!(signal_name(SIGINT), "SIGINT");
        assert_eq!(signal_name(SIGQUIT), "SIGQUIT");
        assert_eq!(signal_name(SIGILL), "SIGILL");
        assert_eq!(signal_name(SIGABRT), "SIGABRT");
        assert_eq!(signal_name(SIGFPE), "SIGFPE");
        assert_eq!(signal_name(SIGKILL), "SIGKILL");
        assert_eq!(signal_name(SIGBUS), "SIGBUS");
        assert_eq!(signal_name(SIGUSR1), "SIGUSR1");
        assert_eq!(signal_name(SIGSEGV), "SIGSEGV");
        assert_eq!(signal_name(SIGUSR2), "SIGUSR2");
        assert_eq!(signal_name(SIGPIPE), "SIGPIPE");
        assert_eq!(signal_name(SIGALRM), "SIGALRM");
        assert_eq!(signal_name(SIGTERM), "SIGTERM");
        assert_eq!(signal_name(SIGCHLD), "SIGCHLD");
        assert_eq!(signal_name(SIGSTOP), "SIGSTOP");
        assert_eq!(signal_name(SIGCONT), "SIGCONT");
        assert_eq!(signal_name(SIGTRAP), "SIGTRAP");
        assert_eq!(signal_name(SIGTSTP), "SIGTSTP");
        assert_eq!(signal_name(SIGTTIN), "SIGTTIN");
        assert_eq!(signal_name(SIGTTOU), "SIGTTOU");
        assert_eq!(signal_name(SIGSYS), "SIGSYS");
        assert_eq!(signal_name(999), "UNKNOWN");
    }

    #[test]
    fn query_signal_disposition_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![t(
                "signal",
                vec![ArgMatcher::Int(SIGINT as i64), ArgMatcher::Any],
                TraceResult::Err(libc::EINVAL),
            )],
            || {
                assert!(query_signal_disposition(SIGINT).is_err());
            },
        );
    }

    #[test]
    fn set_terminal_attrs_success_and_error() {
        let termios = unsafe { std::mem::zeroed::<libc::termios>() };

        fn fake_tcsetattr_ok(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
            0
        }
        let fake_ok = SystemInterface {
            tcsetattr: fake_tcsetattr_ok,
            ..default_interface()
        };
        test_support::with_test_interface(fake_ok, || {
            assert!(set_terminal_attrs(0, &termios).is_ok());
        });

        fn fake_tcsetattr_err(_: c_int, _: c_int, _: *const libc::termios) -> c_int {
            -1
        }
        let fake_err = SystemInterface {
            tcsetattr: fake_tcsetattr_err,
            ..default_interface()
        };
        test_support::with_test_interface(fake_err, || {
            assert!(set_terminal_attrs(0, &termios).is_err());
        });
    }

    #[test]
    fn get_terminal_attrs_error() {
        fn fake_tcgetattr_err(_: c_int, _: *mut libc::termios) -> c_int {
            -1
        }
        let fake = SystemInterface {
            tcgetattr: fake_tcgetattr_err,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(get_terminal_attrs(0).is_err());
        });
    }

    #[test]
    fn ensure_blocking_setfl_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "isatty",
                    vec![ArgMatcher::Fd(STDIN_FILENO)],
                    TraceResult::Int(1),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_GETFL as i64),
                        ArgMatcher::Int(0),
                    ],
                    TraceResult::Int((O_NONBLOCK | 0o2) as i64),
                ),
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(STDIN_FILENO),
                        ArgMatcher::Int(F_SETFL as i64),
                        ArgMatcher::Int(0o2),
                    ],
                    TraceResult::Err(libc::EIO),
                ),
            ],
            || {
                assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
            },
        );
    }

    #[test]
    fn fifo_like_fd_fstat_error() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t("isatty", vec![ArgMatcher::Fd(99)], TraceResult::Int(0)),
                t(
                    "fstat",
                    vec![ArgMatcher::Fd(99), ArgMatcher::Any],
                    TraceResult::Err(libc::EBADF),
                ),
            ],
            || {
                ensure_blocking_read_fd(99).expect("regular fd no-op");
            },
        );
    }

    #[test]
    fn write_all_fd_zero_write_eio() {
        fn fake_write(_fd: c_int, _data: &[u8]) -> isize {
            0
        }
        let fake = SystemInterface {
            write: fake_write,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let err = write_all_fd(1, b"data").unwrap_err();
            assert_eq!(err, SysError::Errno(libc::EIO));
        });
    }

    #[test]
    fn change_dir_error() {
        fn fake_chdir(_: *const c_char) -> c_int {
            -1
        }
        let fake = SystemInterface {
            chdir: fake_chdir,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(change_dir("/nonexistent").is_err());
        });
    }

    #[test]
    fn canonicalize_error() {
        fn fake_realpath(_: *const c_char, _: *mut c_char) -> *mut c_char {
            std::ptr::null_mut()
        }
        let fake = SystemInterface {
            realpath: fake_realpath,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            assert!(canonicalize("/nonexistent").is_err());
        });
    }

    #[test]
    fn open_for_redirect_noclobber_rewrites_flags() {
        use std::sync::atomic::{AtomicI32, Ordering};
        static CAPTURED_FLAGS: AtomicI32 = AtomicI32::new(0);

        fn fake_open(_: *const c_char, flags: c_int, _: mode_t) -> c_int {
            CAPTURED_FLAGS.store(flags, Ordering::SeqCst);
            5
        }
        let fake = SystemInterface {
            open: fake_open,
            ..default_interface()
        };
        test_support::with_test_interface(fake, || {
            let fd = open_for_redirect("/tmp/out", O_WRONLY | O_TRUNC | O_CREAT, 0o666, true)
                .expect("open");
            assert_eq!(fd, 5);
            let flags = CAPTURED_FLAGS.load(Ordering::SeqCst);
            assert!(flags & O_TRUNC == 0);
            assert!(flags & O_EXCL != 0);
            assert!(flags & O_CREAT != 0);

            let fd = open_for_redirect("/tmp/out", O_WRONLY | O_TRUNC | O_CREAT, 0o666, false)
                .expect("open");
            assert_eq!(fd, 5);
            let flags = CAPTURED_FLAGS.load(Ordering::SeqCst);
            assert!(flags & O_TRUNC != 0);
        });
    }

    #[test]
    fn child_exit_status_code() {
        let status = ChildExitStatus { code: 42 };
        assert_eq!(status.code(), Some(42));
        assert!(!status.success());
        let zero = ChildExitStatus { code: 0 };
        assert!(zero.success());
    }

    #[test]
    fn child_handle_wait_with_output_reads_pipe() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Bytes(b"hello".to_vec()),
                ),
                t(
                    "read",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Any],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let handle = ChildHandle {
                    pid: 99,
                    stdout_fd: Some(10),
                };
                let output = handle.wait_with_output().expect("wait_with_output");
                assert_eq!(output.stdout, b"hello");
                assert!(output.status.success());
            },
        );
    }

    #[test]
    fn child_handle_wait_closes_stdout_pipe() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t(
                    "waitpid",
                    vec![ArgMatcher::Int(99), ArgMatcher::Any, ArgMatcher::Any],
                    TraceResult::Status(0),
                ),
            ],
            || {
                let handle = ChildHandle {
                    pid: 99,
                    stdout_fd: Some(10),
                };
                let status = handle.wait().expect("wait");
                assert!(status.success());
            },
        );
    }

    #[test]
    fn spawn_child_with_pipe_stdout_and_all_params() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t, t_fork};
        run_trace(
            vec![
                t("pipe", vec![ArgMatcher::Any], TraceResult::Fds(10, 11)),
                t_fork(
                    TraceResult::Pid(100),
                    vec![
                        t(
                            "setpgid",
                            vec![ArgMatcher::Int(0), ArgMatcher::Int(42)],
                            TraceResult::Int(0),
                        ),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(5), ArgMatcher::Fd(STDIN_FILENO)],
                            TraceResult::Fd(STDIN_FILENO),
                        ),
                        t("close", vec![ArgMatcher::Fd(5)], TraceResult::Int(0)),
                        t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(11), ArgMatcher::Fd(STDOUT_FILENO)],
                            TraceResult::Fd(STDOUT_FILENO),
                        ),
                        t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
                        t(
                            "dup2",
                            vec![ArgMatcher::Fd(7), ArgMatcher::Fd(2)],
                            TraceResult::Fd(2),
                        ),
                        t("close", vec![ArgMatcher::Fd(7)], TraceResult::Int(0)),
                        t(
                            "setenv",
                            vec![ArgMatcher::Str("VAR".into()), ArgMatcher::Str("val".into())],
                            TraceResult::Int(0),
                        ),
                        t(
                            "execvp",
                            vec![ArgMatcher::Any, ArgMatcher::Any],
                            TraceResult::Int(-1),
                        ),
                    ],
                ),
                t("close", vec![ArgMatcher::Fd(5)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            ],
            || {
                let handle = spawn_child(
                    "echo",
                    &["echo", "hello"],
                    Some(&[("VAR", "val")]),
                    &[(7, 2)],
                    Some(5),
                    true,
                    Some(42),
                )
                .expect("spawn");
                assert_eq!(handle.pid, 100);
                assert_eq!(handle.stdout_fd, Some(10));
            },
        );
    }

    #[test]
    fn env_set_var_and_env_unset_var_and_env_var() {
        use test_support::{ArgMatcher, TraceResult, run_trace, t};
        run_trace(
            vec![
                t(
                    "setenv",
                    vec![ArgMatcher::Str("K".into()), ArgMatcher::Str("V".into())],
                    TraceResult::Int(0),
                ),
                t(
                    "getenv",
                    vec![ArgMatcher::Str("K".into())],
                    TraceResult::Str("V".into()),
                ),
                t(
                    "unsetenv",
                    vec![ArgMatcher::Str("K".into())],
                    TraceResult::Int(0),
                ),
                t(
                    "getenv",
                    vec![ArgMatcher::Str("K".into())],
                    TraceResult::NullStr,
                ),
            ],
            || {
                env_set_var("K", "V").expect("setenv");
                assert_eq!(env_var("K"), Some("V".into()));
                env_unset_var("K").expect("unsetenv");
                assert_eq!(env_var("K"), None);
            },
        );
    }

    #[test]
    fn default_interface_monotonic_clock_ns() {
        test_support::with_test_interface(default_interface(), || {
            let ns = monotonic_clock_ns();
            assert!(ns > 0, "monotonic clock should return positive nanoseconds");
        });
    }

    #[test]
    fn default_interface_classify_char_ascii() {
        test_support::with_test_interface(default_interface(), || {
            setup_locale();
            assert!(classify_char("alpha", 'a'));
            assert!(classify_char("alpha", 'Z'));
            assert!(!classify_char("alpha", '5'));
            assert!(classify_char("alnum", '9'));
            assert!(!classify_char("alnum", '!'));
            assert!(classify_char("blank", ' '));
            assert!(classify_char("blank", '\t'));
            assert!(!classify_char("blank", 'a'));
            assert!(classify_char("cntrl", '\x01'));
            assert!(!classify_char("cntrl", 'a'));
            assert!(classify_char("digit", '0'));
            assert!(!classify_char("digit", 'x'));
            assert!(classify_char("graph", '!'));
            assert!(!classify_char("graph", ' '));
            assert!(classify_char("lower", 'a'));
            assert!(!classify_char("lower", 'A'));
            assert!(classify_char("print", ' '));
            assert!(classify_char("print", 'a'));
            assert!(!classify_char("print", '\x01'));
            assert!(classify_char("punct", '.'));
            assert!(!classify_char("punct", 'a'));
            assert!(classify_char("space", '\n'));
            assert!(!classify_char("space", 'a'));
            assert!(classify_char("upper", 'A'));
            assert!(!classify_char("upper", 'a'));
            assert!(classify_char("xdigit", 'f'));
            assert!(!classify_char("xdigit", 'g'));
            assert!(!classify_char("bogus", 'a'));
        });
    }

    #[test]
    fn string_to_bytes_ascii_fast_path() {
        assert_eq!(string_to_bytes("hello"), b"hello");
    }

    #[test]
    fn string_to_bytes_latin1_codepoints() {
        let s: String = [0xe9u8 as char, 0xff as char].iter().collect();
        assert_eq!(string_to_bytes(&s), vec![0xe9, 0xff]);
    }

    #[test]
    fn string_to_bytes_non_latin1_codepoints() {
        let s = "\u{1F600}";
        let expected = s.as_bytes().to_vec();
        assert_eq!(string_to_bytes(s), expected);
    }

    #[test]
    fn no_interface_table_stubs_all_panic() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let tbl = test_support::no_interface_table();
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getpid)())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getppid)())).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.waitpid)(
                0,
                std::ptr::null_mut(),
                0
            )))
            .is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.kill)(0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.signal)(0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.isatty)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.tcgetpgrp)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.tcsetpgrp)(0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.setpgid)(0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.pipe)(&mut [0; 2]))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.dup2)(0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.close)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.fcntl)(0, 0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.read)(0, &mut [0u8; 1]))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.umask)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.times)(std::ptr::null_mut()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.sysconf)(0))).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.execvp)(
                std::ptr::null(),
                std::ptr::null()
            )))
            .is_err()
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.execve)(
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null()
            )))
            .is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.open)(std::ptr::null(), 0, 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.write)(0, &[]))).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.stat)(
                std::ptr::null(),
                std::ptr::null_mut()
            )))
            .is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.fstat)(0, std::ptr::null_mut()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.access)(std::ptr::null(), 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.chdir)(std::ptr::null()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getcwd)(std::ptr::null_mut(), 0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.opendir)(std::ptr::null()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.readdir)(std::ptr::null_mut()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.closedir)(std::ptr::null_mut()))).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.realpath)(
                std::ptr::null(),
                std::ptr::null_mut()
            )))
            .is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.fork)())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.exit_process)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.setenv)("k", "v"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.unsetenv)("k"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getenv)("k"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.get_environ)())).is_err());
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.tcgetattr)(
                0,
                std::ptr::null_mut()
            )))
            .is_err()
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.tcsetattr)(0, 0, std::ptr::null()))).is_err()
        );
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getpwnam)("nobody"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.monotonic_clock_ns)())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.setup_locale)())).is_err());
    }

    #[test]
    fn trace_getpid_and_getppid_dispatch() {
        test_support::run_trace(
            vec![
                test_support::t("getpid", vec![], test_support::TraceResult::Pid(42)),
                test_support::t("getppid", vec![], test_support::TraceResult::Pid(43)),
            ],
            || {
                assert_eq!(current_pid(), 42);
                assert_eq!(parent_pid(), 43);
            },
        );
    }

    #[test]
    fn trace_stat_fifo_and_fstat_dir_arms() {
        test_support::run_trace(
            vec![
                test_support::t(
                    "stat",
                    vec![test_support::ArgMatcher::Any, test_support::ArgMatcher::Any],
                    test_support::TraceResult::StatFifo,
                ),
                test_support::t(
                    "stat",
                    vec![test_support::ArgMatcher::Any, test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(0),
                ),
            ],
            || {
                let s = stat_path("/fifo").expect("stat fifo");
                assert!((s.mode & libc::S_IFMT) == libc::S_IFIFO);
                let _ = stat_path("/plain");
            },
        );
    }

    #[test]
    fn trace_setup_locale_is_noop() {
        test_support::run_trace(vec![], || {
            setup_locale();
        });
    }

    #[test]
    fn trace_getcwd_erange_and_pipe_err() {
        test_support::run_trace(
            vec![
                test_support::t(
                    "getcwd",
                    vec![],
                    test_support::TraceResult::Err(libc::ERANGE),
                ),
                test_support::t("pipe", vec![], test_support::TraceResult::Err(libc::EMFILE)),
            ],
            || {
                assert!(get_cwd().is_err());
                assert!(create_pipe().is_err());
            },
        );
    }

    #[test]
    fn trace_opendir_int_and_readdir_fallback() {
        test_support::run_trace(
            vec![test_support::t(
                "opendir",
                vec![test_support::ArgMatcher::Any],
                test_support::TraceResult::Int(0),
            )],
            || {
                assert!(read_dir_entries("/tmp").is_err());
            },
        );
    }

    #[test]
    fn trace_realpath_resolved_and_err() {
        test_support::run_trace(
            vec![
                test_support::t(
                    "realpath",
                    vec![test_support::ArgMatcher::Any, test_support::ArgMatcher::Any],
                    test_support::TraceResult::RealpathStr("/resolved".into()),
                ),
                test_support::t(
                    "realpath",
                    vec![test_support::ArgMatcher::Any, test_support::ArgMatcher::Any],
                    test_support::TraceResult::Err(ENOENT),
                ),
            ],
            || {
                assert_eq!(canonicalize("/foo").expect("resolve"), "/resolved");
                assert!(canonicalize("/bad").is_err());
            },
        );
    }

    #[test]
    fn trace_sysconf_dispatch() {
        test_support::run_trace(
            vec![test_support::t(
                "sysconf",
                vec![test_support::ArgMatcher::Any],
                test_support::TraceResult::Int(100),
            )],
            || {
                assert_eq!(clock_ticks_per_second().expect("sysconf"), 100);
            },
        );
    }

    #[test]
    fn trace_umask_times_sysconf_and_monotonic_dispatch() {
        test_support::run_trace(
            vec![
                test_support::t(
                    "umask",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(0o22),
                ),
                test_support::t(
                    "times",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(500),
                ),
                test_support::t(
                    "sysconf",
                    vec![test_support::ArgMatcher::Any],
                    test_support::TraceResult::Int(100),
                ),
                test_support::t(
                    "monotonic_clock_ns",
                    vec![],
                    test_support::TraceResult::Int(123456),
                ),
            ],
            || {
                assert_eq!(set_umask(0o77), 0o22);
                let times = process_times().expect("times");
                assert_eq!(times.user_ticks, 0);
                assert_eq!(clock_ticks_per_second().expect("sysconf"), 100);
                assert_eq!(monotonic_clock_ns(), 123456);
            },
        );
    }

    #[test]
    fn trace_getpwnam_null_str() {
        test_support::run_trace(
            vec![test_support::t(
                "getpwnam",
                vec![test_support::ArgMatcher::Str("nobody".into())],
                test_support::TraceResult::NullStr,
            )],
            || {
                assert!(home_dir_for_user("nobody").is_none());
            },
        );
    }

    #[test]
    fn trace_waitpid_fallthrough() {
        test_support::run_trace(
            vec![test_support::t(
                "waitpid",
                vec![
                    test_support::ArgMatcher::Int(-1),
                    test_support::ArgMatcher::Any,
                    test_support::ArgMatcher::Any,
                ],
                test_support::TraceResult::Int(0),
            )],
            || {
                let r = wait_pid(-1, true);
                assert!(r.is_ok());
            },
        );
    }

    #[test]
    fn trace_signal_default_fallthrough() {
        test_support::run_trace(
            vec![test_support::t(
                "signal",
                vec![
                    test_support::ArgMatcher::Int(SIGINT as i64),
                    test_support::ArgMatcher::Any,
                ],
                test_support::TraceResult::Int(0),
            )],
            || {
                let _ = default_signal_action(SIGINT);
            },
        );
    }

    #[test]
    fn trace_times_err_path() {
        test_support::run_trace(
            vec![test_support::t(
                "times",
                vec![test_support::ArgMatcher::Any],
                test_support::TraceResult::Err(libc::EINVAL),
            )],
            || {
                assert!(process_times().is_err());
            },
        );
    }

    #[test]
    fn into_arg_matcher_impls() {
        use test_support::{ArgMatcher, IntoArgMatcher, arg_from};
        let _a: ArgMatcher = 42i32.into_arg();
        let _b: ArgMatcher = 100i64.into_arg();
        let _c: ArgMatcher = "hello".into_arg();
        let _d: ArgMatcher = (b"data" as &[u8]).into_arg();
        let v = vec![1u8, 2, 3];
        let _e: ArgMatcher = (&v).into_arg();
        let _f = arg_from(42i32);
    }

    #[test]
    fn take_child_traces_returns_empty_by_default() {
        test_support::assert_no_syscalls(|| {
            let traces = test_support::take_child_traces();
            assert!(traces.is_empty());
        });
    }

    #[test]
    fn exec_replace_with_env_error_path() {
        test_support::run_trace(
            vec![test_support::t(
                "execve",
                vec![test_support::ArgMatcher::Any, test_support::ArgMatcher::Any],
                test_support::TraceResult::Err(ENOENT),
            )],
            || {
                let result = exec_replace_with_env("/nonexistent", &["test".into()], &[]);
                assert!(result.is_err());
            },
        );
    }

    #[test]
    fn exec_replace_with_env_real_execve_error() {
        test_support::with_test_interface(default_interface(), || {
            let result =
                exec_replace_with_env("/nonexistent_path_no_exist", &["no_exist".into()], &[]);
            assert!(result.is_err());
        });
    }
}
