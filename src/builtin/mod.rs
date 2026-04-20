use crate::bstr::{self, ByteWriter};
use crate::shell::error::{ShellError, VarError};
use crate::shell::options::OptionError;
use crate::shell::state::Shell;
use crate::sys;

fn remove_file_bytes(path: &[u8]) {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let _ = std::fs::remove_file(OsStr::from_bytes(path));
}

fn write_stderr(msg: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDERR_FILENO, msg);
}

#[cfg(test)]
fn write_stdout(msg: &[u8]) {
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, msg);
}

fn write_stdout_line(msg: &[u8]) {
    let mut buf = msg.to_vec();
    buf.push(b'\n');
    let _ = sys::fd_io::write_all_fd(sys::constants::STDOUT_FILENO, &buf);
}

fn diag_status(shell: &Shell, status: i32, msg: &[u8]) -> BuiltinOutcome {
    shell.diagnostic(status, msg);
    BuiltinOutcome::Status(status)
}

fn diag_status_syserr(
    shell: &Shell,
    status: i32,
    prefix: &[u8],
    e: &sys::error::SysError,
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
pub(crate) enum BuiltinOutcome {
    Status(i32),
    UtilityError(i32),
    Exit(i32),
    Return(i32),
    Break(usize),
    Continue(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BuiltinKind {
    Special,
    Regular,
}

pub(crate) type BuiltinHandler = fn(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError>;

#[derive(Debug)]
pub(crate) struct BuiltinEntry {
    pub(crate) name: &'static [u8],
    pub(crate) kind: BuiltinKind,
    pub(crate) handler: BuiltinHandler,
}

pub(crate) fn is_builtin(name: &[u8]) -> bool {
    lookup(name).is_some()
}

pub(crate) fn is_special_builtin(name: &[u8]) -> bool {
    matches!(lookup(name), Some(e) if matches!(e.kind, BuiltinKind::Special))
}

/// O(1) first-byte-dispatched lookup for builtin names. Hot path for
/// classification in `execute_simple` and the entry point used by the
/// `argv[0]` memo on `SimpleCommand`.
#[inline]
pub(crate) fn lookup(name: &[u8]) -> Option<&'static BuiltinEntry> {
    let Some(&first) = name.first() else {
        return None;
    };
    let bucket: &'static [BuiltinEntry] = match first {
        b'.' => BUILTINS_DOT,
        b':' => BUILTINS_COLON,
        b'[' => BUILTINS_LBRACKET,
        b'a' => BUILTINS_A,
        b'b' => BUILTINS_B,
        b'c' => BUILTINS_C,
        b'e' => BUILTINS_E,
        b'f' => BUILTINS_F,
        b'g' => BUILTINS_G,
        b'h' => BUILTINS_H,
        b'j' => BUILTINS_J,
        b'k' => BUILTINS_K,
        b'p' => BUILTINS_P,
        b'r' => BUILTINS_R,
        b's' => BUILTINS_S,
        b't' => BUILTINS_T,
        b'u' => BUILTINS_U,
        b'w' => BUILTINS_W,
        _ => return None,
    };
    for entry in bucket {
        if entry.name == name {
            return Some(entry);
        }
    }
    None
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

use alias::{alias, unalias};
use cd::cd;
use command::{command, hash, type_builtin};
use dot::dot;
use echo::echo_builtin;
use eval::eval;
use exec::exec_builtin;
use exit_builtin::exit;
use fc::fc;
use flow::{break_builtin, continue_builtin, return_builtin};
use getopts::getopts;
use jobs::{bg, fg, jobs, wait};
use kill::kill;
use printf::printf_builtin;
use pwd::pwd;
use read::read;
use set::{set, shift};
use test_builtin::test_builtin;
use times::times;
use trap::trap;
use ulimit::ulimit;
use umask::umask;
use vars::{export, readonly, unset};

pub(crate) fn run(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    let Some(first) = argv.first() else {
        return Ok(BuiltinOutcome::Status(0));
    };
    match lookup(first.as_slice()) {
        Some(entry) => (entry.handler)(shell, argv, cmd_assignments),
        None => Ok(BuiltinOutcome::Status(127)),
    }
}

/// Dispatch a command through a previously-resolved `BuiltinEntry`.
/// Used by the `argv[0]` memo fast path to skip lookup entirely.
#[inline]
pub(crate) fn run_entry(
    entry: &BuiltinEntry,
    shell: &mut Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    (entry.handler)(shell, argv, cmd_assignments)
}

// --- Handler adapters: normalize each builtin's signature to
// `BuiltinHandler`. Most builtins already return
// `Result<BuiltinOutcome, ShellError>` and take `(&mut Shell,
// &[Vec<u8>])`; the adapters fan in the ignored `cmd_assignments`
// parameter and wrap the few infallible or differently-shaped handlers.

fn h_colon(
    _shell: &mut Shell,
    _argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(BuiltinOutcome::Status(0))
}

fn h_true(
    _shell: &mut Shell,
    _argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(BuiltinOutcome::Status(0))
}

fn h_false(
    _shell: &mut Shell,
    _argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(BuiltinOutcome::Status(1))
}

fn h_test(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    test_builtin(shell, argv)
}

fn h_echo(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    echo_builtin(shell, argv)
}

fn h_printf(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    printf_builtin(shell, argv)
}

fn h_cd(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    cd(shell, argv)
}

fn h_pwd(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    pwd(shell, argv)
}

fn h_exit(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    exit(shell, argv)
}

fn h_export(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    export(shell, argv)
}

fn h_readonly(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    readonly(shell, argv)
}

fn h_unset(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    unset(shell, argv)
}

fn h_set(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(set(shell, argv))
}

fn h_shift(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    shift(shell, argv)
}

fn h_eval(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    eval(shell, argv)
}

fn h_dot(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    dot(shell, argv)
}

fn h_exec(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    cmd_assignments: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    exec_builtin(shell, argv, cmd_assignments)
}

fn h_jobs(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(jobs(shell, argv))
}

fn h_fg(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    fg(shell, argv)
}

fn h_bg(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    bg(shell, argv)
}

fn h_wait(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    wait(shell, argv)
}

fn h_kill(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    kill(shell, argv)
}

fn h_read(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    read(shell, argv)
}

fn h_getopts(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    getopts(shell, argv)
}

fn h_alias(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    alias(shell, argv)
}

fn h_unalias(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    unalias(shell, argv)
}

fn h_return(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    return_builtin(shell, argv)
}

fn h_break(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    break_builtin(shell, argv)
}

fn h_continue(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    continue_builtin(shell, argv)
}

fn h_times(
    shell: &mut Shell,
    _argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(times(shell))
}

fn h_trap(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    Ok(trap(shell, argv))
}

fn h_umask(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    umask(shell, argv)
}

fn h_command(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    command(shell, argv)
}

fn h_type(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    type_builtin(shell, argv)
}

fn h_hash(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    hash(shell, argv)
}

fn h_fc(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    fc(shell, argv)
}

fn h_ulimit(
    shell: &mut Shell,
    argv: &[Vec<u8>],
    _ca: &[(Vec<u8>, Vec<u8>)],
) -> Result<BuiltinOutcome, ShellError> {
    ulimit(shell, argv)
}

// --- Static per-first-byte dispatch tables. Union of Special+Regular
// is the full builtin name set; each entry tags its kind.

use BuiltinKind::Regular as Reg;
use BuiltinKind::Special as Spc;

const BUILTINS_DOT: &[BuiltinEntry] = &[BuiltinEntry {
    name: b".",
    kind: Spc,
    handler: h_dot,
}];
const BUILTINS_COLON: &[BuiltinEntry] = &[BuiltinEntry {
    name: b":",
    kind: Spc,
    handler: h_colon,
}];
const BUILTINS_LBRACKET: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"[",
    kind: Reg,
    handler: h_test,
}];
const BUILTINS_A: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"alias",
    kind: Reg,
    handler: h_alias,
}];
const BUILTINS_B: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"bg",
        kind: Reg,
        handler: h_bg,
    },
    BuiltinEntry {
        name: b"break",
        kind: Spc,
        handler: h_break,
    },
];
const BUILTINS_C: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"cd",
        kind: Reg,
        handler: h_cd,
    },
    BuiltinEntry {
        name: b"command",
        kind: Reg,
        handler: h_command,
    },
    BuiltinEntry {
        name: b"continue",
        kind: Spc,
        handler: h_continue,
    },
];
const BUILTINS_E: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"echo",
        kind: Reg,
        handler: h_echo,
    },
    BuiltinEntry {
        name: b"eval",
        kind: Spc,
        handler: h_eval,
    },
    BuiltinEntry {
        name: b"exec",
        kind: Spc,
        handler: h_exec,
    },
    BuiltinEntry {
        name: b"exit",
        kind: Spc,
        handler: h_exit,
    },
    BuiltinEntry {
        name: b"export",
        kind: Spc,
        handler: h_export,
    },
];
const BUILTINS_F: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"false",
        kind: Reg,
        handler: h_false,
    },
    BuiltinEntry {
        name: b"fc",
        kind: Reg,
        handler: h_fc,
    },
    BuiltinEntry {
        name: b"fg",
        kind: Reg,
        handler: h_fg,
    },
];
const BUILTINS_G: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"getopts",
    kind: Reg,
    handler: h_getopts,
}];
const BUILTINS_H: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"hash",
    kind: Reg,
    handler: h_hash,
}];
const BUILTINS_J: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"jobs",
    kind: Reg,
    handler: h_jobs,
}];
const BUILTINS_K: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"kill",
    kind: Reg,
    handler: h_kill,
}];
const BUILTINS_P: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"printf",
        kind: Reg,
        handler: h_printf,
    },
    BuiltinEntry {
        name: b"pwd",
        kind: Reg,
        handler: h_pwd,
    },
];
const BUILTINS_R: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"read",
        kind: Reg,
        handler: h_read,
    },
    BuiltinEntry {
        name: b"readonly",
        kind: Spc,
        handler: h_readonly,
    },
    BuiltinEntry {
        name: b"return",
        kind: Spc,
        handler: h_return,
    },
];
const BUILTINS_S: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"set",
        kind: Spc,
        handler: h_set,
    },
    BuiltinEntry {
        name: b"shift",
        kind: Spc,
        handler: h_shift,
    },
];
const BUILTINS_T: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"test",
        kind: Reg,
        handler: h_test,
    },
    BuiltinEntry {
        name: b"times",
        kind: Spc,
        handler: h_times,
    },
    BuiltinEntry {
        name: b"trap",
        kind: Spc,
        handler: h_trap,
    },
    BuiltinEntry {
        name: b"true",
        kind: Reg,
        handler: h_true,
    },
    BuiltinEntry {
        name: b"type",
        kind: Reg,
        handler: h_type,
    },
];
const BUILTINS_U: &[BuiltinEntry] = &[
    BuiltinEntry {
        name: b"ulimit",
        kind: Reg,
        handler: h_ulimit,
    },
    BuiltinEntry {
        name: b"umask",
        kind: Reg,
        handler: h_umask,
    },
    BuiltinEntry {
        name: b"unalias",
        kind: Reg,
        handler: h_unalias,
    },
    BuiltinEntry {
        name: b"unset",
        kind: Spc,
        handler: h_unset,
    },
];
const BUILTINS_W: &[BuiltinEntry] = &[BuiltinEntry {
    name: b"wait",
    kind: Reg,
    handler: h_wait,
}];

