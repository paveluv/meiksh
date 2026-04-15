use super::pwd::current_logical_pwd;
use super::{BuiltinOutcome, var_error_msg, write_stdout_line};
use crate::bstr::ByteWriter;
use crate::shell::error::ShellError;
use crate::shell::state::Shell;
use crate::sys;

pub(super) fn cd(shell: &mut Shell, argv: &[Vec<u8>]) -> Result<BuiltinOutcome, ShellError> {
    let (target, print_new_pwd, physical, check_pwd) = parse_cd_target(shell, argv)?;
    let (resolved_target, _, print_new_pwd) = resolve_cd_target(shell, &target, print_new_pwd);
    let curpath = if physical {
        resolved_target.clone()
    } else {
        cd_logical_curpath(shell, &resolved_target)?
    };

    let old_pwd = current_logical_pwd(shell)?;
    sys::fs::change_dir(&curpath).map_err(|e| shell.diagnostic(1, &e.strerror()))?;

    let new_pwd = if physical {
        match sys::fs::get_cwd() {
            Ok(cwd) => cwd,
            Err(_) if check_pwd => {
                shell
                    .set_var(b"OLDPWD", old_pwd)
                    .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
                return Ok(BuiltinOutcome::Status(1));
            }
            Err(_) => curpath.clone(),
        }
    } else {
        curpath.clone()
    };

    shell
        .set_var(b"OLDPWD", old_pwd)
        .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
    shell
        .set_var(b"PWD", new_pwd.clone())
        .map_err(|e| shell.diagnostic(1, &var_error_msg(b"cd", &e)))?;
    if print_new_pwd {
        write_stdout_line(&new_pwd);
    }
    Ok(BuiltinOutcome::Status(0))
}

pub(super) fn cd_logical_curpath(shell: &Shell, target: &[u8]) -> Result<Vec<u8>, ShellError> {
    let curpath = if target.first() == Some(&b'/') {
        target.to_vec()
    } else {
        let pwd = current_logical_pwd(shell)?;
        if pwd.last() == Some(&b'/') {
            let mut r = pwd;
            r.extend_from_slice(target);
            r
        } else {
            let mut r = pwd;
            r.push(b'/');
            r.extend_from_slice(target);
            r
        }
    };
    Ok(canonicalize_logical_path(&curpath))
}

pub(super) fn canonicalize_logical_path(path: &[u8]) -> Vec<u8> {
    let mut components: Vec<&[u8]> = Vec::new();
    for part in path.split(|&b| b == b'/') {
        match part {
            b"" | b"." => {}
            b".." => {
                if !components.is_empty() {
                    components.pop();
                }
            }
            _ => components.push(part),
        }
    }
    if components.is_empty() {
        return b"/".to_vec();
    }
    let mut result = Vec::new();
    for component in &components {
        result.push(b'/');
        result.extend_from_slice(component);
    }
    result
}

pub(super) fn parse_cd_target(
    shell: &Shell,
    argv: &[Vec<u8>],
) -> Result<(Vec<u8>, bool, bool, bool), ShellError> {
    let mut index = 1usize;
    let mut physical = false;
    let mut check_pwd = false;
    while let Some(arg) = argv.get(index) {
        if arg == b"--" {
            index += 1;
            break;
        }
        if arg.first() != Some(&b'-') || arg == b"-" {
            break;
        }
        for &ch in &arg[1..] {
            match ch {
                b'L' => {
                    physical = false;
                    check_pwd = false;
                }
                b'P' => physical = true,
                b'e' => check_pwd = true,
                _ => {
                    let msg = ByteWriter::new()
                        .bytes(b"cd: invalid option: -")
                        .byte(ch)
                        .finish();
                    return Err(shell.diagnostic(1, &msg));
                }
            }
        }
        index += 1;
    }
    if !physical {
        check_pwd = false;
    }
    if argv.len() > index + 1 {
        return Err(shell.diagnostic(1, b"cd: too many arguments"));
    }
    let Some(target) = argv.get(index) else {
        let home = shell
            .get_var(b"HOME")
            .ok_or_else(|| shell.diagnostic(1, b"cd: HOME not set"))?;
        return Ok((home.to_vec(), false, physical, check_pwd));
    };
    if target.is_empty() {
        return Err(shell.diagnostic(1, b"cd: empty directory"));
    }
    if target == b"-" {
        return Ok((
            shell
                .get_var(b"OLDPWD")
                .ok_or_else(|| shell.diagnostic(1, b"cd: OLDPWD not set"))?
                .to_vec(),
            true,
            physical,
            check_pwd,
        ));
    }
    Ok((target.clone(), false, physical, check_pwd))
}

