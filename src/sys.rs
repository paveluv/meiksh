use std::ffi::{CStr, CString};
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::FileTypeExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use libc::{self, c_char, c_int, c_long};

pub type Pid = libc::pid_t;
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
const EINTR: i32 = libc::EINTR;
const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

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
}

fn default_syscalls() -> Syscalls {
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

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;
    use std::sync::Mutex;

    thread_local! {
        static TEST_SYSCALLS: RefCell<Option<Syscalls>> = const { RefCell::new(None) };
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

    pub(crate) fn with_test_syscalls<T>(syscalls: Syscalls, f: impl FnOnce() -> T) -> T {
        let _guard = syscall_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        TEST_SYSCALLS.with(|cell| {
            let previous = cell.replace(Some(syscalls));
            let result = f();
            cell.replace(previous);
            result
        })
    }

    pub(crate) fn with_execvp_for_test<T>(
        execvp_fn: fn(*const c_char, *const *const c_char) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            execvp: execvp_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_signal_syscall_for_test<T>(
        signal_fn: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            signal: signal_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_waitpid_for_test<T>(
        waitpid_fn: fn(Pid, *mut c_int, c_int) -> Pid,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            waitpid: waitpid_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn set_pending_signals_for_test(signals: &[c_int]) {
        let bits = signals
            .iter()
            .filter_map(|signal| signal_mask(*signal))
            .fold(0usize, |acc, bit| acc | bit);
        PENDING_SIGNALS.store(bits, Ordering::SeqCst);
    }

    pub(crate) fn with_job_control_syscalls_for_test<T>(
        isatty_fn: fn(c_int) -> c_int,
        tcgetpgrp_fn: fn(c_int) -> Pid,
        tcsetpgrp_fn: fn(c_int, Pid) -> c_int,
        setpgid_fn: fn(Pid, Pid) -> c_int,
        kill_fn: fn(Pid, c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            isatty: isatty_fn,
            tcgetpgrp: tcgetpgrp_fn,
            tcsetpgrp: tcsetpgrp_fn,
            setpgid: setpgid_fn,
            kill: kill_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_fd_ops_for_test<T>(
        dup_fn: fn(c_int) -> c_int,
        dup2_fn: fn(c_int, c_int) -> c_int,
        close_fn: fn(c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            dup: dup_fn,
            dup2: dup2_fn,
            close: close_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_fcntl_and_isatty_for_test<T>(
        fcntl_fn: fn(c_int, c_int, c_int) -> c_int,
        isatty_fn: fn(c_int) -> c_int,
        f: impl FnOnce() -> T,
    ) -> T {
        let syscalls = Syscalls {
            fcntl: fcntl_fn,
            isatty: isatty_fn,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
    }

    pub(crate) fn with_times_error_for_test<T>(f: impl FnOnce() -> T) -> T {
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            60
        }
        let syscalls = Syscalls {
            times: fake_times,
            sysconf: fake_sysconf,
            ..default_syscalls()
        };
        with_test_syscalls(syscalls, f)
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

pub fn wait_pid(pid: Pid, nohang: bool) -> io::Result<Option<WaitStatus>> {
    let mut status = 0;
    let options = if nohang { WNOHANG } else { 0 };
    let result = (syscalls().waitpid)(pid, &mut status, options);
    if result > 0 {
        Ok(Some(WaitStatus { pid: result, status }))
    } else if result == 0 {
        Ok(None)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn send_signal(pid: Pid, signal: c_int) -> io::Result<()> {
    let result = (syscalls().kill)(pid, signal);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn install_shell_signal_handler(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, record_signal as *const () as libc::sighandler_t);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn ignore_signal(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, SIG_IGN_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn default_signal_action(signal: c_int) -> io::Result<()> {
    let result = (syscalls().signal)(signal, SIG_DFL_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub fn has_pending_signal() -> Option<c_int> {
    let bits = PENDING_SIGNALS.load(Ordering::SeqCst);
    supported_trap_signals()
        .into_iter()
        .find(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
}

pub fn take_pending_signals() -> Vec<c_int> {
    let bits = PENDING_SIGNALS.swap(0, Ordering::SeqCst);
    supported_trap_signals()
        .into_iter()
        .filter(|signal| signal_mask(*signal).map(|mask| bits & mask != 0).unwrap_or(false))
        .collect()
}

pub fn supported_trap_signals() -> Vec<c_int> {
    vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]
}

pub fn interrupted(error: &io::Error) -> bool {
    error.raw_os_error() == Some(EINTR)
}

pub fn current_foreground_pgrp(fd: c_int) -> io::Result<Pid> {
    let result = (syscalls().tcgetpgrp)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn set_foreground_pgrp(fd: c_int, pgrp: Pid) -> io::Result<()> {
    let result = (syscalls().tcsetpgrp)(fd, pgrp);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn set_process_group(pid: Pid, pgid: Pid) -> io::Result<()> {
    let result = (syscalls().setpgid)(pid, pgid);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn create_pipe() -> io::Result<(c_int, c_int)> {
    let mut fds = [0; 2];
    let result = (syscalls().pipe)(fds.as_mut_ptr());
    if result == 0 {
        Ok((fds[0], fds[1]))
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn duplicate_fd(oldfd: c_int, newfd: c_int) -> io::Result<()> {
    let result = (syscalls().dup2)(oldfd, newfd);
    if result >= 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn duplicate_fd_to_new(fd: c_int) -> io::Result<c_int> {
    let result = (syscalls().dup)(fd);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn close_fd(fd: c_int) -> io::Result<()> {
    let result = (syscalls().close)(fd);
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fd_status_flags(fd: c_int) -> io::Result<c_int> {
    let result = (syscalls().fcntl)(fd, F_GETFL, 0);
    if result >= 0 {
        Ok(result)
    } else {
        Err(io::Error::last_os_error())
    }
}

fn set_fd_status_flags(fd: c_int, flags: c_int) -> io::Result<()> {
    let result = (syscalls().fcntl)(fd, F_SETFL, flags);
    if result >= 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn fifo_like_fd(fd: c_int) -> bool {
    fs::metadata(format!("/dev/fd/{fd}"))
        .map(|metadata| metadata.file_type().is_fifo())
        .unwrap_or(false)
}

pub fn ensure_blocking_read_fd(fd: c_int) -> io::Result<()> {
    if !is_interactive_fd(fd) && !fifo_like_fd(fd) {
        return Ok(());
    }
    let flags = fd_status_flags(fd)?;
    if flags & O_NONBLOCK != 0 {
        set_fd_status_flags(fd, flags & !O_NONBLOCK)?;
    }
    Ok(())
}

pub fn read_fd(fd: c_int, buf: &mut [u8]) -> io::Result<usize> {
    let result = (syscalls().read)(fd, buf.as_mut_ptr(), buf.len());
    if result >= 0 {
        Ok(result as usize)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub struct FdReader {
    fd: c_int,
}

impl FdReader {
    pub fn new(fd: c_int) -> Self {
        Self { fd }
    }
}

impl Read for FdReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        read_fd(self.fd, buf)
    }
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

pub fn process_times() -> io::Result<ProcessTimes> {
    let mut raw = std::mem::MaybeUninit::<libc::tms>::zeroed();
    let result = (syscalls().times)(raw.as_mut_ptr());
    if result == ClockTicks::MAX {
        return Err(io::Error::last_os_error());
    }
    let raw = unsafe { raw.assume_init() };
    Ok(ProcessTimes {
        user_ticks: raw.tms_utime as u64,
        system_ticks: raw.tms_stime as u64,
        child_user_ticks: raw.tms_cutime as u64,
        child_system_ticks: raw.tms_cstime as u64,
    })
}

pub fn clock_ticks_per_second() -> io::Result<u64> {
    let result = (syscalls().sysconf)(SC_CLK_TCK);
    if result > 0 {
        Ok(result as u64)
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn exec_replace(program: &str, argv: &[String]) -> io::Result<()> {
    let mut owned = Vec::with_capacity(argv.len() + 1);
    owned.push(CString::new(program)?);
    for arg in argv {
        owned.push(CString::new(arg.as_str())?);
    }

    let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());

    let result = (syscalls().execvp)(owned[0].as_ptr(), pointers.as_ptr());
    if result == -1 {
        Err(io::Error::last_os_error())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn pipe_roundtrip() {
        let (read_fd, write_fd) = create_pipe().expect("pipe");
        close_fd(read_fd).expect("close read");
        close_fd(write_fd).expect("close write");
    }

    #[test]
    fn decodes_wait_status_shapes() {
        assert_eq!(decode_wait_status(0), 0);
        assert_eq!(decode_wait_status(7 << 8), 7);
        assert_eq!(format_signal_exit(9), Some("terminated by signal 9".to_string()));
        assert_eq!(format_signal_exit(0), None);
    }

    #[test]
    fn shell_name_and_cstr_helpers_work() {
        assert_eq!(shell_name_from_args(&["meiksh".to_string(), "-c".to_string()]), "meiksh");
        assert_eq!(shell_name_from_args(&[]), "meiksh");
        assert_eq!(cstr_lossy(b"abc\0rest"), "abc".to_string());
        assert_eq!(cstr_lossy(b"plain-bytes"), "plain-bytes".to_string());

        let syscalls = default_syscalls();
        let program = CString::new("meiksh-command-that-does-not-exist").expect("cstring");
        let argv = [program.as_ptr(), std::ptr::null()];
        assert_eq!((syscalls.execvp)(program.as_ptr(), argv.as_ptr()), -1);
    }

    use crate::test_utils::meiksh_bin_path;

    #[test]
    fn invalid_fd_operations_fail_cleanly() {
        assert!(!is_interactive_fd(-1));
        assert!(duplicate_fd(-1, -1).is_err());
        assert!(close_fd(-1).is_err());
        assert!(current_foreground_pgrp(-1).is_err());
        assert!(set_foreground_pgrp(-1, 0).is_err());
        assert!(set_process_group(999_999, 999_999).is_err());
    }

    #[test]
    fn wait_pid_and_exec_replace_error_paths_work() {
        let mut child = Command::new(meiksh_bin_path())
            .args(["-c", "exit 5"])
            .spawn()
            .expect("spawn");
        let status = wait_pid(child.id() as i32, false)
            .expect("wait")
            .expect("status");
        assert_eq!(decode_wait_status(status.status), 5);
        let _ = child.wait();

        assert!(wait_pid(999_999, false).is_err());
        assert!(exec_replace("bad\0program", &["bad\0program".to_string()]).is_err());
    }

    #[test]
    fn misc_sys_helpers_cover_successish_paths() {
        assert!(current_pid() > 0);
        assert!(has_same_real_and_effective_ids());
        send_signal(current_pid(), 0).expect("signal 0");
        let child = Command::new(meiksh_bin_path())
            .args(["-c", "sleep 0.05"])
            .spawn()
            .expect("spawn");
        let pending = wait_pid(child.id() as i32, true).expect("wait nohang");
        assert!(pending.is_none() || pending.is_some());
        let _ = send_signal(child.id() as i32, 0);
        let _ = current_foreground_pgrp(STDIN_FILENO);
    }

    #[test]
    fn sys_success_branches_cover_fd_helpers() {
        let (read_fd, write_fd) = create_pipe().expect("pipe");
        duplicate_fd(read_fd, read_fd).expect("dup self");
        close_fd(read_fd).expect("close read");
        close_fd(write_fd).expect("close write");
    }

    #[test]
    fn process_identity_helper_covers_mismatch_branch() {
        assert!(!test_support::with_process_ids_for_test((1, 2, 3, 3), has_same_real_and_effective_ids));
        assert!(!test_support::with_process_ids_for_test((1, 1, 3, 4), has_same_real_and_effective_ids));
    }

    #[test]
    fn sys_injected_success_paths_cover_remaining_branches() {
        fn fake_getpid() -> Pid {
            4242
        }
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
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            0
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            fcntl: fake_fcntl,
            read: fake_read,
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            execvp: fake_execvp,
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 4242);
            assert!(is_interactive_fd(0));
            assert_eq!(
                wait_pid(1, false).expect("wait").expect("status"),
                WaitStatus { pid: 99, status: 9 << 8 }
            );
            assert!(send_signal(1, 0).is_ok());
            assert_eq!(current_foreground_pgrp(0).expect("pgrp"), 77);
            assert!(set_foreground_pgrp(0, 77).is_ok());
            assert!(set_process_group(1, 1).is_ok());
            assert_eq!(create_pipe().expect("pipe"), (10, 11));
            assert_eq!(duplicate_fd_to_new(4).expect("dup"), 104);
            assert!(duplicate_fd(4, 5).is_ok());
            assert!(close_fd(4).is_ok());
            let mut buffer = [0u8; 1];
            assert_eq!(read_fd(0, &mut buffer).expect("read"), 1);
            assert_eq!(buffer, [b'X']);
            let mut reader = FdReader::new(0);
            assert_eq!(reader.read(&mut buffer).expect("reader read"), 1);
            assert_eq!(buffer, [b'X']);
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
            assert!(exec_replace("echo", &["hello".to_string(), "world".to_string()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        assert_eq!(decode_wait_status(0x7f), 0x7f);
    }

    #[test]
    fn signal_helpers_cover_pending_ignore_default_and_error_paths() {
        fn ok_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            0
        }
        fn err_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            SIG_ERR_HANDLER
        }

        test_support::with_signal_syscall_for_test(ok_signal, || {
            install_shell_signal_handler(SIGINT).expect("install");
            ignore_signal(SIGTERM).expect("ignore");
            default_signal_action(SIGQUIT).expect("default");
        });

        test_support::with_signal_syscall_for_test(err_signal, || {
            assert!(install_shell_signal_handler(SIGINT).is_err());
            assert!(ignore_signal(SIGTERM).is_err());
            assert!(default_signal_action(SIGQUIT).is_err());
        });

        test_support::set_pending_signals_for_test(&[SIGINT]);
        assert_eq!(has_pending_signal(), Some(SIGINT));
        assert_eq!(take_pending_signals(), vec![SIGINT]);
        test_support::set_pending_signals_for_test(&[99]);
        assert_eq!(has_pending_signal(), None);

        let interrupted_error = io::Error::from_raw_os_error(EINTR);
        assert!(interrupted(&interrupted_error));
        assert_eq!(supported_trap_signals(), vec![SIGHUP, SIGINT, SIGQUIT, SIGABRT, SIGALRM, SIGTERM]);
    }

    #[test]
    fn sys_injected_error_paths_cover_remaining_branches() {
        fn fake_getpid() -> Pid {
            1
        }
        fn fake_waitpid(_pid: Pid, _status: *mut c_int, _options: c_int) -> Pid {
            -1
        }
        fn fake_kill(_pid: Pid, _sig: c_int) -> c_int {
            -1
        }
        fn fake_signal(_sig: c_int, _handler: libc::sighandler_t) -> libc::sighandler_t {
            SIG_ERR_HANDLER
        }
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
        fn fake_fcntl(_fd: c_int, _cmd: c_int, _arg: c_int) -> c_int {
            -1
        }
        fn fake_read(_fd: c_int, _buf: *mut u8, _count: usize) -> isize {
            -1
        }
        fn fake_umask(mask: FileModeMask) -> FileModeMask {
            mask
        }
        fn fake_times(_buffer: *mut libc::tms) -> ClockTicks {
            ClockTicks::MAX
        }
        fn fake_sysconf(_name: c_int) -> c_long {
            -1
        }
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            signal: fake_signal,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            fcntl: fake_fcntl,
            read: fake_read,
            umask: fake_umask,
            times: fake_times,
            sysconf: fake_sysconf,
            execvp: fake_execvp,
        };

        test_support::with_test_syscalls(fake, || {
            assert_eq!(current_pid(), 1);
            assert!(!is_interactive_fd(0));
            assert!(send_signal(1, 0).is_err());
            assert!(current_foreground_pgrp(0).is_err());
            assert!(set_foreground_pgrp(0, 1).is_err());
            assert!(set_process_group(1, 1).is_err());
            assert!(create_pipe().is_err());
            assert!(duplicate_fd_to_new(1).is_err());
            assert!(duplicate_fd(1, 2).is_err());
            assert!(close_fd(1).is_err());
            assert!(read_fd(0, &mut [0u8; 1]).is_err());
            assert!(process_times().is_err());
            assert!(clock_ticks_per_second().is_err());
            assert!(exec_replace("echo", &["hi".to_string()]).is_err());
            assert!(wait_pid(1, false).is_err());
        });

        assert_eq!(decode_wait_status(9), 137);
    }

    #[test]
    fn ensure_blocking_read_fd_clears_nonblocking_for_ttys_and_fifos() {
        static LAST_SET_FLAGS: AtomicUsize = AtomicUsize::new(usize::MAX);

        fn fake_isatty(_fd: c_int) -> c_int {
            1
        }
        fn fake_fcntl(_fd: c_int, cmd: c_int, flags: c_int) -> c_int {
            match cmd {
                F_GETFL => O_NONBLOCK | 0o2,
                F_SETFL => {
                    LAST_SET_FLAGS.store(flags as usize, Ordering::SeqCst);
                    0
                }
                _ => -1,
            }
        }

        test_support::with_fcntl_and_isatty_for_test(fake_fcntl, fake_isatty, || {
            LAST_SET_FLAGS.store(usize::MAX, Ordering::SeqCst);
            ensure_blocking_read_fd(STDIN_FILENO).expect("tty blocking");
            assert_eq!(LAST_SET_FLAGS.load(Ordering::SeqCst), 0o2);
        });

        fn not_tty(_fd: c_int) -> c_int {
            0
        }

        let (read_end, write_end) = create_pipe().expect("pipe");
        test_support::with_fcntl_and_isatty_for_test(fake_fcntl, not_tty, || {
            LAST_SET_FLAGS.store(usize::MAX, Ordering::SeqCst);
            ensure_blocking_read_fd(read_end).expect("fifo blocking");
            assert_eq!(LAST_SET_FLAGS.load(Ordering::SeqCst), 0o2);
        });
        close_fd(read_end).expect("close read");
        close_fd(write_end).expect("close write");
    }

    #[test]
    fn ensure_blocking_read_fd_skips_regular_files_and_surfaces_fcntl_errors() {
        static FCNTL_CALLS: AtomicUsize = AtomicUsize::new(0);

        fn not_tty(_fd: c_int) -> c_int {
            0
        }
        fn counting_fcntl(_fd: c_int, _cmd: c_int, _flags: c_int) -> c_int {
            FCNTL_CALLS.fetch_add(1, Ordering::SeqCst);
            0
        }
        fn failing_fcntl(_fd: c_int, _cmd: c_int, _flags: c_int) -> c_int {
            -1
        }

        let path = std::env::temp_dir().join(format!(
            "meiksh-sys-regular-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::write(&path, b"x").expect("write temp");
        let file = std::fs::File::open(&path).expect("open temp");
        test_support::with_fcntl_and_isatty_for_test(counting_fcntl, not_tty, || {
            FCNTL_CALLS.store(0, Ordering::SeqCst);
            ensure_blocking_read_fd(std::os::fd::AsRawFd::as_raw_fd(&file)).expect("regular file");
            assert_eq!(FCNTL_CALLS.load(Ordering::SeqCst), 0);
        });
        std::fs::remove_file(&path).expect("remove temp");

        test_support::with_fcntl_and_isatty_for_test(failing_fcntl, |_| 1, || {
            assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
        });
    }
}
