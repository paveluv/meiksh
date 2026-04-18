use super::error::SysResult;
use super::interface::sys_interface;
use crate::hash::ShellMap;

pub(crate) fn env_set_var(key: &[u8], value: &[u8]) -> SysResult<()> {
    (sys_interface().setenv)(key, value)
}

pub(crate) fn env_unset_var(key: &[u8]) -> SysResult<()> {
    (sys_interface().unsetenv)(key)
}

pub(crate) fn env_var(key: &[u8]) -> Option<Vec<u8>> {
    (sys_interface().getenv)(key)
}

pub(crate) fn env_vars() -> ShellMap<Vec<u8>, Vec<u8>> {
    (sys_interface().get_environ)()
}

pub(crate) fn home_dir_for_user(name: &[u8]) -> Option<Vec<u8>> {
    (sys_interface().getpwnam)(name)
}

#[allow(clippy::disallowed_methods)]
pub(crate) fn env_args_os() -> Vec<Vec<u8>> {
    use std::os::unix::ffi::OsStringExt;
    std::env::args_os().map(|s| s.into_vec()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::interface::sys_interface;
    use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
    use crate::trace_entries;

    #[test]
    fn setenv_success() {
        run_trace(
            trace_entries![setenv(str(b"MY_KEY"), str(b"my_val")) -> 0],
            || {
                let result = env_set_var(b"MY_KEY", b"my_val");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    fn unsetenv_success() {
        run_trace(trace_entries![unsetenv(str(b"MY_KEY")) -> 0], || {
            let result = env_unset_var(b"MY_KEY");
            assert!(result.is_ok());
        });
    }

    #[test]
    fn getenv_found() {
        run_trace(
            trace_entries![
                ..vec![t(
                    "getenv",
                    vec![ArgMatcher::Str(b"HOME".to_vec())],
                    TraceResult::StrVal(b"/home/user".to_vec()),
                )]
            ],
            || {
                let val = (sys_interface().getenv)(b"HOME");
                assert_eq!(val, Some(b"/home/user".to_vec()));
            },
        );
    }

    #[test]
    fn get_environ_returns_map() {
        let mut expected = ShellMap::default();
        expected.insert(b"HOME".to_vec(), b"/home/user".to_vec());
        expected.insert(b"PATH".to_vec(), b"/usr/bin".to_vec());

        run_trace(
            trace_entries![
                ..vec![t(
                    "get_environ",
                    vec![],
                    TraceResult::EnvMap(expected.clone()),
                )]
            ],
            || {
                let map = (sys_interface().get_environ)();
                assert_eq!(map.get(b"HOME".as_ref()), Some(&b"/home/user".to_vec()));
                assert_eq!(map.get(b"PATH".as_ref()), Some(&b"/usr/bin".to_vec()));
            },
        );
    }
}
