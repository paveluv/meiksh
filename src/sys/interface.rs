use libc::{self, c_char, c_int, c_long, mode_t};
use std::collections::HashMap;
use std::ffi::CStr;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::constants::{
    SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT, SIGFPE, SIGHUP, SIGILL, SIGINT, SIGPIPE, SIGQUIT,
    SIGSEGV, SIGSYS, SIGTERM, SIGTRAP, SIGTSTP, SIGTTIN, SIGTTOU, SIGUSR1, SIGUSR2,
};
use super::error::{SysError, SysResult};
use super::types::{ClockTicks, FileModeMask, Pid};

#[derive(Clone, Copy)]
pub(super) struct SystemInterface {
    pub(super) getpid: fn() -> Pid,
    pub(super) getppid: fn() -> Pid,
    pub(super) waitpid: fn(Pid, *mut c_int, c_int) -> Pid,
    pub(super) kill: fn(Pid, c_int) -> c_int,
    pub(super) signal: fn(c_int, libc::sighandler_t) -> libc::sighandler_t,
    pub(super) isatty: fn(c_int) -> c_int,
    pub(super) tcgetpgrp: fn(c_int) -> Pid,
    pub(super) tcsetpgrp: fn(c_int, Pid) -> c_int,
    pub(super) setpgid: fn(Pid, Pid) -> c_int,
    pub(super) pipe: fn(&mut [c_int; 2]) -> c_int,
    pub(super) dup2: fn(c_int, c_int) -> c_int,
    pub(super) close: fn(c_int) -> c_int,
    pub(super) fcntl: fn(c_int, c_int, c_int) -> c_int,
    pub(super) read: fn(c_int, &mut [u8]) -> isize,
    pub(super) umask: fn(FileModeMask) -> FileModeMask,
    pub(super) times: fn(*mut libc::tms) -> ClockTicks,
    pub(super) sysconf: fn(c_int) -> c_long,
    pub(super) execvp: fn(*const c_char, *const *const c_char) -> c_int,
    pub(super) execve: fn(*const c_char, *const *const c_char, *const *const c_char) -> c_int,
    // Filesystem
    pub(super) open: fn(*const c_char, c_int, mode_t) -> c_int,
    pub(super) write: fn(c_int, &[u8]) -> isize,
    pub(super) stat: fn(*const c_char, *mut libc::stat) -> c_int,
    pub(super) lstat: fn(*const c_char, *mut libc::stat) -> c_int,
    pub(super) fstat: fn(c_int, *mut libc::stat) -> c_int,
    pub(super) access: fn(*const c_char, c_int) -> c_int,
    pub(super) chdir: fn(*const c_char) -> c_int,
    pub(super) getcwd: fn(*mut c_char, usize) -> *mut c_char,
    pub(super) opendir: fn(*const c_char) -> *mut libc::DIR,
    pub(super) readdir: fn(*mut libc::DIR) -> *mut libc::dirent,
    pub(super) closedir: fn(*mut libc::DIR) -> c_int,
    pub(super) realpath: fn(*const c_char, *mut c_char) -> *mut c_char,
    pub(super) unlink: fn(*const c_char) -> c_int,
    // Process
    pub(super) fork: fn() -> Pid,
    pub(super) exit_process: fn(c_int),
    // Environment
    pub(super) setenv: fn(&[u8], &[u8]) -> SysResult<()>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) unsetenv: fn(&[u8]) -> SysResult<()>,
    pub(super) getenv: fn(&[u8]) -> Option<Vec<u8>>,
    pub(super) get_environ: fn() -> HashMap<Vec<u8>, Vec<u8>>,
    // Terminal attributes
    pub(super) tcgetattr: fn(c_int, *mut libc::termios) -> c_int,
    pub(super) tcsetattr: fn(c_int, c_int, *const libc::termios) -> c_int,
    // User database
    pub(super) getpwnam: fn(&[u8]) -> Option<Vec<u8>>,
    // Signal state
    pub(super) pending_signal_bits: fn() -> usize,
    pub(super) take_pending_signal_bits: fn() -> usize,
    pub(super) monotonic_clock_ns: fn() -> u64,
    // Locale
    pub(super) setup_locale: fn(),
    pub(super) reinit_locale: fn(),
    #[allow(dead_code)]
    pub(super) classify_byte: fn(&[u8], u8) -> bool,
    pub(super) classify_char: fn(&[u8], u32) -> bool,
    pub(super) decode_char: fn(&[u8]) -> (u32, usize),
    pub(super) encode_char: fn(u32, &mut [u8]) -> usize,
    #[allow(dead_code)]
    pub(super) mb_cur_max: fn() -> usize,
    pub(super) to_upper: fn(u32) -> u32,
    pub(super) to_lower: fn(u32) -> u32,
    pub(super) char_width: fn(u32) -> usize,
    pub(super) strcoll: fn(&[u8], &[u8]) -> std::cmp::Ordering,
    pub(super) decimal_point: fn() -> u8,
}

