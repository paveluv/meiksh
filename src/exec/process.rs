#![allow(unused_imports)]

use std::collections::HashMap;

use crate::arena::ByteArena;
use crate::bstr::ByteWriter;
use crate::builtin;
use crate::expand;
use crate::shell::{
    BlockingWaitOutcome, FlowSignal, JobState, PendingControl, Shell, ShellError, VarError,
};
use crate::syntax::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};
use crate::sys;

use super::*;

pub(super) fn join_boxed_bytes(parts: &[Box<[u8]>], sep: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.push(sep);
        }
        out.extend_from_slice(part);
    }
    out
}

#[derive(Debug)]
pub(super) struct ExpandedSimpleCommand<'a> {
    pub(super) assignments: Vec<(&'a [u8], &'a [u8])>,
    pub(super) argv: Vec<&'a [u8]>,
    pub(super) redirections: Vec<ExpandedRedirection<'a>>,
}

#[derive(Clone, Debug)]
pub(super) struct ExpandedRedirection<'a> {
    pub(super) fd: i32,
    pub(super) kind: RedirectionKind,
    pub(super) target: &'a [u8],
    pub(super) here_doc_body: Option<&'a [u8]>,
    pub(super) line: usize,
}

#[derive(Debug, Clone)]
pub(super) struct ProcessRedirection {
    pub(super) fd: i32,
    pub(super) kind: RedirectionKind,
    pub(super) target: Box<[u8]>,
    pub(super) here_doc_body: Option<Box<[u8]>>,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedProcess {
    pub(super) exec_path: Box<[u8]>,
    pub(super) argv: Box<[Box<[u8]>]>,
    pub(super) child_env: Box<[(Box<[u8]>, Box<[u8]>)]>,
    pub(super) redirections: Vec<ProcessRedirection>,
    pub(super) noclobber: bool,
    pub(super) path_verified: bool,
}

impl RedirectionRef for ProcessRedirection {
    fn fd(&self) -> i32 {
        self.fd
    }
    fn kind(&self) -> RedirectionKind {
        self.kind
    }
    fn target(&self) -> &[u8] {
        &self.target
    }
    fn here_doc_body(&self) -> Option<&[u8]> {
        self.here_doc_body.as_deref()
    }
}

pub(super) fn file_needs_binary_rejection(path: &[u8]) -> bool {
    let fd = match sys::open_file(path, sys::O_RDONLY | sys::O_CLOEXEC, 0) {
        Ok(fd) => fd,
        Err(_) => return false,
    };
    let mut buf = [0u8; 256];
    let n = match sys::read_fd(fd, &mut buf) {
        Ok(n) => n,
        Err(_) => {
            let _ = sys::close_fd(fd);
            return false;
        }
    };
    let _ = sys::close_fd(fd);
    if n == 0 {
        return false;
    }
    let prefix = &buf[..n];
    if n >= 4 && prefix[0] == 0x7f && prefix[1] == b'E' {
        return false;
    }
    if n >= 2 && prefix[0] == b'#' && prefix[1] == b'!' {
        return false;
    }
    let nl_pos = prefix.iter().position(|&b| b == b'\n').unwrap_or(n);
    prefix[..nl_pos].contains(&0)
}

pub(super) fn prepare_prepared_process(
    shell: &Shell,
    prepared: &PreparedProcess,
) -> Result<PreparedRedirections, ShellError> {
    if !prepared.path_verified
        && !prepared.exec_path.is_empty()
        && prepared.exec_path.contains(&b'/')
    {
        if sys::access_path(&prepared.exec_path, sys::F_OK).is_err() {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": not found\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            return Err(ShellError::Status(127));
        }
        if sys::access_path(&prepared.exec_path, sys::X_OK).is_err() {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": Permission denied\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            return Err(ShellError::Status(126));
        }
    }

    prepare_redirections(&prepared.redirections, prepared.noclobber)
        .map_err(|e| shell.diagnostic_syserr(1, &e))
}

pub(super) fn run_prepared_process(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
    prepared_redirections: &PreparedRedirections,
) -> ! {
    match process_group {
        ProcessGroupPlan::NewGroup => {
            let _ = sys::set_process_group(0, 0);
        }
        ProcessGroupPlan::Join(pgid) => {
            let _ = sys::set_process_group(0, pgid);
        }
        ProcessGroupPlan::None => {}
    }
    if let Err(err) = apply_child_fd_actions(&prepared_redirections.actions) {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": ")
            .bytes(&err.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        sys::exit_process(1);
    }

    for (key, value) in &prepared.child_env {
        let _ = sys::env_set_var(key, value);
    }

    if file_needs_binary_rejection(&prepared.exec_path) {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": cannot execute binary file\n")
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        sys::exit_process(126);
    }

