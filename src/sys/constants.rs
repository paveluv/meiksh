use libc::{c_int, mode_t};

pub(crate) const SC_CLK_TCK: c_int = libc::_SC_CLK_TCK;
pub(crate) const F_GETFL: c_int = libc::F_GETFL;
pub(crate) const F_SETFL: c_int = libc::F_SETFL;
pub(crate) const F_DUPFD_CLOEXEC: c_int = libc::F_DUPFD_CLOEXEC;
pub(crate) const O_NONBLOCK: c_int = libc::O_NONBLOCK;

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

pub(crate) const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
pub(crate) const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
pub(crate) const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

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
pub const S_IFBLK: mode_t = libc::S_IFBLK;
pub const S_IFCHR: mode_t = libc::S_IFCHR;
pub const S_IFLNK: mode_t = libc::S_IFLNK;
pub const S_IFSOCK: mode_t = libc::S_IFSOCK;
pub const S_IXUSR: mode_t = libc::S_IXUSR;
pub const S_IXGRP: mode_t = libc::S_IXGRP;
pub const S_IXOTH: mode_t = libc::S_IXOTH;

pub const RLIMIT_CORE: i32 = libc::RLIMIT_CORE as i32;
pub const RLIMIT_DATA: i32 = libc::RLIMIT_DATA as i32;
pub const RLIMIT_FSIZE: i32 = libc::RLIMIT_FSIZE as i32;
pub const RLIMIT_NOFILE: i32 = libc::RLIMIT_NOFILE as i32;
pub const RLIMIT_STACK: i32 = libc::RLIMIT_STACK as i32;
pub const RLIMIT_CPU: i32 = libc::RLIMIT_CPU as i32;
pub const RLIMIT_AS: i32 = libc::RLIMIT_AS as i32;
pub const RLIM_INFINITY: u64 = libc::RLIM_INFINITY;

#[cfg(test)]
mod tests {
    use libc::{c_char, c_int, c_long, mode_t};
    use std::collections::HashMap;
    use std::ffi::CString;

    use crate::sys::test_support;
    use crate::sys::types::ClockTicks;

    use super::*;
    use crate::sys::*;

    #[test]
    fn signal_name_covers_all_branches() {
        assert_eq!(signal_name(SIGHUP), b"SIGHUP");
        assert_eq!(signal_name(SIGINT), b"SIGINT");
        assert_eq!(signal_name(SIGQUIT), b"SIGQUIT");
        assert_eq!(signal_name(SIGILL), b"SIGILL");
        assert_eq!(signal_name(SIGABRT), b"SIGABRT");
        assert_eq!(signal_name(SIGFPE), b"SIGFPE");
        assert_eq!(signal_name(SIGKILL), b"SIGKILL");
        assert_eq!(signal_name(SIGBUS), b"SIGBUS");
        assert_eq!(signal_name(SIGUSR1), b"SIGUSR1");
        assert_eq!(signal_name(SIGSEGV), b"SIGSEGV");
        assert_eq!(signal_name(SIGUSR2), b"SIGUSR2");
        assert_eq!(signal_name(SIGPIPE), b"SIGPIPE");
        assert_eq!(signal_name(SIGALRM), b"SIGALRM");
        assert_eq!(signal_name(SIGTERM), b"SIGTERM");
        assert_eq!(signal_name(SIGCHLD), b"SIGCHLD");
        assert_eq!(signal_name(SIGSTOP), b"SIGSTOP");
        assert_eq!(signal_name(SIGCONT), b"SIGCONT");
        assert_eq!(signal_name(SIGTRAP), b"SIGTRAP");
        assert_eq!(signal_name(SIGTSTP), b"SIGTSTP");
        assert_eq!(signal_name(SIGTTIN), b"SIGTTIN");
        assert_eq!(signal_name(SIGTTOU), b"SIGTTOU");
        assert_eq!(signal_name(SIGSYS), b"SIGSYS");
        assert_eq!(signal_name(999), b"UNKNOWN");
    }
}