fn classify_wchar_wctype(class: &[u8], wc: u32) -> bool {
    unsafe extern "C" {
        fn wctype(name: *const c_char) -> usize;
        fn iswctype(wc: u32, desc: usize) -> c_int;
    }
    let c_class = crate::bstr::to_cstring(class).unwrap_or_default();
    let desc = unsafe { wctype(c_class.as_ptr()) };
    if desc == 0 {
        false
    } else {
        unsafe { iswctype(wc, desc) != 0 }
    }
}

fn classify_wchar(class: &[u8], wc: u32) -> bool {
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
            _ => classify_wchar_wctype(class, wc),
        }
    }
}

fn mb_cur_max_impl() -> usize {
    #[cfg(target_os = "linux")]
    {
        unsafe extern "C" {
            fn __ctype_get_mb_cur_max() -> usize;
        }
        unsafe { __ctype_get_mb_cur_max() }
    }
    #[cfg(target_os = "macos")]
    {
        unsafe extern "C" {
            static __mb_cur_max: c_int;
        }
        unsafe { __mb_cur_max as usize }
    }
    #[cfg(target_os = "freebsd")]
    {
        unsafe extern "C" {
            fn __mb_cur_max() -> usize;
        }
        unsafe { __mb_cur_max() }
    }
}

pub(super) fn default_interface() -> SystemInterface {
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
            if key.is_empty() || key.contains(&b'=') {
                return Err(SysError::Errno(libc::EINVAL));
            }
            let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
            let c_value = crate::bstr::to_cstring(value).map_err(|_| SysError::NulInPath)?;
            let rc = unsafe { libc::setenv(c_key.as_ptr(), c_value.as_ptr(), 1) };
            if rc == 0 { Ok(()) } else { Err(last_error()) }
        },
        unsetenv: |key| {
            if key.is_empty() || key.contains(&b'=') {
                return Err(SysError::Errno(libc::EINVAL));
            }
            let c_key = crate::bstr::to_cstring(key).map_err(|_| SysError::NulInPath)?;
            let rc = unsafe { libc::unsetenv(c_key.as_ptr()) };
            if rc == 0 { Ok(()) } else { Err(last_error()) }
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
        reinit_locale: || unsafe {
            libc::setlocale(libc::LC_ALL, b"\0".as_ptr().cast());
        },
        classify_byte: |class, byte| classify_wchar(class, byte as u32),
        classify_char: classify_wchar,
        decode_char: |bytes| {
            if bytes.is_empty() {
                return (0, 0);
            }
            #[repr(C, align(8))]
            struct MbState([u8; 128]);
            unsafe extern "C" {
                fn mbrtowc(
                    pwc: *mut libc::wchar_t,
                    s: *const u8,
                    n: usize,
                    ps: *mut MbState,
                ) -> usize;
            }
            unsafe {
                let mut wc: libc::wchar_t = 0;
                let mut ps: MbState = std::mem::zeroed();
                let n = mbrtowc(&mut wc, bytes.as_ptr(), bytes.len(), &mut ps);
                if n == 0 {
                    (0, 0)
                } else if n == usize::MAX || n == usize::MAX - 1 {
                    (bytes[0] as u32, 1)
                } else {
                    (wc as u32, n)
                }
            }
        },
        encode_char: |wc, buf| {
            #[repr(C, align(8))]
            struct MbState([u8; 128]);
            unsafe extern "C" {
                fn wcrtomb(s: *mut u8, wc: libc::wchar_t, ps: *mut MbState) -> usize;
            }
            unsafe {
                let mut ps: MbState = std::mem::zeroed();
                let n = wcrtomb(buf.as_mut_ptr(), wc as libc::wchar_t, &mut ps);
                if n == usize::MAX { 0 } else { n }
            }
        },
        mb_cur_max: mb_cur_max_impl,
        to_upper: |wc| unsafe {
            unsafe extern "C" {
                fn towupper(wc: u32) -> u32;
            }
            towupper(wc)
        },
        to_lower: |wc| unsafe {
            unsafe extern "C" {
                fn towlower(wc: u32) -> u32;
            }
            towlower(wc)
        },
        char_width: |wc| {
            unsafe extern "C" {
                fn wcwidth(wc: u32) -> c_int;
            }
            let w = unsafe { wcwidth(wc) };
            if w < 0 { 0 } else { w as usize }
        },
        strcoll: |a, b| {
            let ca = crate::bstr::to_cstring(a).unwrap_or_default();
            let cb = crate::bstr::to_cstring(b).unwrap_or_default();
            let r = unsafe { libc::strcoll(ca.as_ptr(), cb.as_ptr()) };
            r.cmp(&0)
        },
        decimal_point: || {
            let dp = unsafe { *(*libc::localeconv()).decimal_point };
            if dp == 0 { b'.' } else { dp as u8 }
        },
    }
}
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
        PENDING_SIGNALS.fetch_or(mask, Ordering::SeqCst);
    }
}