    shell.restore_signals_for_child();
    match sys::exec_replace(&prepared.exec_path, &prepared.argv) {
        Err(err) if err.is_enoexec() => {
            let mut child_shell = shell.clone();
            child_shell.owns_terminal = false;
            child_shell.in_subshell = true;
            let _ = child_shell.reset_traps_for_subshell();
            child_shell.shell_name = prepared.argv[0].clone();
            child_shell.positional = prepared.argv[1..].iter().map(|s| s.to_vec()).collect();
            let status = child_shell.source_path(&prepared.exec_path).unwrap_or(126);
            sys::exit_process(status as sys::RawFd);
        }
        Err(err) if err.is_enoent() => {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": not found\n")
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            sys::exit_process(127);
        }
        Err(err) => {
            let msg = ByteWriter::new()
                .bytes(&prepared.argv[0])
                .bytes(b": ")
                .bytes(&err.strerror())
                .byte(b'\n')
                .finish();
            let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
            sys::exit_process(126);
        }
        Ok(()) => sys::exit_process(0),
    }
}

pub(super) fn exec_prepared_in_current_process(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
) -> ! {
    let prepared_redirections = match prepare_prepared_process(shell, prepared) {
        Ok(prepared_redirections) => prepared_redirections,
        Err(error) => sys::exit_process(error.exit_status() as sys::RawFd),
    };
    run_prepared_process(shell, prepared, process_group, &prepared_redirections)
}

pub(super) fn spawn_prepared(
    shell: &Shell,
    prepared: &PreparedProcess,
    process_group: ProcessGroupPlan,
) -> Result<sys::ChildHandle, ShellError> {
    let prepared_redirections = prepare_prepared_process(shell, prepared)?;

    let pid = sys::fork_process().map_err(|e| {
        let msg = ByteWriter::new()
            .bytes(&prepared.argv[0])
            .bytes(b": ")
            .bytes(&e.strerror())
            .byte(b'\n')
            .finish();
        let _ = sys::write_all_fd(sys::STDERR_FILENO, &msg);
        ShellError::Status(1)
    })?;
    if pid == 0 {
        run_prepared_process(shell, prepared, process_group, &prepared_redirections);
    }

    close_parent_redirection_fds(&prepared_redirections);

    Ok(sys::ChildHandle {
        pid,
        stdout_fd: None,
    })
}

pub(super) fn resolve_command_path(
    shell: &Shell,
    program: &[u8],
    path_override: Option<&[u8]>,
) -> Option<Vec<u8>> {
    if program.contains(&b'/') {
        return Some(program.to_vec());
    }

    if path_override.is_none()
        && let Some(cached) = shell.path_cache.get(program)
    {
        return Some(cached.clone());
    }

    let path = path_override
        .map(|s| s.to_vec())
        .or_else(|| shell.get_var(b"PATH").map(|s| s.to_vec()))
        .or_else(|| sys::env_var(b"PATH"))
        .unwrap_or_default();

    split_bytes(&path, b':')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut candidate = segment.to_vec();
            candidate.push(b'/');
            candidate.extend_from_slice(program);
            candidate
        })
        .find(|candidate| {
            sys::stat_path(candidate)
                .map(|stat| stat.is_regular_file() && stat.is_executable())
                .unwrap_or(false)
        })
}

