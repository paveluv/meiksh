use libc::{c_char, c_int};

use super::constants::{
    SIG_DFL_HANDLER, SIG_ERR_HANDLER, SIG_IGN_HANDLER, SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT,
    SIGFPE, SIGHUP, SIGILL, SIGINT, SIGKILL, SIGPIPE, SIGQUIT, SIGSEGV, SIGSTOP, SIGSYS, SIGTERM,
    SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGUSR1, SIGUSR2, STDIN_FILENO, STDOUT_FILENO, WCONTINUED,
    WNOHANG, WUNTRACED,
};
use super::env::env_set_var;
use super::error::{SysError, SysResult};
#[cfg(test)]
use super::fd_io::read_fd;
use super::fd_io::{close_fd, create_pipe, duplicate_fd};
use super::interface::{self, last_error, record_signal, signal_mask};
use super::tty::set_process_group;
#[cfg(test)]
use super::types::{ChildExitStatus, ChildOutput};
use super::types::{ChildHandle, Pid, WaitStatus};

pub(crate) fn current_pid() -> Pid {
    interface::getpid()
}

pub(crate) fn parent_pid() -> Pid {
    interface::getppid()
}
pub(crate) fn has_same_real_and_effective_ids() -> bool {
    #[cfg(test)]
    if let Some((uid, euid, gid, egid)) = super::test_support::current_process_ids() {
        return uid == euid && gid == egid;
    }
    unsafe { libc::getuid() == libc::geteuid() && libc::getgid() == libc::getegid() }
}

/// `true` iff `geteuid() == 0`. Used by `\$` prompt expansion to
/// decide between `#` (root) and `$` (non-root).
pub(crate) fn effective_uid_is_root() -> bool {
    interface::effective_uid_is_root()
}

/// Effective user id as a `u32`. Used in combination with
/// [`getpwuid_name`] to resolve a login name when `$USER` is unset.
pub(crate) fn effective_uid_raw() -> u32 {
    interface::effective_uid_raw()
}

/// Login name (`pw_name`) for `uid`. Returns `None` if the user is
/// not found in `/etc/passwd` (or the equivalent NSS source).
pub(crate) fn getpwuid_name(uid: u32) -> Option<Vec<u8>> {
    interface::getpwuid_name(uid)
}

/// Host name via `gethostname(2)`. Returns `None` on failure.
/// Unlike the raw syscall this drops any trailing NUL and returns
/// only the populated prefix of the buffer.
pub(crate) fn hostname_bytes() -> Option<Vec<u8>> {
    let mut buf = [0u8; 256];
    let rc = interface::gethostname_raw(&mut buf);
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    Some(buf[..end].to_vec())
}

pub(crate) fn wait_pid(pid: Pid, nohang: bool) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let options = if nohang { WNOHANG } else { 0 };
    let result = interface::waitpid(pid, &mut status, options);
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

pub(crate) fn wait_pid_untraced(pid: Pid, _nohang: bool) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let result = interface::waitpid(pid, &mut status, WUNTRACED);
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

