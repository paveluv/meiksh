use libc::{self, c_char, c_int, c_long, mode_t};
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::constants::*;
use super::error::{SysError, SysResult};
use super::types::{ClockTicks, FileModeMask, Pid};

#[derive(Clone, Copy)]
pub(crate) struct SystemInterface {
    pub(crate) getpid: fn() -> Pid,
    pub(crate) getppid: fn() -> Pid,
    pub(crate) waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    pub(crate) kill: fn(Pid, c_int) -> c_int,
    pub(crate) signal: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
    pub(crate) isatty: fn(c_int) -> c_int,
    pub(crate) tcgetpgrp: fn(c_int) -> Pid,
    pub(crate) tcsetpgrp: fn(c_int, Pid) -> c_int,
    pub(crate) setpgid: fn(Pid, Pid) -> c_int,
    pub(crate) pipe: fn(&mut [c_int; 2]) -> c_int,
    pub(crate) dup2: fn(c_int, c_int) -> c_int,
    pub(crate) close: fn(c_int) -> c_int,
    pub(crate) fcntl: fn(c_int, c_int, c_int) -> c_int,
    pub(crate) read: fn(c_int, &mut [u8]) -> isize,
    pub(crate) umask: fn(FileModeMask) -> FileModeMask,
    pub(crate) times: fn(*mut libc::tms) -> ClockTicks,
    pub(crate) sysconf: fn(c_int) -> c_long,
    pub(crate) execvp: fn(*const c_char, *const *const c_char) -> c_int,
    pub(crate) execve: fn(*const c_char, *const *const c_char, *const *const c_char) -> c_int,
    // Filesystem
    pub(crate) open: fn(*const c_char, c_int, mode_t) -> c_int,
    pub(crate) write: fn(c_int, &[u8]) -> isize,
    pub(crate) stat: fn(*const c_char, *mut libc::stat) -> c_int,
    pub(crate) lstat: fn(*const c_char, *mut libc::stat) -> c_int,
    pub(crate) fstat: fn(c_int, *mut libc::stat) -> c_int,
    pub(crate) access: fn(*const c_char, c_int) -> c_int,
    pub(crate) chdir: fn(*const c_char) -> c_int,
    pub(crate) getcwd: fn(*mut c_char, usize) -> *mut c_char,
    pub(crate) opendir: fn(*const c_char) -> *mut libc::DIR,
    pub(crate) readdir: fn(*mut libc::DIR) -> *mut libc::dirent,
    pub(crate) closedir: fn(*mut libc::DIR) -> c_int,
    pub(crate) realpath: fn(*const c_char, *mut c_char) -> *mut c_char,
    pub(crate) unlink: fn(*const c_char) -> c_int,
    // Process
    pub(crate) fork: fn() -> Pid,
    pub(crate) exit_process: fn(c_int),
    // Environment
    pub(crate) setenv: fn(&[u8], &[u8]) -> SysResult<()>,
    pub(crate) unsetenv: fn(&[u8]) -> SysResult<()>,
    pub(crate) getenv: fn(&[u8]) -> Option<Vec<u8>>,
    pub(crate) get_environ: fn() -> HashMap<Vec<u8>, Vec<u8>>,
    // Terminal attributes
    pub(crate) tcgetattr: fn(c_int, *mut libc::termios) -> c_int,
    pub(crate) tcsetattr: fn(c_int, c_int, *const libc::termios) -> c_int,
    // User database
    pub(crate) getpwnam: fn(&[u8]) -> Option<Vec<u8>>,
    // Signal state
    pub(crate) pending_signal_bits: fn() -> usize,
    pub(crate) take_pending_signal_bits: fn() -> usize,
    pub(crate) monotonic_clock_ns: fn() -> u64,
    // Locale
    pub(crate) setup_locale: fn(),
    pub(crate) classify_byte: fn(&[u8], u8) -> bool,
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
        lstat: |path, buf| unsafe { libc::lstat(path, buf) },
        fstat: |fd, buf| unsafe { libc::fstat(fd, buf) },
        access: |path, mode| unsafe { libc::access(path, mode) },
        chdir: |path| unsafe { libc::chdir(path) },
        getcwd: |buf, size| unsafe { libc::getcwd(buf, size) },
        opendir: |path| unsafe { libc::opendir(path) },
        readdir: |dirp| unsafe { libc::readdir(dirp) },
        closedir: |dirp| unsafe { libc::closedir(dirp) },
        realpath: |path, resolved| unsafe { libc::realpath(path, resolved) },
        unlink: |path| unsafe { libc::unlink(path) },
        fork: || unsafe { libc::fork() },
        exit_process: |status| {
            #[cfg(coverage)]
            flush_coverage();
            unsafe { libc::_exit(status) }
        },
        setenv: |key, value| {
            let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
            let c_value = crate::bstr::to_cstring(value).map_err(|_| SysError::NulInPath)?;
            let result = unsafe { libc::setenv(c_key.as_ptr(), c_value.as_ptr(), 1) };
            if result == 0 {
                Ok(())
            } else {
                Err(last_error())
            }
        },
        unsetenv: |key| {
            let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
            let result = unsafe { libc::unsetenv(c_key.as_ptr()) };
            if result == 0 {
                Ok(())
            } else {
                Err(last_error())
            }
        },
        getenv: |key| {
            let c_key = crate::bstr::to_cstring(key).ok()?;
            let ptr = unsafe { libc::getenv(c_key.as_ptr()) };
            if ptr.is_null() {
                None
            } else {
                Some(crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(ptr) }))
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
        },
        tcgetattr: |fd, termios_p| unsafe { libc::tcgetattr(fd, termios_p) },
        tcsetattr: |fd, action, termios_p| unsafe { libc::tcsetattr(fd, action, termios_p) },
        getpwnam: |name| {
            let c_name = crate::bstr::to_cstring(name).ok()?;
            let pw = unsafe { libc::getpwnam(c_name.as_ptr()) };
            if pw.is_null() {
                return None;
            }
            let dir = unsafe { CStr::from_ptr((*pw).pw_dir) };
            Some(crate::bstr::bytes_from_cstr(dir))
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
        classify_byte: |class, byte| {
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
            let wc = byte as u32;
            unsafe {
                match class {
                    b"alnum" => iswalnum(wc) != 0,
                    b"alpha" => iswalpha(wc) != 0,
                    b"blank" => iswblank(wc) != 0,
                    b"cntrl" => iswcntrl(wc) != 0,
                    b"digit" => iswdigit(wc) != 0,
                    b"graph" => iswgraph(wc) != 0,
                    b"lower" => iswlower(wc) != 0,
                    b"print" => iswprint(wc) != 0,
                    b"punct" => iswpunct(wc) != 0,
                    b"space" => iswspace(wc) != 0,
                    b"upper" => iswupper(wc) != 0,
                    b"xdigit" => iswxdigit(wc) != 0,
                    _ => false,
                }
            }
        },
    }
}
pub(crate) fn signal_mask(signal: c_int) -> Option<usize> {
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

pub(crate) extern "C" fn record_signal(sig: c_int) {
    if let Some(mask) = signal_mask(sig) {
        PENDING_SIGNALS.fetch_or(mask, Ordering::SeqCst);
    }
}

pub(crate) fn sys_interface() -> SystemInterface {
    #[cfg(test)]
    {
        return super::test_support::current_interface()
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
}

pub(crate) fn last_error() -> SysError {
    #[cfg(test)]
    {
        return super::test_support::take_test_error();
    }

    #[cfg(not(test))]
    SysError::Errno(unsafe { *errno_ptr() })
}

#[cfg(coverage)]
pub(crate) fn flush_coverage() {
    unsafe {
        unsafe extern "C" {
            fn __llvm_profile_write_file() -> c_int;
        }
        __llvm_profile_write_file();
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;

    use crate::sys::test_support;
    use crate::trace_entries;

    use super::*;
    use crate::sys::*;

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
    fn default_interface_monotonic_clock_ns() {
        test_support::with_test_interface(default_interface(), || {
            let ns = monotonic_clock_ns();
            assert!(ns > 0, "monotonic clock should return positive nanoseconds");
        });
    }

    #[test]
    fn default_interface_classify_byte_ascii() {
        test_support::with_test_interface(default_interface(), || {
            setup_locale();
            assert!(classify_byte(b"alpha", b'a'));
            assert!(classify_byte(b"alpha", b'Z'));
            assert!(!classify_byte(b"alpha", b'5'));
            assert!(classify_byte(b"alnum", b'9'));
            assert!(!classify_byte(b"alnum", b'!'));
            assert!(classify_byte(b"blank", b' '));
            assert!(classify_byte(b"blank", b'\t'));
            assert!(!classify_byte(b"blank", b'a'));
            assert!(classify_byte(b"cntrl", 0x01));
            assert!(!classify_byte(b"cntrl", b'a'));
            assert!(classify_byte(b"digit", b'0'));
            assert!(!classify_byte(b"digit", b'x'));
            assert!(classify_byte(b"graph", b'!'));
            assert!(!classify_byte(b"graph", b' '));
            assert!(classify_byte(b"lower", b'a'));
            assert!(!classify_byte(b"lower", b'A'));
            assert!(classify_byte(b"print", b' '));
            assert!(classify_byte(b"print", b'a'));
            assert!(!classify_byte(b"print", 0x01));
            assert!(classify_byte(b"punct", b'.'));
            assert!(!classify_byte(b"punct", b'a'));
            assert!(classify_byte(b"space", b'\n'));
            assert!(!classify_byte(b"space", b'a'));
            assert!(classify_byte(b"upper", b'A'));
            assert!(!classify_byte(b"upper", b'a'));
            assert!(classify_byte(b"xdigit", b'f'));
            assert!(!classify_byte(b"xdigit", b'g'));
            assert!(!classify_byte(b"bogus", b'a'));
        });
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
        assert!(
            catch_unwind(AssertUnwindSafe(|| (tbl.lstat)(
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
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.unlink)(std::ptr::null()))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.fork)())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.exit_process)(0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.setenv)(b"k", b"v"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.unsetenv)(b"k"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getenv)(b"k"))).is_err());
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
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.getpwnam)(b"nobody"))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.monotonic_clock_ns)())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| (tbl.setup_locale)())).is_err());
    }

    #[test]
    fn default_interface_env_errors() {
        let tbl = default_interface();
        // EINVAL cases: empty key, or key containing =
        assert!((tbl.setenv)(b"", b"val").is_err());
        assert!((tbl.setenv)(b"k=v", b"val").is_err());
        assert!((tbl.unsetenv)(b"").is_err());
        assert!((tbl.unsetenv)(b"k=v").is_err());
    }

    #[test]
    fn default_interface_lstat_and_unlink_error_on_null() {
        let tbl = default_interface();
        unsafe {
            let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
            assert!((tbl.lstat)(std::ptr::null(), buf.as_mut_ptr()) < 0);
            assert!((tbl.unlink)(std::ptr::null()) < 0);
        }
    }

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
