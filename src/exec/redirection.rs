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

pub(super) trait RedirectionRef {
    fn fd(&self) -> i32;
    fn kind(&self) -> RedirectionKind;
    fn target(&self) -> &[u8];
    fn here_doc_body(&self) -> Option<&[u8]>;
}

#[derive(Debug)]
pub(super) enum ChildFdAction {
    DupRawFd {
        fd: i32,
        target_fd: i32,
        close_source: bool,
    },
    DupFd {
        source_fd: i32,
        target_fd: i32,
    },
    CloseFd {
        target_fd: i32,
    },
}

#[derive(Debug, Default)]
pub(super) struct PreparedRedirections {
    pub(super) actions: Vec<ChildFdAction>,
}

#[derive(Debug)]
pub(super) struct ShellRedirectionGuard {
    pub(super) saved: Vec<(i32, Option<i32>)>,
}

pub(super) fn close_parent_redirection_fds(prepared_redirections: &PreparedRedirections) {
    for action in &prepared_redirections.actions {
        if let ChildFdAction::DupRawFd {
            fd,
            close_source: true,
            ..
        } = action
        {
            let _ = sys::close_fd(*fd);
        }
    }
}

pub(super) fn prepare_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<PreparedRedirections, sys::SysError> {
    let mut prepared = PreparedRedirections::default();
    for redirection in redirections {
        match redirection.kind() {
            RedirectionKind::Read => {
                let fd = sys::open_file(redirection.target(), sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::Write | RedirectionKind::ClobberWrite => {
                let fd = if noclobber && redirection.kind() == RedirectionKind::Write {
                    open_for_write_noclobber(redirection.target())?
                } else {
                    let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC;
                    sys::open_file(redirection.target(), flags, 0o666)?
                };
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::Append => {
                let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
                let fd = sys::open_file(redirection.target(), flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::HereDoc => {
                let (read_fd, write_fd) = sys::create_pipe()?;
                let body = redirection.here_doc_body().unwrap_or(b"");
                sys::write_all_fd(write_fd, body)?;
                sys::close_fd(write_fd)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd: read_fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::ReadWrite => {
                let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
                let fd = sys::open_file(redirection.target(), flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::DupInput | RedirectionKind::DupOutput => {
                if redirection.target() == b"-" {
                    prepared.actions.push(ChildFdAction::CloseFd {
                        target_fd: redirection.fd(),
                    });
                } else {
                    let source_fd =
                        parse_i32_bytes(redirection.target()).expect("validated at expansion");
                    prepared.actions.push(ChildFdAction::DupFd {
                        source_fd,
                        target_fd: redirection.fd(),
                    });
                }
            }
        }
    }
    Ok(prepared)
}

pub(super) fn apply_child_fd_actions(actions: &[ChildFdAction]) -> sys::SysResult<()> {
    for action in actions {
        match action {
            ChildFdAction::DupRawFd { fd, target_fd, .. } => {
                sys::duplicate_fd(*fd, *target_fd)?;
            }
            ChildFdAction::DupFd {
                source_fd,
                target_fd,
            } => {
                sys::duplicate_fd(*source_fd, *target_fd)?;
            }
            ChildFdAction::CloseFd { target_fd } => {
                if let Err(error) = sys::close_fd(*target_fd) {
                    if !error.is_ebadf() {
                        return Err(error);
                    }
                }
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub(super) fn apply_child_setup(
    actions: &[ChildFdAction],
    process_group: ProcessGroupPlan,
) -> sys::SysResult<()> {
    apply_child_fd_actions(actions)?;
    match process_group {
        ProcessGroupPlan::None => {}
        ProcessGroupPlan::NewGroup => sys::set_process_group(0, 0)?,
        ProcessGroupPlan::Join(pgid) => sys::set_process_group(0, pgid)?,
    }
    Ok(())
}

impl Drop for ShellRedirectionGuard {
    fn drop(&mut self) {
        for (target_fd, saved_fd) in self.saved.iter().rev() {
            match saved_fd {
                Some(saved_fd) => {
                    let _ = sys::duplicate_fd(*saved_fd, *target_fd);
                    let _ = sys::close_fd(*saved_fd);
                }
                None => {
                    let _ = sys::close_fd(*target_fd);
                }
            }
        }
    }
}

pub(super) fn apply_shell_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<ShellRedirectionGuard, sys::SysError> {
    let mut guard = ShellRedirectionGuard { saved: Vec::new() };
    let mut saved = HashMap::new();

    for redirection in redirections {
        if let std::collections::hash_map::Entry::Vacant(entry) = saved.entry(redirection.fd()) {
            let original = match sys::duplicate_fd_to_new(redirection.fd()) {
                Ok(fd) => Some(fd),
                Err(error) if error.is_ebadf() => None,
                Err(error) => return Err(error),
            };
            entry.insert(original);
            guard.saved.push((redirection.fd(), original));
        }
        apply_shell_redirection(redirection, noclobber)?;
    }

    Ok(guard)
}

pub(super) fn open_for_write_noclobber(path: &[u8]) -> sys::SysResult<i32> {
    let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_EXCL | sys::O_CLOEXEC;
    match sys::open_file(path, flags, 0o666) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == Some(sys::EEXIST) => {
            if sys::is_regular_file(path) {
                Err(e)
            } else {
                sys::open_file(path, sys::O_WRONLY | sys::O_CLOEXEC, 0o666)
            }
        }
        Err(e) => Err(e),
    }
}

pub(super) fn apply_shell_redirection<R: RedirectionRef>(
    redirection: &R,
    noclobber: bool,
) -> sys::SysResult<()> {
    match redirection.kind() {
        RedirectionKind::Read => {
            let fd = sys::open_file(redirection.target(), sys::O_RDONLY | sys::O_CLOEXEC, 0)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Write | RedirectionKind::ClobberWrite => {
            let fd = if noclobber && redirection.kind() == RedirectionKind::Write {
                open_for_write_noclobber(redirection.target())?
            } else {
                let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_TRUNC | sys::O_CLOEXEC;
                sys::open_file(redirection.target(), flags, 0o666)?
            };
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Append => {
            let flags = sys::O_WRONLY | sys::O_CREAT | sys::O_APPEND | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::create_pipe()?;
            let body = redirection.here_doc_body().unwrap_or(b"");
            sys::write_all_fd(write_fd, body)?;
            sys::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd())?;
        }
        RedirectionKind::ReadWrite => {
            let flags = sys::O_RDWR | sys::O_CREAT | sys::O_CLOEXEC;
            let fd = sys::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::DupInput | RedirectionKind::DupOutput => {
            if redirection.target() == b"-" {
                close_shell_fd(redirection.fd())?;
            } else {
                let source_fd =
                    parse_i32_bytes(redirection.target()).expect("validated at expansion");
                sys::duplicate_fd(source_fd, redirection.fd())?;
            }
        }
    }
    Ok(())
}

pub(super) fn replace_shell_fd(fd: i32, target_fd: i32) -> sys::SysResult<()> {
    if fd == target_fd {
        return Ok(());
    }
    sys::duplicate_fd(fd, target_fd)?;
    sys::close_fd(fd)?;
    Ok(())
}

pub(super) fn close_shell_fd(target_fd: i32) -> sys::SysResult<()> {
    if let Err(error) = sys::close_fd(target_fd) {
        if !error.is_ebadf() {
            return Err(error);
        }
    }
    Ok(())
}

pub(super) fn default_fd_for_redirection(kind: RedirectionKind) -> i32 {
    match kind {
        RedirectionKind::Read
        | RedirectionKind::HereDoc
        | RedirectionKind::ReadWrite
        | RedirectionKind::DupInput => 0,
        RedirectionKind::Write
        | RedirectionKind::ClobberWrite
        | RedirectionKind::Append
        | RedirectionKind::DupOutput => 1,
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::test_support::*;
    use crate::shell::Shell;
    use crate::syntax::{Assignment, HereDoc, Redirection, Word};

    #[test]
    fn apply_child_fd_actions_applies_dup_close() {
        run_trace(
            vec![
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(90)],
                    TraceResult::Int(90),
                ),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(90), ArgMatcher::Fd(91)],
                    TraceResult::Int(91),
                ),
                t("close", vec![ArgMatcher::Fd(91)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(123_456)], TraceResult::Int(0)),
            ],
            || {
                apply_child_fd_actions(&[
                    ChildFdAction::DupRawFd {
                        fd: 10,
                        target_fd: 90,
                        close_source: true,
                    },
                    ChildFdAction::DupFd {
                        source_fd: 90,
                        target_fd: 91,
                    },
                    ChildFdAction::CloseFd { target_fd: 91 },
                    ChildFdAction::CloseFd { target_fd: 123_456 },
                ])
                .expect("apply child actions");
            },
        );
    }

    #[test]
    fn prepare_redirections_produces_correct_fd_actions() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/rw.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(100),
            )],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::DupOutput,
                            target: b"-",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::ReadWrite,
                            target: b"/tmp/rw.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                    ],
                    false,
                )
                .expect("prepare");
                assert_eq!(prepared.actions.len(), 2);
            },
        );
    }

    #[test]
    fn heredoc_expansion_error_paths() {
        run_trace(
            vec![
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: missing here-document body\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: redirection target must be a file descriptor or '-'\n"
                                .to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(b"meiksh: missing here-document body\n".to_vec()),
                    ],
                    TraceResult::Auto,
                ),
                t(
                    "write",
                    vec![
                        ArgMatcher::Fd(sys::STDERR_FILENO),
                        ArgMatcher::Bytes(
                            b"meiksh: redirection target must be a file descriptor or '-'\n"
                                .to_vec(),
                        ),
                    ],
                    TraceResult::Auto,
                ),
            ],
            || {
                let arena = ByteArena::new();
                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"cat".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: None,
                            kind: RedirectionKind::HereDoc,
                            target: Word {
                                raw: b"EOF".to_vec().into(),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
                    &arena,
                )
                .expect_err("expected missing here-document body");
                assert_eq!(error.exit_status(), 2);

                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"echo".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: Some(1),
                            kind: RedirectionKind::DupOutput,
                            target: Word {
                                raw: b"bad".to_vec().into(),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
                    &arena,
                )
                .expect_err("bad dup target");
                assert_eq!(error.exit_status(), 1);

                let mut shell = test_shell();
                let expanded = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello $USER".to_vec().into(),
                            expand: true,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("expand heredoc redirection");
                assert_eq!(expanded[0].target, b"EOF");
                assert_eq!(expanded[0].here_doc_body, Some(b"hello " as &[u8]));

                let mut shell = test_shell();
                let literal = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello $USER".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("literal heredoc redirection");
                assert_eq!(literal[0].here_doc_body, Some(b"hello $USER" as &[u8]));

                let mut shell = test_shell();
                let error = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    }],
                    &arena,
                )
                .expect_err("missing expanded heredoc body");
                assert_eq!(error.exit_status(), 2);

                let mut shell = test_shell();
                let stripped = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF".to_vec().into(),
                            body: b"hello\nworld\n".to_vec().into(),
                            expand: false,
                            strip_tabs: true,
                            body_line: 0,
                        }),
                    }],
                    &arena,
                )
                .expect("strip tabs expand");
                assert_eq!(stripped[0].here_doc_body, Some(b"hello\nworld\n" as &[u8]));

                let mut shell = test_shell();
                let dup_err = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: Some(2),
                        kind: RedirectionKind::DupOutput,
                        target: Word {
                            raw: b"abc".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    }],
                    &arena,
                )
                .expect_err("bad dup target in standalone");
                assert_eq!(dup_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn prepare_redirections_creates_heredoc_pipe() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
            ],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF",
                        here_doc_body: Some(b"body\n"),
                        line: 0,
                    }],
                    false,
                )
                .expect("prepare heredoc");
                assert_eq!(prepared.actions.len(), 1);
            },
        );
    }

    #[test]
    fn prepare_redirections_heredoc_write_error() {
        run_trace(
            vec![
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"body\n".to_vec())],
                    TraceResult::Err(sys::EIO),
                ),
            ],
            || {
                let err = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF",
                        here_doc_body: Some(b"body\n"),
                        line: 0,
                    }],
                    false,
                )
                .expect_err("heredoc write should fail");
                assert!(!err.strerror().is_empty());
            },
        );
    }

    #[test]
    fn execute_nested_program_sets_up_heredoc_fd() {
        run_trace(
            vec![
                t(
                    "fcntl",
                    vec![
                        ArgMatcher::Fd(0),
                        ArgMatcher::Int(1030),
                        ArgMatcher::Int(10),
                    ],
                    TraceResult::Err(sys::EBADF),
                ),
                t("pipe", vec![], TraceResult::Fds(10, 11)),
                t(
                    "write",
                    vec![ArgMatcher::Fd(11), ArgMatcher::Bytes(b"hello\n".to_vec())],
                    TraceResult::Auto,
                ),
                t("close", vec![ArgMatcher::Fd(11)], TraceResult::Int(0)),
                t(
                    "dup2",
                    vec![ArgMatcher::Fd(10), ArgMatcher::Fd(0)],
                    TraceResult::Int(0),
                ),
                t("close", vec![ArgMatcher::Fd(10)], TraceResult::Int(0)),
                t("close", vec![ArgMatcher::Fd(0)], TraceResult::Int(0)),
            ],
            || {
                let heredoc_program = parse_test(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
                let mut shell = test_shell();
                let status = execute_nested_program(&mut shell, &heredoc_program)
                    .expect("execute heredoc nested");
                assert_eq!(status, 0);
            },
        );
    }

    #[test]
    fn replace_shell_fd_same_fd() {
        assert_no_syscalls(|| {
            replace_shell_fd(42, 42).expect("same-fd replacement");
        });
    }

    #[test]
    fn open_for_write_noclobber_new_file_succeeds() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/new/file.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Int(10),
            )],
            || {
                let fd = open_for_write_noclobber(b"/new/file.txt").expect("new file");
                assert_eq!(fd, 10);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_non_regular_reopens() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EEXIST),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/dev/null".into()), ArgMatcher::Any],
                    TraceResult::StatFifo,
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/dev/null".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Int(11),
                ),
            ],
            || {
                let fd = open_for_write_noclobber(b"/dev/null").expect("fifo reopen");
                assert_eq!(fd, 11);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_other_error() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/noperm".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Err(libc::EACCES),
            )],
            || {
                let err = open_for_write_noclobber(b"/noperm").expect_err("eacces");
                assert_eq!(err.errno(), Some(libc::EACCES));
            },
        );
    }

    #[test]
    fn default_fd_for_redirection_covers_all_kinds() {
        assert_no_syscalls(|| {
            assert_eq!(default_fd_for_redirection(RedirectionKind::Read), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::HereDoc), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::ReadWrite), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::DupInput), 0);
            assert_eq!(default_fd_for_redirection(RedirectionKind::Write), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::ClobberWrite), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::Append), 1);
            assert_eq!(default_fd_for_redirection(RedirectionKind::DupOutput), 1);
        });
    }

    #[test]
    fn close_parent_redirection_fds_only_closes_marked() {
        run_trace(
            vec![t("close", vec![ArgMatcher::Fd(77)], TraceResult::Int(0))],
            || {
                let prepared = PreparedRedirections {
                    actions: vec![
                        ChildFdAction::DupRawFd {
                            fd: 77,
                            target_fd: 1,
                            close_source: true,
                        },
                        ChildFdAction::DupRawFd {
                            fd: 88,
                            target_fd: 2,
                            close_source: false,
                        },
                        ChildFdAction::DupFd {
                            source_fd: 3,
                            target_fd: 4,
                        },
                        ChildFdAction::CloseFd { target_fd: 5 },
                    ],
                };
                close_parent_redirection_fds(&prepared);
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_ebadf_ignored() {
        run_trace(
            vec![t(
                "close",
                vec![ArgMatcher::Fd(999)],
                TraceResult::Err(sys::EBADF),
            )],
            || {
                apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect("ebadf on close should be ignored");
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_non_ebadf_propagates() {
        run_trace(
            vec![t(
                "close",
                vec![ArgMatcher::Fd(999)],
                TraceResult::Err(sys::EIO),
            )],
            || {
                let err = apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect_err("non-ebadf close should fail");
                assert_eq!(err.errno(), Some(sys::EIO));
            },
        );
    }

    #[test]
    fn prepare_redirections_read_write_append_noclobber() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/r.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(10),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/w.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(11),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/a.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(12),
                ),
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/tmp/cw.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Fd(13),
                ),
            ],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::Read,
                            target: b"/tmp/r.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::Write,
                            target: b"/tmp/w.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 2,
                            kind: RedirectionKind::Append,
                            target: b"/tmp/a.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::ClobberWrite,
                            target: b"/tmp/cw.txt",
                            here_doc_body: None,
                            line: 0,
                        },
                    ],
                    false,
                )
                .expect("prepare");
                assert_eq!(prepared.actions.len(), 4);
            },
        );
    }

    #[test]
    fn prepare_redirections_dup_input_and_close() {
        assert_no_syscalls(|| {
            let prepared = prepare_redirections(
                &[
                    ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::DupInput,
                        target: b"3",
                        here_doc_body: None,
                        line: 0,
                    },
                    ExpandedRedirection {
                        fd: 5,
                        kind: RedirectionKind::DupOutput,
                        target: b"-",
                        here_doc_body: None,
                        line: 0,
                    },
                ],
                false,
            )
            .expect("prepare dup");
            assert_eq!(prepared.actions.len(), 2);
            match &prepared.actions[0] {
                ChildFdAction::DupFd {
                    source_fd,
                    target_fd,
                } => {
                    assert_eq!(*source_fd, 3);
                    assert_eq!(*target_fd, 0);
                }
                other => panic!("expected DupFd, got {other:?}"),
            }
            match &prepared.actions[1] {
                ChildFdAction::CloseFd { target_fd } => {
                    assert_eq!(*target_fd, 5);
                }
                other => panic!("expected CloseFd, got {other:?}"),
            }
        });
    }

    #[test]
    fn prepare_redirections_noclobber_uses_excl() {
        run_trace(
            vec![t(
                "open",
                vec![
                    ArgMatcher::Str("/tmp/nc.txt".into()),
                    ArgMatcher::Any,
                    ArgMatcher::Any,
                ],
                TraceResult::Fd(20),
            )],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 1,
                        kind: RedirectionKind::Write,
                        target: b"/tmp/nc.txt",
                        here_doc_body: None,
                        line: 0,
                    }],
                    true,
                )
                .expect("prepare noclobber");
                assert_eq!(prepared.actions.len(), 1);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_regular_file_fails() {
        run_trace(
            vec![
                t(
                    "open",
                    vec![
                        ArgMatcher::Str("/exists.txt".into()),
                        ArgMatcher::Any,
                        ArgMatcher::Any,
                    ],
                    TraceResult::Err(sys::EEXIST),
                ),
                t(
                    "stat",
                    vec![ArgMatcher::Str("/exists.txt".into()), ArgMatcher::Any],
                    TraceResult::StatFile(0o644),
                ),
            ],
            || {
                let err = open_for_write_noclobber(b"/exists.txt").expect_err("should fail");
                assert_eq!(err.errno(), Some(sys::EEXIST));
            },
        );
    }
}