pub(crate) fn wait_pid_job_status(pid: Pid) -> SysResult<Option<WaitStatus>> {
    let mut status = 0;
    let options = WUNTRACED | WCONTINUED | WNOHANG;
    let result = interface::waitpid(pid, &mut status, options);
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

pub(crate) fn send_signal(pid: Pid, signal: c_int) -> SysResult<()> {
    let result = interface::kill(pid, signal);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub(crate) fn install_shell_signal_handler(signal: c_int) -> SysResult<()> {
    let result = interface::signal(signal, record_signal as *const () as libc::sighandler_t);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub(crate) fn ignore_signal(signal: c_int) -> SysResult<()> {
    let result = interface::signal(signal, SIG_IGN_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub(crate) fn default_signal_action(signal: c_int) -> SysResult<()> {
    let result = interface::signal(signal, SIG_DFL_HANDLER);
    if result == SIG_ERR_HANDLER {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub(crate) const SUPPORTED_TRAP_SIGNALS: [c_int; 20] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGABRT, SIGFPE, SIGBUS, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE,
    SIGALRM, SIGTERM, SIGCHLD, SIGCONT, SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGSYS,
];

pub(crate) fn supported_trap_signals() -> &'static [c_int] {
    &SUPPORTED_TRAP_SIGNALS
}

pub(crate) fn pending_signal_bits() -> usize {
    super::interface::pending_signal_bits()
}

pub(crate) fn has_pending_signal() -> Option<c_int> {
    let bits = super::interface::pending_signal_bits();
    if bits == 0 {
        return None;
    }
    SUPPORTED_TRAP_SIGNALS.iter().copied().find(|signal| {
        signal_mask(*signal)
            .map(|mask| bits & mask != 0)
            .unwrap_or(false)
    })
}

pub(crate) fn take_pending_signals() -> Vec<c_int> {
    // Fast path: no pending signals means no allocation or atomic swap.
    if super::interface::pending_signal_bits() == 0 {
        return Vec::new();
    }
    let bits = super::interface::take_pending_signal_bits();
    if bits == 0 {
        return Vec::new();
    }
    SUPPORTED_TRAP_SIGNALS
        .iter()
        .copied()
        .filter(|signal| {
            signal_mask(*signal)
                .map(|mask| bits & mask != 0)
                .unwrap_or(false)
        })
        .collect()
}

pub(crate) fn query_signal_disposition(signal: c_int) -> SysResult<bool> {
    let prev = interface::signal(signal, SIG_IGN_HANDLER);
    if prev == SIG_ERR_HANDLER {
        return Err(last_error());
    }
    let _ = interface::signal(signal, prev);
    Ok(prev == SIG_IGN_HANDLER)
}

pub(crate) fn interrupted(error: &SysError) -> bool {
    error.is_eintr()
}
#[cfg(test)]
impl ChildHandle {
    pub(crate) fn wait_with_output(self) -> SysResult<ChildOutput> {
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

    pub(crate) fn wait(self) -> SysResult<ChildExitStatus> {
        if let Some(fd) = self.stdout_fd {
            close_fd(fd)?;
        }
        let ws = wait_pid(self.pid, false)?.expect("child status");
        Ok(ChildExitStatus {
            code: decode_wait_status(ws.status),
        })
    }
}
pub(crate) fn fork_process() -> SysResult<Pid> {
    let pid = interface::fork();
    if pid < 0 { Err(last_error()) } else { Ok(pid) }
}

pub fn exit_process(status: c_int) -> ! {
    interface::exit_process(status);
    unreachable!()
}

pub(crate) fn spawn_child(
    program: &[u8],
    argv: &[&[u8]],
    env_vars: Option<&[(&[u8], &[u8])]>,
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
        let argv_owned: Vec<Vec<u8>> = argv.iter().map(|s| s.to_vec()).collect();
        let _ = exec_replace(program, argv_owned);
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
pub(crate) fn getrlimit(resource: i32) -> SysResult<(u64, u64)> {
    let mut rlim = std::mem::MaybeUninit::<libc::rlimit>::zeroed();
    let rc = unsafe { libc::getrlimit(resource as _, rlim.as_mut_ptr()) };
    if rc < 0 {
        return Err(last_error());
    }
    let rlim = unsafe { rlim.assume_init() };
    Ok((rlim.rlim_cur as u64, rlim.rlim_max as u64))
}

pub(crate) fn setrlimit(resource: i32, soft: u64, hard: u64) -> SysResult<()> {
    let rlim = libc::rlimit {
        rlim_cur: soft as _,
        rlim_max: hard as _,
    };
    let rc = unsafe { libc::setrlimit(resource as _, &rlim) };
    if rc < 0 {
        return Err(last_error());
    }
    Ok(())
}
pub(crate) fn exec_replace(file: &[u8], argv: Vec<Vec<u8>>) -> SysResult<()> {
    let c_file = crate::bstr::to_cstring(file).map_err(|_| SysError::NulInPath)?;
    let mut owned = Vec::with_capacity(argv.len());
    for arg in argv {
        owned.push(crate::bstr::vec_to_cstring(arg).map_err(|_| SysError::NulInPath)?);
    }

    let mut pointers: Vec<*const c_char> = owned.iter().map(|arg| arg.as_ptr()).collect();
    pointers.push(std::ptr::null());

    #[cfg(coverage)]
    super::interface::flush_coverage();
    let result = interface::execvp(c_file.as_ptr(), pointers.as_ptr());
    if result == -1 {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub(crate) fn exec_replace_with_env(
    file: &[u8],
    argv: Vec<Vec<u8>>,
    env: Vec<(Vec<u8>, Vec<u8>)>,
) -> SysResult<()> {
    let c_file = crate::bstr::to_cstring(file).map_err(|_| SysError::NulInPath)?;
    let mut argv_owned = Vec::with_capacity(argv.len());
    for arg in argv {
        argv_owned.push(crate::bstr::vec_to_cstring(arg).map_err(|_| SysError::NulInPath)?);
    }
    let mut argp: Vec<*const c_char> = argv_owned.iter().map(|a| a.as_ptr()).collect();
    argp.push(std::ptr::null());

    let mut env_owned = Vec::with_capacity(env.len());
    for (mut k, v) in env {
        k.reserve(v.len() + 1);
        k.push(b'=');
        k.extend_from_slice(&v);
        env_owned.push(crate::bstr::vec_to_cstring(k).map_err(|_| SysError::NulInPath)?);
    }
    let mut envp: Vec<*const c_char> = env_owned.iter().map(|e| e.as_ptr()).collect();
    envp.push(std::ptr::null());

    #[cfg(coverage)]
    super::interface::flush_coverage();
    let result = interface::execve(c_file.as_ptr(), argp.as_ptr(), envp.as_ptr());
    if result == -1 {
        Err(last_error())
    } else {
        Ok(())
    }
}

pub(crate) fn decode_wait_status(status: c_int) -> i32 {
    if wifexited(status) {
        wexitstatus(status)
    } else if wifsignaled(status) {
        128 + wtermsig(status)
    } else {
        status
    }
}

#[cfg(test)]
pub(crate) fn format_signal_exit(status: c_int) -> Option<Vec<u8>> {
    if wifsignaled(status) {
        let mut buf = b"terminated by signal ".to_vec();
        crate::bstr::push_i64(&mut buf, wtermsig(status) as i64);
        Some(buf)
    } else {
        None
    }
}

pub(crate) fn signal_name(sig: c_int) -> &'static [u8] {
    match sig {
        SIGHUP => b"SIGHUP",
        SIGINT => b"SIGINT",
        SIGQUIT => b"SIGQUIT",
        SIGILL => b"SIGILL",
        SIGABRT => b"SIGABRT",
        SIGFPE => b"SIGFPE",
        SIGKILL => b"SIGKILL",
        SIGBUS => b"SIGBUS",
        SIGUSR1 => b"SIGUSR1",
        SIGSEGV => b"SIGSEGV",
        SIGUSR2 => b"SIGUSR2",
        SIGPIPE => b"SIGPIPE",
        SIGALRM => b"SIGALRM",
        SIGTERM => b"SIGTERM",
        SIGCHLD => b"SIGCHLD",
        SIGSTOP => b"SIGSTOP",
        SIGCONT => b"SIGCONT",
        SIGTRAP => b"SIGTRAP",
        SIGTSTP => b"SIGTSTP",
        SIGTTIN => b"SIGTTIN",
        SIGTTOU => b"SIGTTOU",
        SIGSYS => b"SIGSYS",
        _ => b"UNKNOWN",
    }
}

pub(crate) fn all_signal_names() -> &'static [(&'static [u8], c_int)] {
    &[
        (b"HUP", SIGHUP),
        (b"INT", SIGINT),
        (b"QUIT", SIGQUIT),
        (b"ILL", SIGILL),
        (b"ABRT", SIGABRT),
        (b"FPE", SIGFPE),
        (b"KILL", SIGKILL),
        (b"BUS", SIGBUS),
        (b"USR1", SIGUSR1),
        (b"SEGV", SIGSEGV),
        (b"USR2", SIGUSR2),
        (b"PIPE", SIGPIPE),
        (b"ALRM", SIGALRM),
        (b"TERM", SIGTERM),
        (b"CHLD", SIGCHLD),
        (b"STOP", SIGSTOP),
        (b"CONT", SIGCONT),
        (b"TRAP", SIGTRAP),
        (b"TSTP", SIGTSTP),
        (b"TTIN", SIGTTIN),
        (b"TTOU", SIGTTOU),
        (b"SYS", SIGSYS),
    ]
}

// Wait-status decoders. In production these wrap the host libc's
// `WIFEXITED(3)` / `WEXITSTATUS(3)` macros directly; in tests they
// interpret the synthetic tag encoding produced by the `encode_*`
// helpers in `test_support`. These are pure-logic operations (no
// syscall), so they live here rather than behind the `#[cfg]`-gated
// `interface::*` wrappers that only cover real syscalls.

#[cfg(not(test))]
fn wifexited(status: c_int) -> bool {
    libc::WIFEXITED(status)
}

#[cfg(test)]
fn wifexited(status: c_int) -> bool {
    use super::test_support::{WAIT_TAG_EXITED, WAIT_TAG_MASK};
    (status as u32) & WAIT_TAG_MASK == WAIT_TAG_EXITED
}

#[cfg(not(test))]
pub(crate) fn wexitstatus(status: c_int) -> i32 {
    libc::WEXITSTATUS(status)
}

#[cfg(test)]
pub(crate) fn wexitstatus(status: c_int) -> i32 {
    (status & 0xff) as i32
}

#[cfg(not(test))]
pub(crate) fn wifsignaled(status: c_int) -> bool {
    libc::WIFSIGNALED(status)
}

#[cfg(test)]
pub(crate) fn wifsignaled(status: c_int) -> bool {
    use super::test_support::{WAIT_TAG_MASK, WAIT_TAG_SIGNALED};
    (status as u32) & WAIT_TAG_MASK == WAIT_TAG_SIGNALED
}

#[cfg(not(test))]
pub(crate) fn wtermsig(status: c_int) -> i32 {
    libc::WTERMSIG(status)
}

#[cfg(test)]
pub(crate) fn wtermsig(status: c_int) -> i32 {
    (status & 0xff) as i32
}

#[cfg(not(test))]
pub(crate) fn wifstopped(status: c_int) -> bool {
    libc::WIFSTOPPED(status)
}

#[cfg(test)]
pub(crate) fn wifstopped(status: c_int) -> bool {
    use super::test_support::{WAIT_TAG_MASK, WAIT_TAG_STOPPED};
    (status as u32) & WAIT_TAG_MASK == WAIT_TAG_STOPPED
}

#[cfg(not(test))]
pub(crate) fn wifcontinued(status: c_int) -> bool {
    libc::WIFCONTINUED(status)
}

#[cfg(test)]
pub(crate) fn wifcontinued(status: c_int) -> bool {
    use super::test_support::{WAIT_TAG_CONTINUED, WAIT_TAG_MASK};
    (status as u32) & WAIT_TAG_MASK == WAIT_TAG_CONTINUED
}

#[cfg(not(test))]
pub(crate) fn wstopsig(status: c_int) -> i32 {
    libc::WSTOPSIG(status)
}

#[cfg(test)]
pub(crate) fn wstopsig(status: c_int) -> i32 {
    (status & 0xff) as i32
}

#[cfg(test)]
pub(crate) fn shell_name_from_args(args: &[Vec<u8>]) -> &[u8] {
    args.first().map(|s| s.as_slice()).unwrap_or(b"meiksh")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support;
    use crate::trace_entries;

    use super::super::constants::{
        EINTR, ENOENT, F_GETFL, F_SETFL, O_NONBLOCK, SIGABRT, SIGALRM, SIGCHLD, SIGCONT, SIGHUP,
        SIGINT, SIGPIPE, SIGQUIT, SIGTERM, SIGTRAP, SIGUSR1, SIGUSR2, STDIN_FILENO, STDOUT_FILENO,
    };
    use super::super::error::SysError;
    use super::super::types::{ChildHandle, FdReader};
    use crate::sys::env::home_dir_for_user;
    use crate::sys::fd_io::ensure_blocking_read_fd;
    use crate::sys::fs::{canonicalize, get_cwd};

    #[test]
    fn decodes_wait_status_shapes() {
        test_support::assert_no_syscalls(|| {
            assert_eq!(decode_wait_status(test_support::encode_exited(0)), 0);
            assert_eq!(decode_wait_status(test_support::encode_exited(7)), 7);
            assert_eq!(
                format_signal_exit(test_support::encode_signaled(9)),
                Some(b"terminated by signal 9".to_vec())
            );
            assert_eq!(format_signal_exit(test_support::encode_exited(0)), None);
        });
    }

    #[test]
    fn shell_name_from_args_returns_first_arg_or_default() {
        assert_eq!(
            shell_name_from_args(&[b"meiksh".to_vec(), b"-c".to_vec()]),
            b"meiksh"
        );
        assert_eq!(shell_name_from_args(&[]), b"meiksh");
    }

    #[test]
    fn execvp_failure_returns_minus_one() {
        test_support::run_trace(trace_entries![execvp(_, _) -> err(ENOENT)], || {
            assert!(
                exec_replace(b"meiksh-command-that-does-not-exist", vec![b"x".to_vec()]).is_err()
            );
        });
    }

    #[test]
    fn wait_pid_error_surfaces_errno() {
        test_support::run_trace(
            trace_entries![waitpid(int(999_999), _, _) -> err(libc::ECHILD)],
            || {
                assert!(wait_pid(999_999, false).is_err());
            },
        );
    }

    #[test]
    fn exec_replace_rejects_nul_in_program_and_args() {
        let err = exec_replace(b"bad\0program", vec![]).unwrap_err();
        assert_eq!(err, SysError::NulInPath);
        assert!(err.errno().is_none());
        assert!(!err.is_enoent());
        assert!(err.strerror().windows(4).any(|w| w == b"null"));

        let err = exec_replace(b"ok", vec![b"bad\0arg".to_vec()]).unwrap_err();
        assert_eq!(err, SysError::NulInPath);
    }

    #[test]
    fn sys_success_branches_cover_fd_helpers() {
        test_support::run_trace(
            trace_entries![
                pipe() -> fds(20, 21),
                dup2(fd(20), fd(20)) -> fd(20),
                close(fd(20)) -> 0,
                close(fd(21)) -> 0,
            ],
            || {
                let (read_fd, write_fd) = create_pipe().expect("pipe");
                duplicate_fd(read_fd, read_fd).expect("dup self");
                close_fd(read_fd).expect("close read");
                close_fd(write_fd).expect("close write");
            },
        );
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
        test_support::run_trace(trace_entries![getpid() -> pid(4242)], || {
            assert_eq!(current_pid(), 4242);
        });
    }

    #[test]
    fn success_wait_and_signal() {
        // `waitpid` -> status(9) encodes WIFEXITED with exit code 9 in the
        // synthetic wait-status encoding (see `encode_exited`). The test
        // just verifies that the value is returned verbatim to the caller
        // through `wait_pid`.
        test_support::run_trace(
            trace_entries![
                waitpid(1, _) -> status(9),
                kill(int(1), int(0)) -> 0,
            ],
            || {
                let ws = wait_pid(1, false).expect("wait").expect("status");
                assert_eq!(ws.pid, 1);
                assert!(super::wifexited(ws.status));
                assert_eq!(super::wexitstatus(ws.status), 9);
                assert!(send_signal(1, 0).is_ok());
            },
        );
    }

    #[test]
    fn success_file_io() {
        test_support::run_trace(
            trace_entries![
                read(fd(0), _) -> bytes(b"X"),
                read(fd(0), _) -> bytes(b"X"),
            ],
            || {
                let mut buffer = [0u8; 1];
                assert_eq!(read_fd(0, &mut buffer).expect("read"), 1);
                assert_eq!(buffer, [b'X']);
                let mut reader = FdReader::new(0);
                assert_eq!(reader.read(&mut buffer).expect("reader read"), 1);
                assert_eq!(buffer, [b'X']);
            },
        );
    }

    #[test]
    fn success_exec() {
        test_support::run_trace(trace_entries![execvp(_, _) -> 0], || {
            assert!(exec_replace(b"echo", vec![b"hello".to_vec(), b"world".to_vec()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        test_support::assert_no_syscalls(|| {
            // 0x7f has no tag bits set, so it matches neither exited,
            // signaled, stopped, nor continued -- exercising the fallback
            // branch that returns the raw status unchanged.
            assert_eq!(decode_wait_status(0x7f), 0x7f);
        });
    }

    #[test]
    fn signal_handler_installation_succeeds() {
        use test_support::run_trace;

        run_trace(
            trace_entries![
                signal(int(SIGINT), _) -> 0,
                signal(int(SIGTERM), _) -> 0,
                signal(int(SIGQUIT), _) -> 0,
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
        use test_support::run_trace;

        run_trace(
            trace_entries![
                signal(int(SIGINT), _) -> err(libc::EINVAL),
                signal(int(SIGTERM), _) -> err(libc::EINVAL),
                signal(int(SIGQUIT), _) -> err(libc::EINVAL),
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
        test_support::run_trace(trace_entries![getpid() -> pid(1)], || {
            assert_eq!(current_pid(), 1);
        });
    }

    #[test]
    fn error_wait_and_signal() {
        test_support::run_trace(
            trace_entries![
                kill(int(1), int(0)) -> err(libc::EPERM),
                waitpid(1, _) -> err(libc::ECHILD),
            ],
            || {
                assert!(send_signal(1, 0).is_err());
                assert!(wait_pid(1, false).is_err());
            },
        );
    }

    #[test]
    fn error_file_io() {
        test_support::run_trace(trace_entries![read(fd(0), _) -> err(libc::EIO)], || {
            assert!(read_fd(0, &mut [0u8; 1]).is_err());
        });
    }

    #[test]
    fn error_exec() {
        test_support::run_trace(trace_entries![execvp(_, _) -> err(ENOENT)], || {
            assert!(exec_replace(b"echo", vec![b"hi".to_vec()]).is_err());
        });
    }

    #[test]
    fn decode_wait_status_signal_terminated() {
        test_support::assert_no_syscalls(|| {
            assert_eq!(decode_wait_status(test_support::encode_signaled(9)), 137);
        });
    }

    #[test]
    fn query_signal_disposition_error() {
        use test_support::run_trace;
        run_trace(
            trace_entries![
                signal(int(SIGINT), _) -> err(libc::EINVAL),
            ],
            || {
                assert!(query_signal_disposition(SIGINT).is_err());
            },
        );
    }

    #[test]
    fn ensure_blocking_setfl_error() {
        use test_support::run_trace;
        run_trace(
            trace_entries![
                fstat(fd(STDIN_FILENO), _) -> stat_char,
                isatty(fd(STDIN_FILENO)) -> 1,
                fcntl(fd(STDIN_FILENO), int(F_GETFL), int(0)) -> int((O_NONBLOCK | 0o2) as i64),
                fcntl(fd(STDIN_FILENO), int(F_SETFL), int(0o2)) -> err(libc::EIO),
            ],
            || {
                assert!(ensure_blocking_read_fd(STDIN_FILENO).is_err());
            },
        );
    }

    #[test]
    fn child_handle_wait_with_output_reads_pipe() {
        use test_support::run_trace;
        run_trace(
            trace_entries![
                read(fd(10), _) -> bytes(b"hello"),
                read(fd(10), _) -> 0,
                close(fd(10)) -> 0,
                waitpid(int(99), _, _) -> status(0),
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
        use test_support::run_trace;
        run_trace(
            trace_entries![
                close(fd(10)) -> 0,
                waitpid(int(99), _, _) -> status(0),
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
        use test_support::run_trace;
        run_trace(
            trace_entries![
                pipe(_) -> fds(10, 11),
                fork() -> pid(100), child: [
                    setpgid(int(0), int(42)) -> 0,
                    dup2(fd(5), fd(STDIN_FILENO)) -> fd(STDIN_FILENO),
                    close(fd(5)) -> 0,
                    close(fd(10)) -> 0,
                    dup2(fd(11), fd(STDOUT_FILENO)) -> fd(STDOUT_FILENO),
                    close(fd(11)) -> 0,
                    dup2(fd(7), fd(2)) -> fd(2),
                    close(fd(7)) -> 0,
                    setenv(str(b"VAR"), str(b"val")) -> 0,
                    execvp(_, _) -> int(-1),
                ],
                close(fd(5)) -> 0,
                close(fd(11)) -> 0,
            ],
            || {
                let handle = spawn_child(
                    b"echo",
                    &[b"echo" as &[u8], b"hello"],
                    Some(&[(b"VAR" as &[u8], b"val" as &[u8])]),
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
    fn spawn_child_no_pipe_stdout() {
        use test_support::run_trace;
        run_trace(
            trace_entries![
                fork() -> pid(100), child: [
                    execvp(_, _) -> int(-1),
                ],
            ],
            || {
                let handle = spawn_child(
                    b"echo",
                    &[b"echo" as &[u8], b"hello"],
                    None,
                    &[],
                    None,
                    false,
                    None,
                )
                .expect("spawn");
                assert_eq!(handle.pid, 100);
                assert_eq!(handle.stdout_fd, None);
            },
        );
    }

    #[test]
    fn trace_getcwd_erange_and_pipe_err() {
        test_support::run_trace(
            trace_entries![
                getcwd() -> err(libc::ERANGE),
                pipe() -> err(libc::EMFILE),
            ],
            || {
                assert!(get_cwd().is_err());
                assert!(create_pipe().is_err());
            },
        );
    }

    #[test]
    fn trace_realpath_resolved_and_err() {
        test_support::run_trace(
            trace_entries![
                realpath(_, _) -> realpath("/resolved"),
                realpath(_, _) -> err(ENOENT),
            ],
            || {
                assert_eq!(canonicalize(b"/foo").expect("resolve"), b"/resolved");
                assert!(canonicalize(b"/bad").is_err());
            },
        );
    }

    #[test]
    fn trace_getpwnam_null_str() {
        use test_support::{ArgMatcher, TraceResult, t};
        test_support::run_trace(
            trace_entries![
                ..vec![t(
                    "getpwnam",
                    vec![ArgMatcher::Str(b"nobody".to_vec())],
                    TraceResult::NullStr,
                )],
            ],
            || {
                assert!(home_dir_for_user(b"nobody").is_none());
            },
        );
    }

    #[test]
    fn trace_waitpid_fallthrough() {
        test_support::run_trace(
            trace_entries![
                waitpid(int(-1), _, _) -> 0,
            ],
            || {
                let r = wait_pid(-1, true);
                assert!(r.is_ok());
            },
        );
    }

    #[test]
    fn trace_signal_default_fallthrough() {
        test_support::run_trace(
            trace_entries![
                signal(int(SIGINT), _) -> 0,
            ],
            || {
                let _ = default_signal_action(SIGINT);
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
            trace_entries![
                execve(_, _) -> err(ENOENT),
            ],
            || {
                let result = exec_replace_with_env(b"/nonexistent", vec![b"test".to_vec()], vec![]);
                assert!(result.is_err());
            },
        );
    }
}