pub(super) fn split_bytes(data: &[u8], sep: u8) -> impl Iterator<Item = &[u8]> {
    data.split(move |&b| b == sep)
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::test_support::*;
    use crate::shell::Shell;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};
    use crate::trace_entries;

    #[test]
    fn build_process_from_expanded_covers_empty_and_assignment_env() {
        run_trace(
            trace_entries![
                write(fd(sys::STDERR_FILENO), bytes(b"meiksh: empty command\n")) -> auto,
            ],
            || {
                let arena = ByteArena::new();
                let shell = test_shell();
                let error = build_process_from_expanded(
                    &shell,
                    ExpandedSimpleCommand {
                        assignments: Vec::new(),

                        argv: Vec::new(),

                        redirections: Vec::new(),
                    },
                    Vec::new(),
                    Vec::new(),
                )
                .expect_err("empty command");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                shell.env.insert(b"PATH".to_vec(), Vec::new());
                let expanded = ExpandedSimpleCommand {
                    assignments: vec![(
                        arena.intern_bytes(b"ASSIGN_VAR"),
                        arena.intern_bytes(b"works"),
                    )],
                    argv: vec![arena.intern_bytes(b"echo"), arena.intern_bytes(b"hello")],
                    redirections: Vec::new(),
                };
                let owned_argv: Vec<Vec<u8>> = expanded.argv.iter().map(|s| s.to_vec()).collect();
                let owned_assignments: Vec<(Vec<u8>, Vec<u8>)> = expanded
                    .assignments
                    .iter()
                    .map(|&(n, v)| (n.to_vec(), v.to_vec()))
                    .collect();
                let prepared =
                    build_process_from_expanded(&shell, expanded, owned_argv, owned_assignments)
                        .expect("process");
                assert_eq!(
                    &*prepared.child_env,
                    &[(
                        Box::from(b"ASSIGN_VAR" as &[u8]),
                        Box::from(b"works" as &[u8])
                    )] as &[(Box<[u8]>, Box<[u8]>)]
                );
                assert_eq!(
                    &*prepared.argv,
                    &[Box::from(b"echo" as &[u8]), Box::from(b"hello" as &[u8])] as &[Box<[u8]>]
                );
            },
        );
    }

    #[test]
    fn spawn_prepared_enoexec_falls_back_to_source() {
        run_trace(
            trace_entries![
                fork() -> pid(1000), child: [
                    open(str("/tmp/script.sh"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"echo hello\n"),
                    close(fd(20)) -> int(0),
                    execvp(str("/tmp/script.sh"), _) -> err(sys::ENOEXEC),
                    open(str("/tmp/script.sh"), _, _) -> fd(10),
                    read(fd(10), _) -> bytes(b"true\n"),
                    read(fd(10), _) -> int(0),
                    close(fd(10)) -> int(0),
                ],
                waitpid(1000, _) -> status(0),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: b"/tmp/script.sh".to_vec().into(),
                    argv: vec![b"/tmp/script.sh".to_vec().into(), b"arg1".to_vec().into()]
                        .into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: true,
                };
                let child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("enoexec fallback spawn");
                let output = child.wait_with_output().expect("output");
                assert!(output.status.success());
            },
        );
    }

    #[test]
    fn spawn_prepared_errors_for_missing_executable() {
        run_trace(
            trace_entries![
                access(str("/nonexistent/missing"), int(0)) -> err(sys::ENOENT),
                write(fd(sys::STDERR_FILENO), bytes(b"missing: not found\n")) -> auto,
            ],
            || {
                let missing = PreparedProcess {
                    exec_path: b"/nonexistent/missing".to_vec().into(),
                    argv: vec![b"missing".to_vec().into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),

                    redirections: Vec::new(),

                    noclobber: false,
                    path_verified: false,
                };
                let shell = test_shell();
                assert!(spawn_prepared(&shell, &missing, ProcessGroupPlan::None).is_err());
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_handles_errors_and_empty() {
        run_trace(
            trace_entries![
                open(_, _, _) -> err(sys::EACCES),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> err(libc::EIO),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> int(0),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/file"));
            },
        );
    }

    #[test]
    fn join_boxed_bytes_various_inputs() {
        assert_no_syscalls(|| {
            let empty: Vec<Box<[u8]>> = vec![];
            assert_eq!(join_boxed_bytes(&empty, b' '), b"");

            let single: Vec<Box<[u8]>> = vec![b"hello".to_vec().into()];
            assert_eq!(join_boxed_bytes(&single, b' '), b"hello");

            let multi: Vec<Box<[u8]>> = vec![
                b"a".to_vec().into(),
                b"bb".to_vec().into(),
                b"ccc".to_vec().into(),
            ];
            assert_eq!(join_boxed_bytes(&multi, b' '), b"a bb ccc");
            assert_eq!(join_boxed_bytes(&multi, b'/'), b"a/bb/ccc");
        });
    }

    #[test]
    fn file_needs_binary_rejection_elf_prefix_allowed() {
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> bytes(b"\x7fELF\x02\x01\x01\x00"),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/elf"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_shebang_allowed() {
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> bytes(b"#!/bin/sh\necho hi\n"),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/script"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_null_byte_triggers() {
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> bytes(b"binary\x00data\n"),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(file_needs_binary_rejection(b"/some/binary"));
            },
        );
    }

    #[test]
    fn file_needs_binary_rejection_text_without_null_ok() {
        run_trace(
            trace_entries![
                open(_, _, _) -> fd(50),
                read(fd(50), _) -> bytes(b"just plain text\n"),
                close(fd(50)) -> int(0),
            ],
            || {
                assert!(!file_needs_binary_rejection(b"/some/text"));
            },
        );
    }

    #[test]
    fn split_bytes_helper() {
        assert_no_syscalls(|| {
            let parts: Vec<&[u8]> = split_bytes(b"a:b:c", b':').collect();
            assert_eq!(parts, vec![b"a" as &[u8], b"b", b"c"]);

            let single: Vec<&[u8]> = split_bytes(b"hello", b':').collect();
            assert_eq!(single, vec![b"hello" as &[u8]]);

            let empty: Vec<&[u8]> = split_bytes(b"", b':').collect();
            assert_eq!(empty, vec![b"" as &[u8]]);

            let trailing: Vec<&[u8]> = split_bytes(b"a:", b':').collect();
            assert_eq!(trailing, vec![b"a" as &[u8], b""]);
        });
    }
    #[test]
    fn spawn_child_access_denied() {
        run_trace(
            trace_entries![
                access(str("/bin/noperm"), sys::F_OK) -> 0,
                access(str("/bin/noperm"), sys::X_OK) -> err(libc::EACCES),
                write(fd(2), bytes(b"noperm: Permission denied\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    argv: vec![b"noperm".to_vec().into()].into(),
                    exec_path: b"/bin/noperm".to_vec().into(),
                    path_verified: false,
                    child_env: vec![].into(),
                    redirections: vec![],
                    noclobber: false,
                };
                let err = crate::exec::process::spawn_prepared(
                    &mut shell,
                    &prepared,
                    ProcessGroupPlan::None,
                )
                .unwrap_err();
                assert_eq!(err.exit_status(), 126);
            },
        );
    }

    #[test]
    fn spawn_path_verified_false_passes_both_checks() {
        run_trace(
            trace_entries![
                access(str("/bin/ok"), sys::F_OK) -> 0,
                access(str("/bin/ok"), sys::X_OK) -> 0,
                fork() -> pid(300), child: [
                    open(str("/bin/ok"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"#!/bin/sh\n"),
                    close(fd(20)) -> int(0),
                    execvp(str("/bin/ok"), _) -> err(sys::ENOEXEC),
                    open(str("/bin/ok"), _, _) -> fd(10),
                    read(fd(10), _) -> bytes(b"true\n"),
                    read(fd(10), _) -> int(0),
                    close(fd(10)) -> int(0),
                ],
                waitpid(300, _) -> status(0),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    argv: vec![b"/bin/ok".to_vec().into()].into(),
                    exec_path: b"/bin/ok".to_vec().into(),
                    path_verified: false,
                    child_env: vec![].into(),
                    redirections: vec![],
                    noclobber: false,
                };
                let child =
                    spawn_prepared(&shell, &prepared, ProcessGroupPlan::None).expect("spawn ok");
                assert_eq!(child.pid, 300);
                let _ = sys::wait_pid(300, false);
            },
        );
    }

    #[test]
    fn spawn_child_not_found() {
        run_trace(
            trace_entries![
                access(str("/bin/missing"), sys::F_OK) -> err(libc::ENOENT),
                write(fd(2), bytes(b"missing: not found\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    argv: vec![b"missing".to_vec().into()].into(),
                    exec_path: b"/bin/missing".to_vec().into(),
                    path_verified: false,
                    child_env: vec![].into(),
                    redirections: vec![],
                    noclobber: false,
                };
                let err = crate::exec::process::spawn_prepared(
                    &mut shell,
                    &prepared,
                    ProcessGroupPlan::None,
                )
                .unwrap_err();
                assert_eq!(err.exit_status(), 127);
            },
        );
    }

    #[test]
    fn spawn_prepared_binary_rejection_exits_126() {
        run_trace(
            trace_entries![
                fork() -> pid(200), child: [
                    open(str("/tmp/binfile"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"binary\x00data\n"),
                    close(fd(20)) -> int(0),
                    write(fd(2), bytes(b"/tmp/binfile: cannot execute binary file\n")) -> auto,
                ],
                waitpid(200, _) -> status(126),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: b"/tmp/binfile".to_vec().into(),
                    argv: vec![b"/tmp/binfile".to_vec().into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let _child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("spawn for binary rejection");
                let _ = sys::wait_pid(200, false);
            },
        );
    }

    #[test]
    fn spawn_prepared_exec_enoent_exits_127() {
        run_trace(
            trace_entries![
                fork() -> pid(201), child: [
                    open(str("/tmp/gone"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"echo hello\n"),
                    close(fd(20)) -> int(0),
                    execvp(str("/tmp/gone"), _) -> err(sys::ENOENT),
                    write(fd(2), bytes(b"/tmp/gone: not found\n")) -> auto,
                ],
                waitpid(201, _) -> status(127),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: b"/tmp/gone".to_vec().into(),
                    argv: vec![b"/tmp/gone".to_vec().into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let _child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("spawn for exec enoent");
                let _ = sys::wait_pid(201, false);
            },
        );
    }

    #[test]
    fn spawn_prepared_exec_generic_error_exits_126() {
        let eio_msg = sys::SysError::Errno(libc::EIO).strerror();
        let mut expected_stderr = b"/tmp/badexec: ".to_vec();
        expected_stderr.extend_from_slice(&eio_msg);
        expected_stderr.push(b'\n');
        run_trace(
            trace_entries![
                fork() -> pid(202), child: [
                    open(str("/tmp/badexec"), _, _) -> fd(20),
                    read(fd(20), _) -> bytes(b"echo hello\n"),
                    close(fd(20)) -> int(0),
                    execvp(str("/tmp/badexec"), _) -> err(libc::EIO),
                    write(fd(2), bytes(&expected_stderr)) -> auto,
                ],
                waitpid(202, _) -> status(126),
            ],
            || {
                let shell = test_shell();
                let prepared = PreparedProcess {
                    exec_path: b"/tmp/badexec".to_vec().into(),
                    argv: vec![b"/tmp/badexec".to_vec().into()].into_boxed_slice(),
                    child_env: Vec::new().into_boxed_slice(),
                    redirections: Vec::new(),
                    noclobber: false,
                    path_verified: true,
                };
                let _child = spawn_prepared(&shell, &prepared, ProcessGroupPlan::None)
                    .expect("spawn for exec eio");
                let _ = sys::wait_pid(202, false);
            },
        );
    }

    #[test]
    fn exec_prepared_in_current_process_prepare_failure_exits() {
        run_trace(
            trace_entries![
                fork() -> pid(203), child: [
                    access(str("/nonexistent/cmd"), sys::F_OK) -> err(libc::ENOENT),
                    write(fd(2), bytes(b"cmd: not found\n")) -> auto,
                ],
                waitpid(203, _) -> status(127),
            ],
            || {
                let pid = sys::fork_process().expect("fork");
                if pid == 0 {
                    let shell = test_shell();
                    let prepared = PreparedProcess {
                        exec_path: b"/nonexistent/cmd".to_vec().into(),
                        argv: vec![b"cmd".to_vec().into()].into_boxed_slice(),
                        child_env: Vec::new().into_boxed_slice(),
                        redirections: Vec::new(),
                        noclobber: false,
                        path_verified: false,
                    };
                    exec_prepared_in_current_process(&shell, &prepared, ProcessGroupPlan::None);
                }
                let _ = sys::wait_pid(203, false);
            },
        );
    }

    #[test]
    fn spawn_child_join_group_and_fd_action_error() {
        run_trace(
            trace_entries![
                open(str("file.txt"), _, _) -> fd(10),
                fork() -> pid(123), child: [
                    setpgid(0, 500) -> 0,
                    dup2(fd(10), fd(1)) -> err(libc::EBADF),
                    write(fd(2), bytes(b"/bin/true: Bad file descriptor\n")) -> auto,
                ],
                close(fd(10)) -> 0,
                waitpid(123, _) -> status(1),
            ],
            || {
                let mut shell = test_shell();
                let prepared = PreparedProcess {
                    argv: vec![b"/bin/true".to_vec().into()].into(),
                    exec_path: b"/bin/true".to_vec().into(),
                    path_verified: true,
                    child_env: vec![].into(),
                    redirections: vec![ProcessRedirection {
                        kind: crate::syntax::RedirectionKind::Write,
                        fd: 1,
                        target: b"file.txt".to_vec().into(),
                        here_doc_body: None,
                    }],
                    noclobber: false,
                };
                let handle = crate::exec::process::spawn_prepared(
                    &mut shell,
                    &prepared,
                    ProcessGroupPlan::Join(500),
                )
                .unwrap();
                assert_eq!(handle.pid, 123);
                // Manually trigger wait to consume the trace
                let _ = sys::wait_pid(123, false);
            },
        );
    }
}
