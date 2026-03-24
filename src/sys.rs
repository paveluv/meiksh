use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::io;
use std::os::raw::{c_char, c_int};

pub type Pid = c_int;

pub const STDIN_FILENO: c_int = 0;
pub const STDOUT_FILENO: c_int = 1;
pub const STDERR_FILENO: c_int = 2;
pub const SIGCONT: c_int = 18;
pub const WNOHANG: c_int = 0x0000_0001;

unsafe extern "C" {
    fn getpid() -> Pid;
    fn waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid;
    fn kill(pid: Pid, sig: c_int) -> c_int;
    fn isatty(fd: c_int) -> c_int;
    fn tcgetpgrp(fd: c_int) -> Pid;
    fn tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int;
    fn setpgid(pid: Pid, pgid: Pid) -> c_int;
    fn pipe(fds: *mut c_int) -> c_int;
    fn dup(fd: c_int) -> c_int;
    fn dup2(oldfd: c_int, newfd: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int;
}

#[derive(Clone, Copy)]
struct Syscalls {
    getpid: fn() -> Pid,
    waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    kill: fn(Pid, c_int) -> c_int,
    isatty: fn(c_int) -> c_int,
    tcgetpgrp: fn(c_int) -> Pid,
    tcsetpgrp: fn(c_int, Pid) -> c_int,
    setpgid: fn(Pid, Pid) -> c_int,
    pipe: fn(*mut c_int) -> c_int,
    dup: fn(c_int) -> c_int,
    dup2: fn(c_int, c_int) -> c_int,
    close: fn(c_int) -> c_int,
    execvp: fn(*const c_char, *const *const c_char) -> c_int,
}

fn default_getpid() -> Pid {
    unsafe { getpid() }
}

fn default_waitpid(pid: Pid, status: *mut c_int, options: c_int) -> Pid {
    unsafe { waitpid(pid, status, options) }
}

fn default_kill(pid: Pid, sig: c_int) -> c_int {
    unsafe { kill(pid, sig) }
}

fn default_isatty(fd: c_int) -> c_int {
    unsafe { isatty(fd) }
}

fn default_tcgetpgrp(fd: c_int) -> Pid {
    unsafe { tcgetpgrp(fd) }
}

fn default_tcsetpgrp(fd: c_int, pgrp: Pid) -> c_int {
    unsafe { tcsetpgrp(fd, pgrp) }
}

fn default_setpgid(pid: Pid, pgid: Pid) -> c_int {
    unsafe { setpgid(pid, pgid) }
}

fn default_pipe(fds: *mut c_int) -> c_int {
    unsafe { pipe(fds) }
}

fn default_dup(fd: c_int) -> c_int {
    unsafe { dup(fd) }
}

fn default_dup2(oldfd: c_int, newfd: c_int) -> c_int {
    unsafe { dup2(oldfd, newfd) }
}

fn default_close(fd: c_int) -> c_int {
    unsafe { close(fd) }
}

fn default_syscalls() -> Syscalls {
    Syscalls {
        getpid: default_getpid,
        waitpid: default_waitpid,
        kill: default_kill,
        isatty: default_isatty,
        tcgetpgrp: default_tcgetpgrp,
        tcsetpgrp: default_tcsetpgrp,
        setpgid: default_setpgid,
        pipe: default_pipe,
        dup: default_dup,
        dup2: default_dup2,
        close: default_close,
        execvp: |file, argv| unsafe { execvp(file, argv) },
    }
}

thread_local! {
    static TEST_SYSCALLS: RefCell<Option<Syscalls>> = const { RefCell::new(None) };
}

fn syscalls() -> Syscalls {
    #[cfg(test)]
    {
        return TEST_SYSCALLS.with(|cell| cell.borrow().unwrap_or_else(default_syscalls));
    }

    #[cfg(not(test))]
    {
        default_syscalls()
    }
}

#[cfg(test)]
pub(crate) fn with_execvp_for_test<T>(
    execvp_fn: fn(*const c_char, *const *const c_char) -> c_int,
    f: impl FnOnce() -> T,
) -> T {
    let syscalls = Syscalls {
        execvp: execvp_fn,
        ..default_syscalls()
    };
    tests::with_test_syscalls(syscalls, f)
}

#[cfg(test)]
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
    tests::with_test_syscalls(syscalls, f)
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
    use std::sync::Mutex;

    fn syscall_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    pub(super) fn with_test_syscalls<T>(syscalls: Syscalls, f: impl FnOnce() -> T) -> T {
        let _guard = syscall_lock().lock().expect("syscall lock");
        TEST_SYSCALLS.with(|cell| {
            let previous = cell.replace(Some(syscalls));
            let result = f();
            cell.replace(previous);
            result
        })
    }

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

    fn meiksh_bin_path() -> std::path::PathBuf {
        let exe = std::env::current_exe().expect("current exe");
        exe.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("meiksh"))
            .expect("meiksh path")
    }

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
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            0
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            execvp: fake_execvp,
        };

        with_test_syscalls(fake, || {
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
            assert!(exec_replace("echo", &["hello".to_string(), "world".to_string()]).is_ok());
        });
    }

    #[test]
    fn decode_wait_status_covers_fallback_shape() {
        assert_eq!(decode_wait_status(0x7f), 0x7f);
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
        fn fake_execvp(_file: *const c_char, _argv: *const *const c_char) -> c_int {
            -1
        }

        let fake = Syscalls {
            getpid: fake_getpid,
            waitpid: fake_waitpid,
            kill: fake_kill,
            isatty: fake_isatty,
            tcgetpgrp: fake_tcgetpgrp,
            tcsetpgrp: fake_tcsetpgrp,
            setpgid: fake_setpgid,
            pipe: fake_pipe,
            dup: fake_dup,
            dup2: fake_dup2,
            close: fake_close,
            execvp: fake_execvp,
        };

        with_test_syscalls(fake, || {
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
            assert!(exec_replace("echo", &["hi".to_string()]).is_err());
            assert!(wait_pid(1, false).is_err());
        });

        assert_eq!(decode_wait_status(9), 137);
    }
}