pub(super) fn sys_interface() -> SystemInterface {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support;
    use crate::trace_entries;

    use crate::sys::constants::STDIN_FILENO;
    use crate::sys::locale::{classify_byte, setup_locale};
    use crate::sys::process::{current_pid, parent_pid};
    use crate::sys::time::monotonic_clock_ns;

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
    }

    #[test]
    fn default_interface_env_validation() {
        let tbl = default_interface();
        assert!((tbl.setenv)(b"", b"val").is_err());
        assert!((tbl.setenv)(b"k=v", b"val").is_err());
        assert!((tbl.unsetenv)(b"").is_err());
        assert!((tbl.unsetenv)(b"k=v").is_err());
    }

    #[test]
    fn default_interface_lstat_and_unlink_error_on_null() {
        let tbl = default_interface();
        let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
        assert!((tbl.lstat)(std::ptr::null(), buf.as_mut_ptr()) < 0);
        assert!((tbl.unlink)(std::ptr::null()) < 0);
    }

    #[test]
    fn default_interface_decode_char() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let (wc, len) = (tbl.decode_char)(b"A");
        assert_eq!(wc, b'A' as u32);
        assert_eq!(len, 1);
        let (_, len0) = (tbl.decode_char)(b"");
        assert_eq!(len0, 0);
        let (wc_inv, len_inv) = (tbl.decode_char)(&[0xFF, 0xFF]);
        assert_eq!(len_inv, 1);
        assert_eq!(wc_inv, 0xFF);
        let (wc_nul, len_nul) = (tbl.decode_char)(&[0x00]);
        assert_eq!(wc_nul, 0);
        assert_eq!(len_nul, 0);
    }

    #[test]
    fn default_interface_encode_char() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let mut buf = [0u8; 8];
        let n = (tbl.encode_char)(b'Z' as u32, &mut buf);
        assert!(n > 0);
        assert_eq!(buf[0], b'Z');
    }

    #[test]
    fn default_interface_to_upper_lower() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let upper = (tbl.to_upper)(b'a' as u32);
        assert_eq!(upper, b'A' as u32);
        let lower = (tbl.to_lower)(b'A' as u32);
        assert_eq!(lower, b'a' as u32);
    }

    #[test]
    fn default_interface_char_width() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let w = (tbl.char_width)(b'A' as u32);
        assert_eq!(w, 1);
        let w_ctrl = (tbl.char_width)(0x01);
        assert_eq!(w_ctrl, 0);
    }

    #[test]
    fn default_interface_mb_cur_max() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let m = (tbl.mb_cur_max)();
        assert!(m >= 1);
    }

    #[test]
    fn default_interface_decimal_point() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        let dp = (tbl.decimal_point)();
        assert!(dp == b'.' || dp == b',');
    }

    #[test]
    fn default_interface_strcoll() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        use std::cmp::Ordering;
        assert_eq!((tbl.strcoll)(b"abc", b"abc"), Ordering::Equal);
        assert_eq!((tbl.strcoll)(b"abc", b"abd"), Ordering::Less);
        assert_eq!((tbl.strcoll)(b"abd", b"abc"), Ordering::Greater);
    }

    #[test]
    fn classify_wchar_wctype_standard_class() {
        let tbl = default_interface();
        (tbl.setup_locale)();
        assert!(classify_wchar_wctype(b"alpha", b'a' as u32));
        assert!(!classify_wchar_wctype(b"alpha", b'1' as u32));
        assert!(!classify_wchar_wctype(b"bogus", b'a' as u32));
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
