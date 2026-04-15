use std::collections::BTreeSet;

use crate::bstr::{self, BStrExt, ByteWriter};
use crate::shell::{OptionError, Shell, ShellError, TrapAction, TrapCondition, VarError};
use crate::sys;

fn remove_file_bytes(path: &[u8]) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let _ = std::fs::remove_file(OsStr::from_bytes(path));
}

fn write_stderr(msg: &[u8]) {
    let _ = sys::write_all_fd(sys::STDERR_FILENO, msg);
}

#[cfg(test)]
fn write_stdout(msg: &[u8]) {
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, msg);
}

fn write_stdout_line(msg: &[u8]) {
    let mut buf = msg.to_vec();
    buf.push(b'\n');
    let _ = sys::write_all_fd(sys::STDOUT_FILENO, &buf);
}

fn diag_status(shell: &Shell, status: i32, msg: &[u8]) -> BuiltinOutcome {
    shell.diagnostic(status, msg);
    BuiltinOutcome::Status(status)
}

fn diag_status_syserr(
    shell: &Shell,
    status: i32,
    prefix: &[u8],
    e: &sys::SysError,
) -> BuiltinOutcome {
    let msg = ByteWriter::new()
        .bytes(prefix)
        .bytes(&e.strerror())
        .finish();
    shell.diagnostic(status, &msg);
    BuiltinOutcome::Status(status)
}

fn parse_usize(s: &[u8]) -> Option<usize> {
    let val = bstr::parse_i64(s)?;
    if val >= 0 { Some(val as usize) } else { None }
}

fn parse_i32(s: &[u8]) -> Option<i32> {
    let val = bstr::parse_i64(s)?;
    if val >= i32::MIN as i64 && val <= i32::MAX as i64 {
        Some(val as i32)
    } else {
        None
    }
}

fn var_error_msg(prefix: &[u8], e: &VarError) -> Vec<u8> {
    match e {
        VarError::Readonly(name) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": readonly variable: ")
            .bytes(name)
            .finish(),
    }
}

fn option_error_msg(prefix: &[u8], e: &OptionError) -> Vec<u8> {
    match e {
        OptionError::InvalidShort(ch) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": invalid option: ")
            .byte(*ch)
            .finish(),
        OptionError::InvalidName(name) => ByteWriter::new()
            .bytes(prefix)
            .bytes(b": invalid option: ")
            .bytes(name)
            .finish(),
    }
}

#[derive(Debug)]
pub enum BuiltinOutcome {
    Status(i32),
    UtilityError(i32),
    Exit(i32),
    Return(i32),
    Break(usize),
    Continue(usize),
}

const BUILTIN_NAMES: &[&[u8]] = &[
    b".",
    b":",
    b"[",
    b"alias",
    b"bg",
    b"break",
    b"cd",
    b"command",
    b"continue",
    b"echo",
    b"eval",
    b"exec",
    b"exit",
    b"export",
    b"false",
    b"fc",
    b"fg",
    b"getopts",
    b"hash",
    b"jobs",
    b"kill",
    b"printf",
    b"pwd",
    b"read",
    b"readonly",
    b"return",
    b"set",
    b"shift",
    b"test",
    b"times",
    b"trap",
    b"true",
    b"type",
    b"ulimit",
    b"umask",
    b"unalias",
    b"unset",
    b"wait",
];

pub fn is_builtin(name: &[u8]) -> bool {
    BUILTIN_NAMES.binary_search(&name).is_ok()
}

const SPECIAL_BUILTIN_NAMES: &[&[u8]] = &[
    b".",
    b":",
    b"break",
    b"continue",
    b"eval",
    b"exec",
    b"exit",
    b"export",
    b"readonly",
    b"return",
    b"set",
    b"shift",
    b"times",
    b"trap",
    b"unset",
];

pub fn is_special_builtin(name: &[u8]) -> bool {
    SPECIAL_BUILTIN_NAMES.binary_search(&name).is_ok()
}