pub(super) fn resolve_cd_target(
    shell: &Shell,
    target: &[u8],
    print_new_pwd: bool,
) -> (Vec<u8>, Vec<u8>, bool) {
    if print_new_pwd || target.first() == Some(&b'/') {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    }
    let first_component = target.split(|&b| b == b'/').next().unwrap_or(b"");
    if first_component == b"." || first_component == b".." {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    }

    let Some(cdpath) = shell.get_var(b"CDPATH") else {
        return (target.to_vec(), target.to_vec(), print_new_pwd);
    };

    for prefix in cdpath.split(|&b| b == b':') {
        let candidate = if prefix.is_empty() {
            let mut c = b"./".to_vec();
            c.extend_from_slice(target);
            c
        } else {
            let mut c = prefix.to_vec();
            c.push(b'/');
            c.extend_from_slice(target);
            c
        };
        if sys::fs::is_directory(&candidate) {
            let should_print = print_new_pwd || !prefix.is_empty();
            let pwd_target = if prefix.is_empty() {
                target.to_vec()
            } else {
                candidate.clone()
            };
            return (candidate, pwd_target, should_print);
        }
    }

    (target.to_vec(), target.to_vec(), print_new_pwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn canonicalize_logical_path_handles_all_cases() {
        assert_no_syscalls(|| {
            assert_eq!(canonicalize_logical_path(b"/usr/.."), b"/");
            assert_eq!(canonicalize_logical_path(b"/a/b/../c"), b"/a/c");
            assert_eq!(canonicalize_logical_path(b"/a/./b"), b"/a/b");
            assert_eq!(canonicalize_logical_path(b"/"), b"/");
            assert_eq!(canonicalize_logical_path(b"/a/b/../../.."), b"/");
            assert_eq!(canonicalize_logical_path(b"/a//b"), b"/a/b");
        });
    }

    #[test]
    fn cd_physical_mode_with_dash_e_get_cwd_fails() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/home"),
                realpath(any, any) -> realpath("/home"),
                realpath(any, any) -> realpath("/home"),
                chdir(str(b"/tmp")) -> 0,
                getcwd() -> err(libc::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"cd".to_vec(), b"-Pe".to_vec(), b"/tmp".to_vec()],
                )
                .expect("cd -Pe");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn cd_physical_mode() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/home"),
                realpath(any, any) -> realpath("/home"),
                realpath(any, any) -> realpath("/home"),
                chdir(str(b"/usr")) -> 0,
                getcwd() -> cwd("/usr"),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"cd".to_vec(), b"-P".to_vec(), b"/usr".to_vec()],
                )
                .expect("cd -P");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var(b"PWD"), Some(b"/usr" as &[u8]));
            },
        );
    }

    #[test]
    fn cd_home_not_set() {
        run_trace(
            trace_entries![write(
                fd(crate::sys::constants::STDERR_FILENO),
                bytes(b"meiksh: cd: HOME not set\n"),
            ) -> auto,],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"cd".to_vec()]);
            },
        );
    }

    #[test]
    fn cd_cdpath_match_found() {
        run_trace(
            trace_entries![
                stat(str(b"/opt/subdir"), any) -> stat_dir,
                getcwd() -> cwd("/home"),
                realpath(any, any) -> realpath("/home"),
                realpath(any, any) -> realpath("/home"),
                chdir(str(b"/opt/subdir")) -> 0,
                write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"/opt/subdir\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home".to_vec());
                shell.env.insert(b"CDPATH".to_vec(), b"/opt".to_vec());
                let outcome =
                    invoke(&mut shell, &[b"cd".to_vec(), b"subdir".to_vec()]).expect("cd cdpath");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn cd_invalid_option() {
        let msg = diag(b"cd: invalid option: -z");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"cd".to_vec(), b"-z".to_vec()]);
            },
        );
    }

    #[test]
    fn cd_too_many_args() {
        let msg = diag(b"cd: too many arguments");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"cd".to_vec(), b"a".to_vec(), b"b".to_vec()]);
            },
        );
    }

    #[test]
    fn cd_empty_dir() {
        let msg = diag(b"cd: empty directory");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let mut shell = test_shell();
                let _ = invoke(&mut shell, &[b"cd".to_vec(), b"".to_vec()]);
            },
        );
    }

    #[test]
    fn cd_dash_dash_handling() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/home"),
                realpath(any, any) -> realpath("/home"),
                realpath(any, any) -> realpath("/home"),
                chdir(str(b"/tmp")) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"cd".to_vec(), b"--".to_vec(), b"/tmp".to_vec()],
                )
                .expect("cd -- /tmp");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
            },
        );
    }

    #[test]
    fn canonicalize_logical_path_removes_dots() {
        assert_no_syscalls(|| {
            assert_eq!(canonicalize_logical_path(b"/a/./b/../c"), b"/a/c");
            assert_eq!(canonicalize_logical_path(b"/"), b"/");
            assert_eq!(canonicalize_logical_path(b"/a/b/.."), b"/a");
            assert_eq!(canonicalize_logical_path(b"/../.."), b"/");
        });
    }

    #[test]
    fn cd_physical_no_dash_e_getcwd_fails_uses_curpath() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/home"),
                realpath(any, any) -> realpath("/home"),
                realpath(any, any) -> realpath("/home"),
                chdir(str(b"/tmp")) -> 0,
                getcwd() -> err(libc::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/home".to_vec());
                let outcome = invoke(
                    &mut shell,
                    &[b"cd".to_vec(), b"-P".to_vec(), b"/tmp".to_vec()],
                )
                .expect("cd -P getcwd fail");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var(b"PWD"), Some(b"/tmp" as &[u8]));
            },
        );
    }

    #[test]
    fn cd_logical_curpath_pwd_ends_with_slash() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/"),
                realpath(any, any) -> realpath("/"),
                realpath(any, any) -> realpath("/"),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/".to_vec());
                let result = cd_logical_curpath(&shell, b"tmp").expect("curpath");
                assert_eq!(result, b"/tmp");
            },
        );
    }

    #[test]
    fn cd_home_when_set() {
        run_trace(
            trace_entries![
                getcwd() -> cwd("/old"),
                realpath(any, any) -> realpath("/old"),
                realpath(any, any) -> realpath("/old"),
                chdir(str(b"/home/user")) -> 0,
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"PWD".to_vec(), b"/old".to_vec());
                shell.env.insert(b"HOME".to_vec(), b"/home/user".to_vec());
                let outcome = invoke(&mut shell, &[b"cd".to_vec()]).expect("cd home");
                assert!(matches!(outcome, BuiltinOutcome::Status(0)));
                assert_eq!(shell.get_var(b"PWD"), Some(b"/home/user" as &[u8]));
            },
        );
    }

    #[test]
    fn resolve_cd_target_no_cdpath() {
        assert_no_syscalls(|| {
            let shell = test_shell();
            let (resolved, _, print) = resolve_cd_target(&shell, b"subdir", false);
            assert_eq!(resolved, b"subdir");
            assert!(!print);
        });
    }
}
