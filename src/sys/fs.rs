use libc::{c_int, mode_t};
use std::ffi::{CStr, CString};

use super::constants::{F_OK, O_CLOEXEC, O_CREAT, O_EXCL, O_RDONLY, O_TRUNC};
use super::error::{SysError, SysResult};
use super::fd_io::{close_fd, read_fd};
use super::interface::{last_error, set_errno, sys_interface};
use super::types::FileStat;

fn to_cstring(path: &[u8]) -> SysResult<CString> {
    crate::bstr::to_cstring(path).map_err(|_| SysError::NulInPath)
}

fn stat_raw(path: &[u8]) -> SysResult<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (sys_interface().stat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(last_error())
    }
}

pub fn open_file(path: &[u8], flags: c_int, mode: mode_t) -> SysResult<c_int> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().open)(c_path.as_ptr(), flags, mode);
    if result >= 0 {
        Ok(result)
    } else {
        Err(last_error())
    }
}

pub fn stat_path(path: &[u8]) -> SysResult<FileStat> {
    let raw = stat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
        dev: raw.st_dev,
        ino: raw.st_ino,
        mtime_sec: raw.st_mtime,
        mtime_nsec: raw.st_mtime_nsec,
    })
}

fn lstat_raw(path: &[u8]) -> SysResult<libc::stat> {
    let c_path = to_cstring(path)?;
    let mut buf = std::mem::MaybeUninit::<libc::stat>::zeroed();
    let result = (sys_interface().lstat)(c_path.as_ptr(), buf.as_mut_ptr());
    if result == 0 {
        Ok(unsafe { buf.assume_init() })
    } else {
        Err(last_error())
    }
}

pub fn lstat_path(path: &[u8]) -> SysResult<FileStat> {
    let raw = lstat_raw(path)?;
    Ok(FileStat {
        mode: raw.st_mode,
        size: raw.st_size as u64,
        dev: raw.st_dev,
        ino: raw.st_ino,
        mtime_sec: raw.st_mtime,
        mtime_nsec: raw.st_mtime_nsec,
    })
}

pub fn access_path(path: &[u8], mode: c_int) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().access)(c_path.as_ptr(), mode);
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn file_exists(path: &[u8]) -> bool {
    access_path(path, F_OK).is_ok()
}

pub fn is_directory(path: &[u8]) -> bool {
    stat_path(path).map(|s| s.is_dir()).unwrap_or(false)
}

pub fn is_regular_file(path: &[u8]) -> bool {
    stat_path(path)
        .map(|s| s.is_regular_file())
        .unwrap_or(false)
}

pub fn change_dir(path: &[u8]) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().chdir)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn get_cwd() -> SysResult<Vec<u8>> {
    let mut buf = vec![0u8; 4096];
    let result = (sys_interface().getcwd)(buf.as_mut_ptr().cast(), buf.len());
    if result.is_null() {
        Err(last_error())
    } else {
        let cstr = unsafe { CStr::from_ptr(result) };
        Ok(crate::bstr::bytes_from_cstr(cstr))
    }
}

pub fn read_dir_entries(path: &[u8]) -> SysResult<Vec<Vec<u8>>> {
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
        let name = crate::bstr::bytes_from_cstr(name);
        if name != b"." && name != b".." {
            entries.push(name);
        }
    }
    Ok(entries)
}

pub fn canonicalize(path: &[u8]) -> SysResult<Vec<u8>> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().realpath)(c_path.as_ptr(), std::ptr::null_mut());
    if result.is_null() {
        Err(last_error())
    } else {
        let s = crate::bstr::bytes_from_cstr(unsafe { CStr::from_ptr(result) });
        unsafe { libc::free(result.cast()) };
        Ok(s)
    }
}