mod alias;
mod cd;
mod command;
mod dot;
mod echo;
mod eval;
mod exec;
mod exit_builtin;
mod fc;
mod flow;
mod getopts;
mod jobs;
mod kill;
mod printf;
mod pwd;
mod read;
mod set;
mod test_builtin;
mod times;
mod trap;
mod ulimit;
mod umask;
mod vars;

use alias::*;
use cd::*;
use command::*;
use dot::*;
use echo::*;
use eval::*;
use exec::*;
use exit_builtin::*;
use fc::*;
use flow::*;
use getopts::*;
use jobs::*;
use kill::*;
use printf::*;
use pwd::*;
use read::*;
use set::*;
use test_builtin::*;
use times::*;
use trap::*;
use ulimit::*;
use umask::*;
use vars::*;

pub fn run(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    if argv.is_empty() {
        return Ok(BuiltinOutcome::Status(0));
    }

    let outcome = match argv[0].as_slice() {
        b":" | b"true" => BuiltinOutcome::Status(0),
        b"false" => BuiltinOutcome::Status(1),
        b"[" | b"test" => test_builtin(shell, argv)?,
        b"echo" => echo_builtin(shell, argv)?,
        b"printf" => printf_builtin(shell, argv)?,
        b"cd" => cd(shell, argv)?,
        b"pwd" => pwd(shell, argv)?,
        b"exit" => exit(shell, argv)?,
        b"export" => export(shell, argv)?,
        b"readonly" => readonly(shell, argv)?,
        b"unset" => unset(shell, argv)?,
        b"set" => set(shell, argv),
        b"shift" => shift(shell, argv)?,
        b"eval" => eval(shell, argv)?,
        b"." => dot(shell, argv)?,
        b"exec" => exec_builtin(shell, argv, cmd_assignments)?,
        b"jobs" => jobs(shell, argv),
        b"fg" => fg(shell, argv)?,
        b"bg" => bg(shell, argv)?,
        b"wait" => wait(shell, argv)?,
        b"kill" => kill(shell, argv)?,
        b"read" => read(shell, argv)?,
        b"getopts" => getopts(shell, argv)?,
        b"alias" => alias(shell, argv)?,
        b"unalias" => unalias(shell, argv)?,
        b"return" => return_builtin(shell, argv)?,
        b"break" => break_builtin(shell, argv)?,
        b"continue" => continue_builtin(shell, argv)?,
        b"times" => times(shell),
        b"trap" => trap(shell, argv),
        b"umask" => umask(shell, argv)?,
        b"command" => command(shell, argv)?,
        b"type" => type_builtin(shell, argv)?,
        b"hash" => hash(shell, argv)?,
        b"fc" => fc(shell, argv)?,
        b"ulimit" => ulimit(shell, argv)?,
        _ => BuiltinOutcome::Status(127),
    };

    Ok(outcome)
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    use crate::shell::ShellOptions;
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    pub(crate) use crate::sys::test_support::{assert_no_syscalls, run_trace};

    pub(crate) fn test_shell() -> Shell {
        Shell {
            options: ShellOptions::default(),
            shell_name: (*b"meiksh").into(),
            env: HashMap::new(),
            exported: BTreeSet::new(),
            readonly: BTreeSet::new(),
            aliases: HashMap::new(),
            functions: HashMap::new(),
            positional: Vec::new(),
            last_status: 3,
            last_background: None,
            running: true,
            jobs: Vec::new(),
            known_pid_statuses: HashMap::new(),
            known_job_statuses: HashMap::new(),
            trap_actions: BTreeMap::new(),
            ignored_on_entry: BTreeSet::new(),
            subshell_saved_traps: None,
            loop_depth: 0,
            function_depth: 0,
            source_depth: 0,
            pending_control: None,
            interactive: false,
            errexit_suppressed: false,
            owns_terminal: false,
            in_subshell: false,
            wait_was_interrupted: false,
            pid: 0,
            lineno: 0,
            path_cache: HashMap::new(),
            history: Vec::new(),
            mail_last_check: 0,
            mail_sizes: HashMap::new(),
        }
    }

    pub(crate) fn invoke(
        shell: &mut Shell,
        argv: &[Vec<u8>],
    ) -> Result<BuiltinOutcome, ShellError> {
        super::run(shell, argv, &[])
    }

    pub(crate) fn diag(msg: &[u8]) -> Vec<u8> {
        let mut v = b"meiksh: ".to_vec();
        v.extend_from_slice(msg);
        v.push(b'\n');
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn builtin_registry_knows_core_commands() {
        assert_no_syscalls(|| {
            assert!(is_builtin(b"cd"));
            assert!(is_builtin(b"export"));
            assert!(is_builtin(b"read"));
            assert!(is_builtin(b"umask"));
            assert!(is_builtin(b"printf"));
            assert!(is_builtin(b"echo"));
            assert!(is_builtin(b"test"));
            assert!(is_builtin(b"["));
        });
    }

    #[test]
    fn parse_hex_i64_helper() {
        assert_no_syscalls(|| {
            assert_eq!(parse_hex_i64(b"ff"), Some(255));
            assert_eq!(parse_hex_i64(b"FF"), Some(255));
            assert_eq!(parse_hex_i64(b"0"), Some(0));
            assert_eq!(parse_hex_i64(b""), None);
            assert_eq!(parse_hex_i64(b"zz"), None);
        });
    }

    #[test]
    fn parse_octal_i64_helper() {
        assert_no_syscalls(|| {
            assert_eq!(parse_octal_i64(b"77"), Some(63));
            assert_eq!(parse_octal_i64(b"0"), Some(0));
            assert_eq!(parse_octal_i64(b""), None);
            assert_eq!(parse_octal_i64(b"89"), None);
        });
    }

    #[test]
    fn write_stdout_coverage() {
        run_trace(
            trace_entries![write(fd(crate::sys::STDOUT_FILENO), bytes(b"hi")) -> auto,],
            || {
                write_stdout(b"hi");
            },
        );
    }

    #[test]
    fn diag_status_syserr_coverage() {
        let msg = diag(b"open: No such file or directory");
        run_trace(
            trace_entries![write(fd(crate::sys::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let shell = test_shell();
                let e = sys::SysError::Errno(libc::ENOENT);
                let outcome = diag_status_syserr(&shell, 1, b"open: ", &e);
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }

    #[test]
    fn parse_i32_out_of_range() {
        assert_no_syscalls(|| {
            assert_eq!(parse_i32(b"2147483648"), None);
            assert_eq!(parse_i32(b"-2147483649"), None);
            assert_eq!(parse_i32(b"42"), Some(42));
        });
    }

    #[test]
    fn run_empty_argv_returns_zero() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[]).expect("empty argv");
            assert!(matches!(outcome, BuiltinOutcome::Status(0)));
        });
    }

    #[test]
    fn run_unknown_builtin_returns_127() {
        assert_no_syscalls(|| {
            let mut shell = test_shell();
            let outcome = invoke(&mut shell, &[b"nonexistent_cmd".to_vec()]).expect("unknown");
            assert!(matches!(outcome, BuiltinOutcome::Status(127)));
        });
    }

    #[test]
    fn resolve_cd_target_cdpath_empty_prefix() {
        run_trace(trace_entries![stat(any, any) -> stat_dir,], || {
            let mut shell = test_shell();
            shell.env.insert(b"CDPATH".to_vec(), b":".to_vec());
            let (resolved, _, print) = resolve_cd_target(&shell, b"subdir", false);
            assert_eq!(resolved, b"./subdir");
            assert!(!print);
        });
    }

    #[test]
    fn resolve_cd_target_cdpath_no_match() {
        run_trace(
            trace_entries![
                stat(any, any) -> err(libc::ENOENT),
                stat(any, any) -> err(libc::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell.env.insert(b"CDPATH".to_vec(), b"/a:/b".to_vec());
                let (resolved, _, _) = resolve_cd_target(&shell, b"nosuch", false);
                assert_eq!(resolved, b"nosuch");
            },
        );
    }
}
