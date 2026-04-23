use libc::{c_int, mode_t};

pub(crate) const SC_CLK_TCK: c_int = libc::_SC_CLK_TCK;
pub(crate) const F_GETFL: c_int = libc::F_GETFL;
pub(crate) const F_SETFL: c_int = libc::F_SETFL;
pub(crate) const F_SETFD: c_int = libc::F_SETFD;
pub(crate) const F_DUPFD_CLOEXEC: c_int = libc::F_DUPFD_CLOEXEC;
pub(crate) const O_NONBLOCK: c_int = libc::O_NONBLOCK;

pub(crate) const SEEK_CUR: c_int = libc::SEEK_CUR;
#[cfg(test)]
pub(crate) const ESPIPE: c_int = libc::ESPIPE;

pub(crate) const STDIN_FILENO: c_int = libc::STDIN_FILENO;
pub(crate) const STDOUT_FILENO: c_int = libc::STDOUT_FILENO;
pub(crate) const STDERR_FILENO: c_int = libc::STDERR_FILENO;
pub(crate) const SIGHUP: c_int = libc::SIGHUP;
pub(crate) const SIGINT: c_int = libc::SIGINT;
pub(crate) const SIGQUIT: c_int = libc::SIGQUIT;
pub(crate) const SIGILL: c_int = libc::SIGILL;
pub(crate) const SIGABRT: c_int = libc::SIGABRT;
pub(crate) const SIGFPE: c_int = libc::SIGFPE;
pub(crate) const SIGKILL: c_int = libc::SIGKILL;
pub(crate) const SIGUSR1: c_int = libc::SIGUSR1;
pub(crate) const SIGSEGV: c_int = libc::SIGSEGV;
pub(crate) const SIGUSR2: c_int = libc::SIGUSR2;
pub(crate) const SIGPIPE: c_int = libc::SIGPIPE;
pub(crate) const SIGALRM: c_int = libc::SIGALRM;
pub(crate) const SIGSTOP: c_int = libc::SIGSTOP;
pub(crate) const SIGCONT: c_int = libc::SIGCONT;
pub(crate) const SIGTERM: c_int = libc::SIGTERM;
pub(crate) const SIGTRAP: c_int = libc::SIGTRAP;
pub(crate) const SIGCHLD: c_int = libc::SIGCHLD;
pub(crate) const SIGTSTP: c_int = libc::SIGTSTP;
pub(crate) const SIGTTIN: c_int = libc::SIGTTIN;
pub(crate) const SIGTTOU: c_int = libc::SIGTTOU;
pub(crate) const SIGBUS: c_int = libc::SIGBUS;
pub(crate) const SIGSYS: c_int = libc::SIGSYS;
pub(crate) const WNOHANG: c_int = libc::WNOHANG;
pub(crate) const WUNTRACED: c_int = libc::WUNTRACED;
pub(crate) const WCONTINUED: c_int = libc::WCONTINUED;
pub(crate) const EEXIST: c_int = libc::EEXIST;
pub(crate) const EINTR: c_int = libc::EINTR;
#[cfg(test)]
pub(crate) const ENOENT: c_int = libc::ENOENT;
#[cfg(test)]
pub(crate) const ENOEXEC: c_int = libc::ENOEXEC;
#[cfg(test)]
pub(crate) const EBADF: c_int = libc::EBADF;
#[cfg(test)]
pub(crate) const ECHILD: c_int = libc::ECHILD;
#[cfg(test)]
pub(crate) const EACCES: c_int = libc::EACCES;
#[cfg(test)]
pub(crate) const EINVAL: c_int = libc::EINVAL;
#[cfg(test)]
pub(crate) const EIO: c_int = libc::EIO;
#[cfg(test)]
pub(crate) const EISDIR: c_int = libc::EISDIR;

pub(crate) const SIG_DFL_HANDLER: libc::sighandler_t = libc::SIG_DFL;
pub(crate) const SIG_IGN_HANDLER: libc::sighandler_t = libc::SIG_IGN;
pub(crate) const SIG_ERR_HANDLER: libc::sighandler_t = libc::SIG_ERR;

/// `tcsetattr` action: apply changes immediately without waiting for
/// the output queue to drain.
///
/// The shell uses `TCSANOW` (rather than `TCSADRAIN`) when toggling the
/// terminal in and out of raw mode because the shell itself is the
/// writer. With `TCSADRAIN`, a mode restore on a PTY with unread
/// output queued blocks in the kernel (`tty_drain`) until the reader
/// consumes the bytes — a deadlock when the reader is slow, absent, or
/// dead (most notably, in integration tests that stop reading before
/// sending `exit`). `readline`/`bash` use the same `TCSANOW` discipline
/// for exactly this reason.
pub(crate) const TCSANOW: c_int = libc::TCSANOW;

