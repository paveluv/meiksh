use libc::c_int;
use std::ffi::CStr;

use super::constants::EINTR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SysError {
    Errno(c_int),
    NulInPath,
}

pub(crate) type SysResult<T> = Result<T, SysError>;

impl SysError {
    pub(crate) fn errno(&self) -> Option<c_int> {
        match self {
            SysError::Errno(e) => Some(*e),
            _ => None,
        }
    }

    pub(crate) fn strerror(&self) -> Vec<u8> {
        match self {
            SysError::Errno(errno) => {
                let msg = unsafe { CStr::from_ptr(libc::strerror(*errno)) };
                crate::bstr::bytes_from_cstr(msg)
            }
            SysError::NulInPath => b"path contains null byte".to_vec(),
        }
    }

    pub(crate) fn is_enoent(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::ENOENT)
    }

    pub(crate) fn is_ebadf(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::EBADF)
    }

    pub(crate) fn is_eacces(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::EACCES)
    }

    pub(crate) fn is_enoexec(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == libc::ENOEXEC)
    }

    pub(crate) fn is_eintr(&self) -> bool {
        matches!(self, SysError::Errno(e) if *e == EINTR)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sys_error_helper_methods_report_correct_variants() {
        let errno_err = SysError::Errno(libc::ENOENT);
        assert_eq!(errno_err.errno(), Some(libc::ENOENT));
        assert!(errno_err.is_enoent());
        assert!(!errno_err.is_ebadf());
        assert!(!errno_err.is_enoexec());
        assert!(!errno_err.is_eintr());
        assert!(!errno_err.strerror().is_empty());
        assert!(!errno_err.strerror().is_empty());

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
}