#[cfg(test)]
pub(super) mod test_support {
    use crate::shell::error::ShellError;
    use crate::shell::state::Shell;

    use super::BuiltinOutcome;

    pub(crate) fn test_shell() -> Shell {
        let mut shell = crate::shell::test_support::test_shell();
        shell.last_status = 3;
        shell
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
    use super::cd::resolve_cd_target;
    use super::*;
    use crate::builtin::test_support::{diag, invoke, test_shell};
    use crate::sys;
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
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
    fn write_stdout_coverage() {
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDOUT_FILENO), bytes(b"hi")) -> auto,],
            || {
                write_stdout(b"hi");
            },
        );
    }

    #[test]
    fn diag_status_syserr_coverage() {
        let msg = diag(b"open: No such file or directory");
        run_trace(
            trace_entries![write(fd(crate::sys::constants::STDERR_FILENO), bytes(&msg)) -> auto,],
            || {
                let shell = test_shell();
                let e = sys::error::SysError::Errno(sys::constants::ENOENT);
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
            shell.env_mut().insert(b"CDPATH".to_vec(), b":".to_vec());
            let (resolved, _, print) = resolve_cd_target(&shell, b"subdir", false);
            assert_eq!(resolved, b"./subdir");
            assert!(!print);
        });
    }

    #[test]
    fn resolve_cd_target_cdpath_no_match() {
        run_trace(
            trace_entries![
                stat(any, any) -> err(sys::constants::ENOENT),
                stat(any, any) -> err(sys::constants::ENOENT),
            ],
            || {
                let mut shell = test_shell();
                shell
                    .env_mut()
                    .insert(b"CDPATH".to_vec(), b"/a:/b".to_vec());
                let (resolved, _, _) = resolve_cd_target(&shell, b"nosuch", false);
                assert_eq!(resolved, b"nosuch");
            },
        );
    }
}