pub(crate) const O_RDONLY: c_int = libc::O_RDONLY;
pub(crate) const O_WRONLY: c_int = libc::O_WRONLY;
pub(crate) const O_RDWR: c_int = libc::O_RDWR;
pub(crate) const O_CREAT: c_int = libc::O_CREAT;
pub(crate) const O_TRUNC: c_int = libc::O_TRUNC;
pub(crate) const O_APPEND: c_int = libc::O_APPEND;
pub(crate) const O_EXCL: c_int = libc::O_EXCL;
pub(crate) const O_CLOEXEC: c_int = libc::O_CLOEXEC;

pub(crate) const F_OK: c_int = libc::F_OK;
pub(crate) const R_OK: c_int = libc::R_OK;
pub(crate) const W_OK: c_int = libc::W_OK;
pub(crate) const X_OK: c_int = libc::X_OK;

#[cfg(test)]
pub(crate) const EMFILE: c_int = libc::EMFILE;
#[cfg(test)]
pub(crate) const ENOMEM: c_int = libc::ENOMEM;
#[cfg(test)]
pub(crate) const ENOTTY: c_int = libc::ENOTTY;
#[cfg(test)]
pub(crate) const ESRCH: c_int = libc::ESRCH;

pub(crate) const ICANON: libc::tcflag_t = libc::ICANON;
pub(crate) const ECHO: libc::tcflag_t = libc::ECHO;
pub(crate) const ISIG: libc::tcflag_t = libc::ISIG;
/// Input extended functions (BSD/POSIX): when set, the tty driver
/// treats `VLNEXT` (default `^V`, octal 026) as "literal next", eating
/// it and passing the following byte through raw. Interactive line
/// editors must clear `IEXTEN` so they can implement their own
/// `quoted-insert` binding on `^V`/`^Q`.
pub(crate) const IEXTEN: libc::tcflag_t = libc::IEXTEN;
/// Input flag: XON/XOFF flow control on output. When set, the tty
/// driver consumes `VSTART` (default `^Q`, 0x11) and `VSTOP`
/// (default `^S`, 0x13) instead of delivering them to the process.
/// Interactive editors must clear it so `^Q` reaches `quoted-insert`
/// and `^S` reaches `forward-search-history`.
pub(crate) const IXON: libc::tcflag_t = libc::IXON;
pub(crate) const VMIN: usize = libc::VMIN;
pub(crate) const VTIME: usize = libc::VTIME;
pub(crate) const VERASE: usize = libc::VERASE;

pub(crate) const S_IFMT: mode_t = libc::S_IFMT;
pub(crate) const S_IFDIR: mode_t = libc::S_IFDIR;
pub(crate) const S_IFREG: mode_t = libc::S_IFREG;
pub(crate) const S_IFIFO: mode_t = libc::S_IFIFO;
pub(crate) const S_IFBLK: mode_t = libc::S_IFBLK;
pub(crate) const S_IFCHR: mode_t = libc::S_IFCHR;
pub(crate) const S_IFLNK: mode_t = libc::S_IFLNK;
pub(crate) const S_IFSOCK: mode_t = libc::S_IFSOCK;
pub(crate) const S_IXUSR: mode_t = libc::S_IXUSR;
pub(crate) const S_IXGRP: mode_t = libc::S_IXGRP;
pub(crate) const S_IXOTH: mode_t = libc::S_IXOTH;

pub(crate) const RLIMIT_CORE: i32 = libc::RLIMIT_CORE as i32;
pub(crate) const RLIMIT_DATA: i32 = libc::RLIMIT_DATA as i32;
pub(crate) const RLIMIT_FSIZE: i32 = libc::RLIMIT_FSIZE as i32;
pub(crate) const RLIMIT_NOFILE: i32 = libc::RLIMIT_NOFILE as i32;
pub(crate) const RLIMIT_STACK: i32 = libc::RLIMIT_STACK as i32;
pub(crate) const RLIMIT_CPU: i32 = libc::RLIMIT_CPU as i32;
pub(crate) const RLIMIT_AS: i32 = libc::RLIMIT_AS as i32;
pub(crate) const RLIM_INFINITY: u64 = libc::RLIM_INFINITY as u64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::process::signal_name;

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
