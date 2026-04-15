use libc::{c_int, mode_t};

use super::constants::{
    S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFMT, S_IFREG, S_IFSOCK, S_IXGRP, S_IXOTH,
    S_IXUSR,
};

pub(crate) type Pid = libc::pid_t;
pub(crate) type RawFd = c_int;
pub(crate) type FileModeMask = libc::mode_t;
pub(crate) type ClockTicks = libc::clock_t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WaitStatus {
    pub(crate) pid: Pid,
    pub(crate) status: c_int,
}

pub(crate) struct FdReader {
    pub(crate) fd: c_int,
}

impl FdReader {
    pub(crate) fn new(fd: c_int) -> Self {
        Self { fd }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FileStat {
    pub(crate) mode: mode_t,
    pub(crate) size: u64,
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) mtime_sec: i64,
    pub(crate) mtime_nsec: i64,
}

impl FileStat {
    pub(crate) fn is_dir(&self) -> bool {
        (self.mode & S_IFMT) == S_IFDIR
    }

    pub(crate) fn is_regular_file(&self) -> bool {
        (self.mode & S_IFMT) == S_IFREG
    }

    pub(crate) fn is_executable(&self) -> bool {
        self.mode & (S_IXUSR | S_IXGRP | S_IXOTH) != 0
    }

    pub(crate) fn is_block_special(&self) -> bool {
        (self.mode & S_IFMT) == S_IFBLK
    }

    pub(crate) fn is_char_special(&self) -> bool {
        (self.mode & S_IFMT) == S_IFCHR
    }

    pub(crate) fn is_fifo(&self) -> bool {
        (self.mode & S_IFMT) == S_IFIFO
    }

    pub(crate) fn is_symlink(&self) -> bool {
        (self.mode & S_IFMT) == S_IFLNK
    }

    pub(crate) fn is_socket(&self) -> bool {
        (self.mode & S_IFMT) == S_IFSOCK
    }

    pub(crate) fn is_setuid(&self) -> bool {
        self.mode & libc::S_ISUID != 0
    }

    pub(crate) fn is_setgid(&self) -> bool {
        self.mode & libc::S_ISGID != 0
    }

    pub(crate) fn same_file(&self, other: &FileStat) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }

    pub(crate) fn newer_than(&self, other: &FileStat) -> bool {
        (self.mtime_sec, self.mtime_nsec) > (other.mtime_sec, other.mtime_nsec)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ChildHandle {
    pub(crate) pid: Pid,
    pub(crate) stdout_fd: Option<c_int>,
}

pub(crate) struct ChildOutput {
    pub(crate) status: ChildExitStatus,
    pub(crate) stdout: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChildExitStatus {
    pub(crate) code: i32,
}

impl ChildExitStatus {
    pub(crate) fn success(&self) -> bool {
        self.code == 0
    }

    pub(crate) fn code(&self) -> Option<i32> {
        Some(self.code)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessTimes {
    pub(crate) user_ticks: u64,
    pub(crate) system_ticks: u64,
    pub(crate) child_user_ticks: u64,
    pub(crate) child_system_ticks: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_exit_status_code() {
        let status = ChildExitStatus { code: 42 };
        assert_eq!(status.code(), Some(42));
        assert!(!status.success());
        let zero = ChildExitStatus { code: 0 };
        assert!(zero.success());
    }

    #[test]
    fn file_stat_type_predicates() {
        let base = FileStat {
            mode: 0,
            size: 0,
            dev: 1,
            ino: 100,
            mtime_sec: 0,
            mtime_nsec: 0,
        };

        let blk = FileStat {
            mode: S_IFBLK,
            ..base.clone()
        };
        assert!(blk.is_block_special());
        assert!(!blk.is_char_special());

        let chr = FileStat {
            mode: S_IFCHR,
            ..base.clone()
        };
        assert!(chr.is_char_special());
        assert!(!chr.is_fifo());

        let fifo = FileStat {
            mode: S_IFIFO,
            ..base.clone()
        };
        assert!(fifo.is_fifo());
        assert!(!fifo.is_symlink());

        let lnk = FileStat {
            mode: S_IFLNK,
            ..base.clone()
        };
        assert!(lnk.is_symlink());
        assert!(!lnk.is_socket());

        let sock = FileStat {
            mode: S_IFSOCK,
            ..base.clone()
        };
        assert!(sock.is_socket());
        assert!(!sock.is_dir());
    }

    #[test]
    fn file_stat_setuid_setgid() {
        let base = FileStat {
            mode: 0,
            size: 0,
            dev: 1,
            ino: 100,
            mtime_sec: 0,
            mtime_nsec: 0,
        };

        let suid = FileStat {
            mode: libc::S_ISUID,
            ..base.clone()
        };
        assert!(suid.is_setuid());
        assert!(!suid.is_setgid());

        let sgid = FileStat {
            mode: libc::S_ISGID,
            ..base.clone()
        };
        assert!(sgid.is_setgid());
        assert!(!sgid.is_setuid());

        assert!(!base.is_setuid());
        assert!(!base.is_setgid());
    }

    #[test]
    fn file_stat_same_file_and_newer_than() {
        let a = FileStat {
            mode: S_IFREG,
            size: 100,
            dev: 1,
            ino: 42,
            mtime_sec: 1000,
            mtime_nsec: 0,
        };
        let b = FileStat {
            mode: S_IFREG,
            size: 200,
            dev: 1,
            ino: 42,
            mtime_sec: 900,
            mtime_nsec: 0,
        };
        let c = FileStat {
            mode: S_IFREG,
            size: 100,
            dev: 2,
            ino: 42,
            mtime_sec: 1000,
            mtime_nsec: 500,
        };

        assert!(a.same_file(&b));
        assert!(!a.same_file(&c));
        assert!(a.newer_than(&b));
        assert!(!b.newer_than(&a));
        assert!(c.newer_than(&a));
    }
}