pub fn read_file_bytes(path: &[u8]) -> SysResult<Vec<u8>> {
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

pub fn read_file(path: &[u8]) -> SysResult<Vec<u8>> {
    read_file_bytes(path)
}

pub fn unlink(path: &[u8]) -> SysResult<()> {
    let c_path = to_cstring(path)?;
    let result = (sys_interface().unlink)(c_path.as_ptr());
    if result == 0 {
        Ok(())
    } else {
        Err(last_error())
    }
}

pub fn open_for_redirect(
    path: &[u8],
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

#[cfg(test)]
mod tests {
    use libc::{c_char, c_int, mode_t};

    use crate::sys::test_support;
    use crate::trace_entries;

    use super::*;
    use crate::sys::*;

    #[test]
    fn read_dir_entries_readdir_error() {
        test_support::run_trace(
            trace_entries![
                opendir(_) -> 1,
                readdir(_) -> err(libc::EIO),
                closedir(_) -> 0,
            ],
            || {
                assert!(read_dir_entries(b"/tmp").is_err());
            },
        );
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
            assert!(change_dir(b"/nonexistent").is_err());
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
            assert!(canonicalize(b"/nonexistent").is_err());
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
            let fd = open_for_redirect(b"/tmp/out", O_WRONLY | O_TRUNC | O_CREAT, 0o666, true)
                .expect("open");
            assert_eq!(fd, 5);
            let flags = CAPTURED_FLAGS.load(Ordering::SeqCst);
            assert!(flags & O_TRUNC == 0);
            assert!(flags & O_EXCL != 0);
            assert!(flags & O_CREAT != 0);

            let fd = open_for_redirect(b"/tmp/out", O_WRONLY | O_TRUNC | O_CREAT, 0o666, false)
                .expect("open");
            assert_eq!(fd, 5);
            let flags = CAPTURED_FLAGS.load(Ordering::SeqCst);
            assert!(flags & O_TRUNC != 0);
        });
    }

    #[test]
    fn trace_stat_fifo_and_fstat_dir_arms() {
        test_support::run_trace(
            trace_entries![
                stat(_, _) -> stat_fifo,
                stat(_, _) -> 0,
            ],
            || {
                let s = stat_path(b"/fifo").expect("stat fifo");
                assert!((s.mode & libc::S_IFMT) == libc::S_IFIFO);
                let _ = stat_path(b"/plain");
            },
        );
    }

    #[test]
    fn lstat_path_success() {
        test_support::run_trace(
            trace_entries![
                ..vec![test_support::t(
                    "lstat",
                    vec![
                        test_support::ArgMatcher::Str(b"file.txt".to_vec()),
                        test_support::ArgMatcher::Any
                    ],
                    test_support::TraceResult::StatSymlink,
                )]
            ],
            || {
                let stat = lstat_path(b"file.txt").unwrap();
                assert_eq!(stat.mode & libc::S_IFMT, libc::S_IFLNK);
            },
        );
    }

    #[test]
    fn lstat_path_failure() {
        test_support::run_trace(
            trace_entries![
                ..vec![test_support::t(
                    "lstat",
                    vec![
                        test_support::ArgMatcher::Str(b"file.txt".to_vec()),
                        test_support::ArgMatcher::Any
                    ],
                    test_support::TraceResult::Err(libc::ENOENT),
                )]
            ],
            || {
                let err = lstat_path(b"file.txt").unwrap_err();
                assert!(matches!(err, SysError::Errno(libc::ENOENT)));
            },
        );
    }

    #[test]
    fn unlink_success() {
        test_support::run_trace(
            trace_entries![
                ..vec![test_support::t(
                    "unlink",
                    vec![test_support::ArgMatcher::Str(b"file.txt".to_vec())],
                    test_support::TraceResult::Int(0),
                )]
            ],
            || {
                let result = unlink(b"file.txt");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    fn unlink_failure() {
        test_support::run_trace(
            trace_entries![
                ..vec![test_support::t(
                    "unlink",
                    vec![test_support::ArgMatcher::Str(b"file.txt".to_vec())],
                    test_support::TraceResult::Err(libc::ENOENT),
                )]
            ],
            || {
                let err = unlink(b"file.txt").unwrap_err();
                assert!(matches!(err, SysError::Errno(libc::ENOENT)));
            },
        );
    }

    #[test]
    fn trace_opendir_int_and_readdir_fallback() {
        test_support::run_trace(
            trace_entries![
                opendir(_) -> 0,
            ],
            || {
                assert!(read_dir_entries(b"/tmp").is_err());
            },
        );
    }
}
