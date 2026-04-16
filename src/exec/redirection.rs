use crate::syntax::ast::RedirectionKind;
use crate::sys;

#[cfg(test)]
use super::and_or::ProcessGroupPlan;
use super::simple::parse_i32_bytes;

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
            let _ = sys::fd_io::close_fd(*fd);
        }
    }
}

pub(super) fn prepare_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<PreparedRedirections, sys::error::SysError> {
    let mut prepared = PreparedRedirections::default();
    for redirection in redirections {
        match redirection.kind() {
            RedirectionKind::Read => {
                let fd = sys::fs::open_file(
                    redirection.target(),
                    sys::constants::O_RDONLY | sys::constants::O_CLOEXEC,
                    0,
                )?;
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
                    let flags = sys::constants::O_WRONLY
                        | sys::constants::O_CREAT
                        | sys::constants::O_TRUNC
                        | sys::constants::O_CLOEXEC;
                    sys::fs::open_file(redirection.target(), flags, 0o666)?
                };
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::Append => {
                let flags = sys::constants::O_WRONLY
                    | sys::constants::O_CREAT
                    | sys::constants::O_APPEND
                    | sys::constants::O_CLOEXEC;
                let fd = sys::fs::open_file(redirection.target(), flags, 0o666)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::HereDoc => {
                let (read_fd, write_fd) = sys::fd_io::create_pipe()?;
                let body = redirection.here_doc_body().unwrap_or(b"");
                sys::fd_io::write_all_fd(write_fd, body)?;
                sys::fd_io::close_fd(write_fd)?;
                prepared.actions.push(ChildFdAction::DupRawFd {
                    fd: read_fd,
                    target_fd: redirection.fd(),
                    close_source: true,
                });
            }
            RedirectionKind::ReadWrite => {
                let flags =
                    sys::constants::O_RDWR | sys::constants::O_CREAT | sys::constants::O_CLOEXEC;
                let fd = sys::fs::open_file(redirection.target(), flags, 0o666)?;
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

pub(super) fn apply_child_fd_actions(actions: &[ChildFdAction]) -> sys::error::SysResult<()> {
    for action in actions {
        match action {
            ChildFdAction::DupRawFd { fd, target_fd, .. } => {
                sys::fd_io::duplicate_fd(*fd, *target_fd)?;
            }
            ChildFdAction::DupFd {
                source_fd,
                target_fd,
            } => {
                sys::fd_io::duplicate_fd(*source_fd, *target_fd)?;
            }
            ChildFdAction::CloseFd { target_fd } => {
                if let Err(error) = sys::fd_io::close_fd(*target_fd) {
                    if !error.is_ebadf() {
                        return Err(error);
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn apply_child_setup(
    actions: &[ChildFdAction],
    process_group: ProcessGroupPlan,
) -> sys::error::SysResult<()> {
    apply_child_fd_actions(actions)?;
    match process_group {
        ProcessGroupPlan::None => {}
        ProcessGroupPlan::NewGroup => sys::tty::set_process_group(0, 0)?,
        ProcessGroupPlan::Join(pgid) => sys::tty::set_process_group(0, pgid)?,
    }
    Ok(())
}

impl Drop for ShellRedirectionGuard {
    fn drop(&mut self) {
        for (target_fd, saved_fd) in self.saved.iter().rev() {
            match saved_fd {
                Some(saved_fd) => {
                    let _ = sys::fd_io::duplicate_fd(*saved_fd, *target_fd);
                    let _ = sys::fd_io::close_fd(*saved_fd);
                }
                None => {
                    let _ = sys::fd_io::close_fd(*target_fd);
                }
            }
        }
    }
}

pub(super) fn apply_shell_redirections<R: RedirectionRef>(
    redirections: &[R],
    noclobber: bool,
) -> Result<ShellRedirectionGuard, sys::error::SysError> {
    let mut guard = ShellRedirectionGuard { saved: Vec::new() };

    for redirection in redirections {
        if !guard.saved.iter().any(|(fd, _)| *fd == redirection.fd()) {
            let original = match sys::fd_io::duplicate_fd_to_new(redirection.fd()) {
                Ok(fd) => Some(fd),
                Err(error) if error.is_ebadf() => None,
                Err(error) => return Err(error),
            };
            guard.saved.push((redirection.fd(), original));
        }
        apply_shell_redirection(redirection, noclobber)?;
    }

    Ok(guard)
}

pub(super) fn open_for_write_noclobber(path: &[u8]) -> sys::error::SysResult<i32> {
    let flags = sys::constants::O_WRONLY
        | sys::constants::O_CREAT
        | sys::constants::O_EXCL
        | sys::constants::O_CLOEXEC;
    match sys::fs::open_file(path, flags, 0o666) {
        Ok(fd) => Ok(fd),
        Err(e) if e.errno() == Some(sys::constants::EEXIST) => {
            if sys::fs::is_regular_file(path) {
                Err(e)
            } else {
                sys::fs::open_file(
                    path,
                    sys::constants::O_WRONLY | sys::constants::O_CLOEXEC,
                    0o666,
                )
            }
        }
        Err(e) => Err(e),
    }
}

pub(super) fn apply_shell_redirection<R: RedirectionRef>(
    redirection: &R,
    noclobber: bool,
) -> sys::error::SysResult<()> {
    match redirection.kind() {
        RedirectionKind::Read => {
            let fd = sys::fs::open_file(
                redirection.target(),
                sys::constants::O_RDONLY | sys::constants::O_CLOEXEC,
                0,
            )?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Write | RedirectionKind::ClobberWrite => {
            let fd = if noclobber && redirection.kind() == RedirectionKind::Write {
                open_for_write_noclobber(redirection.target())?
            } else {
                let flags = sys::constants::O_WRONLY
                    | sys::constants::O_CREAT
                    | sys::constants::O_TRUNC
                    | sys::constants::O_CLOEXEC;
                sys::fs::open_file(redirection.target(), flags, 0o666)?
            };
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::Append => {
            let flags = sys::constants::O_WRONLY
                | sys::constants::O_CREAT
                | sys::constants::O_APPEND
                | sys::constants::O_CLOEXEC;
            let fd = sys::fs::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::HereDoc => {
            let (read_fd, write_fd) = sys::fd_io::create_pipe()?;
            let body = redirection.here_doc_body().unwrap_or(b"");
            sys::fd_io::write_all_fd(write_fd, body)?;
            sys::fd_io::close_fd(write_fd)?;
            replace_shell_fd(read_fd, redirection.fd())?;
        }
        RedirectionKind::ReadWrite => {
            let flags =
                sys::constants::O_RDWR | sys::constants::O_CREAT | sys::constants::O_CLOEXEC;
            let fd = sys::fs::open_file(redirection.target(), flags, 0o666)?;
            replace_shell_fd(fd, redirection.fd())?;
        }
        RedirectionKind::DupInput | RedirectionKind::DupOutput => {
            if redirection.target() == b"-" {
                close_shell_fd(redirection.fd())?;
            } else {
                let source_fd =
                    parse_i32_bytes(redirection.target()).expect("validated at expansion");
                sys::fd_io::duplicate_fd(source_fd, redirection.fd())?;
            }
        }
    }
    Ok(())
}

pub(super) fn replace_shell_fd(fd: i32, target_fd: i32) -> sys::error::SysResult<()> {
    if fd == target_fd {
        return Ok(());
    }
    sys::fd_io::duplicate_fd(fd, target_fd)?;
    sys::fd_io::close_fd(fd)?;
    Ok(())
}

pub(super) fn close_shell_fd(target_fd: i32) -> sys::error::SysResult<()> {
    if let Err(error) = sys::fd_io::close_fd(target_fd) {
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
    use crate::exec::command::execute_nested_program;
    use crate::exec::process::ExpandedRedirection;
    use crate::exec::simple::{expand_redirections, expand_simple};
    use crate::exec::test_support::{parse_test, test_shell};
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, SimpleCommand, Word};
    use crate::sys::test_support::{assert_no_syscalls, run_trace};
    use crate::trace_entries;

    #[test]
    fn apply_child_fd_actions_applies_dup_close() {
        run_trace(
            trace_entries![
                dup2(fd(10), fd(90)) -> 90,
                dup2(fd(90), fd(91)) -> 91,
                close(fd(91)) -> 0,
                close(fd(123_456)) -> 0,
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
            trace_entries![
                open(str("/tmp/rw.txt"), _, _) -> fd(100),
            ],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::DupOutput,
                            target: b"-".to_vec(),
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::ReadWrite,
                            target: b"/tmp/rw.txt".to_vec(),
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
            trace_entries![
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: missing here-document body\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: redirection target must be a file descriptor or '-'\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: missing here-document body\n")) -> auto,
                write(fd(sys::constants::STDERR_FILENO), bytes(b"meiksh: redirection target must be a file descriptor or '-'\n")) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"cat".to_vec().into(),
                            parts: Box::new([]),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: None,
                            kind: RedirectionKind::HereDoc,
                            target: Word {
                                raw: b"EOF".to_vec().into(),
                                parts: Box::new([]),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
                )
                .expect_err("expected missing here-document body");
                assert_eq!(error.exit_status(), 2);

                let mut shell = test_shell();
                let error = expand_simple(
                    &mut shell,
                    &SimpleCommand {
                        words: vec![Word {
                            raw: b"echo".to_vec().into(),
                            parts: Box::new([]),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        redirections: vec![Redirection {
                            fd: Some(1),
                            kind: RedirectionKind::DupOutput,
                            target: Word {
                                raw: b"bad".to_vec().into(),
                                parts: Box::new([]),
                                line: 0,
                            },
                            here_doc: None,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    },
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
                            parts: Box::new([]),
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
                )
                .expect("expand heredoc redirection");
                assert_eq!(expanded[0].target, b"EOF");
                assert_eq!(
                    expanded[0].here_doc_body.as_deref(),
                    Some(b"hello " as &[u8])
                );

                let mut shell = test_shell();
                let literal = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            parts: Box::new([]),
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
                )
                .expect("literal heredoc redirection");
                assert_eq!(
                    literal[0].here_doc_body.as_deref(),
                    Some(b"hello $USER" as &[u8])
                );

                let mut shell = test_shell();
                let error = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF".to_vec().into(),
                            parts: Box::new([]),
                            line: 0,
                        },
                        here_doc: None,
                    }],
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
                            parts: Box::new([]),
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
                )
                .expect("strip tabs expand");
                assert_eq!(
                    stripped[0].here_doc_body.as_deref(),
                    Some(b"hello\nworld\n" as &[u8])
                );

                let mut shell = test_shell();
                let dup_err = expand_redirections(
                    &mut shell,
                    &[Redirection {
                        fd: Some(2),
                        kind: RedirectionKind::DupOutput,
                        target: Word {
                            raw: b"abc".to_vec().into(),
                            parts: Box::new([]),
                            line: 0,
                        },
                        here_doc: None,
                    }],
                )
                .expect_err("bad dup target in standalone");
                assert_eq!(dup_err.exit_status(), 1);
            },
        );
    }

    #[test]
    fn prepare_redirections_creates_heredoc_pipe() {
        run_trace(
            trace_entries![
                pipe() -> fds(10, 11),
                write(fd(11), bytes(b"body\n")) -> auto,
                close(fd(11)) -> 0,
            ],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF".to_vec(),
                        here_doc_body: Some(b"body\n".to_vec()),
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
            trace_entries![
                pipe() -> fds(10, 11),
                write(fd(11), bytes(b"body\n")) -> err(sys::constants::EIO),
            ],
            || {
                let err = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::HereDoc,
                        target: b"EOF".to_vec(),
                        here_doc_body: Some(b"body\n".to_vec()),
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
            trace_entries![
                fcntl(fd(0), int(sys::constants::F_DUPFD_CLOEXEC as i64), int(10)) -> err(sys::constants::EBADF),
                pipe() -> fds(10, 11),
                write(fd(11), bytes(b"hello\n")) -> auto,
                close(fd(11)) -> 0,
                dup2(fd(10), fd(0)) -> 0,
                close(fd(10)) -> 0,
                close(fd(0)) -> 0,
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
            trace_entries![
                open(str("/new/file.txt"), _, _) -> 10,
            ],
            || {
                let fd = open_for_write_noclobber(b"/new/file.txt").expect("new file");
                assert_eq!(fd, 10);
            },
        );
    }

    #[test]
    fn open_for_write_noclobber_non_regular_reopens() {
        run_trace(
            trace_entries![
                open(str("/dev/null"), _, _) -> err(sys::constants::EEXIST),
                stat(str("/dev/null"), _) -> stat_fifo,
                open(str("/dev/null"), _, _) -> 11,
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
            trace_entries![
                open(str("/noperm"), _, _) -> err(libc::EACCES),
            ],
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
            trace_entries![
                close(fd(77)) -> 0,
            ],
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
            trace_entries![
                close(fd(999)) -> err(sys::constants::EBADF),
            ],
            || {
                apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect("ebadf on close should be ignored");
            },
        );
    }

    #[test]
    fn apply_child_fd_actions_close_non_ebadf_propagates() {
        run_trace(
            trace_entries![
                close(fd(999)) -> err(sys::constants::EIO),
            ],
            || {
                let err = apply_child_fd_actions(&[ChildFdAction::CloseFd { target_fd: 999 }])
                    .expect_err("non-ebadf close should fail");
                assert_eq!(err.errno(), Some(sys::constants::EIO));
            },
        );
    }

    #[test]
    fn prepare_redirections_read_write_append_noclobber() {
        run_trace(
            trace_entries![
                open(str("/tmp/r.txt"), _, _) -> fd(10),
                open(str("/tmp/w.txt"), _, _) -> fd(11),
                open(str("/tmp/a.txt"), _, _) -> fd(12),
                open(str("/tmp/cw.txt"), _, _) -> fd(13),
            ],
            || {
                let prepared = prepare_redirections(
                    &[
                        ExpandedRedirection {
                            fd: 0,
                            kind: RedirectionKind::Read,
                            target: b"/tmp/r.txt".to_vec(),
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::Write,
                            target: b"/tmp/w.txt".to_vec(),
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 2,
                            kind: RedirectionKind::Append,
                            target: b"/tmp/a.txt".to_vec(),
                            here_doc_body: None,
                            line: 0,
                        },
                        ExpandedRedirection {
                            fd: 1,
                            kind: RedirectionKind::ClobberWrite,
                            target: b"/tmp/cw.txt".to_vec(),
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
                        target: b"3".to_vec(),
                        here_doc_body: None,
                        line: 0,
                    },
                    ExpandedRedirection {
                        fd: 5,
                        kind: RedirectionKind::DupOutput,
                        target: b"-".to_vec(),
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
            trace_entries![
                open(str("/tmp/nc.txt"), _, _) -> fd(20),
            ],
            || {
                let prepared = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 1,
                        kind: RedirectionKind::Write,
                        target: b"/tmp/nc.txt".to_vec(),
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
            trace_entries![
                open(str("/exists.txt"), _, _) -> err(sys::constants::EEXIST),
                stat(str("/exists.txt"), _) -> stat_file(0o644),
            ],
            || {
                let err = open_for_write_noclobber(b"/exists.txt").expect_err("should fail");
                assert_eq!(err.errno(), Some(sys::constants::EEXIST));
            },
        );
    }
    #[test]
    fn apply_shell_redirections_multiple_same_fd() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> int(10), // save
                open(str("a"), _, _) -> fd(11),
                dup2(fd(11), fd(1)) -> fd(1),
                close(fd(11)) -> 0,
                // second save skips fcntl
                open(str("b"), _, _) -> fd(12),
                dup2(fd(12), fd(1)) -> fd(1),
                close(fd(12)) -> 0,
                // restore
                dup2(fd(10), fd(1)) -> fd(1),
                close(fd(10)) -> 0,
            ],
            || {
                let expanded1 = ExpandedRedirection {
                    kind: RedirectionKind::Write,
                    fd: 1,
                    target: b"a".to_vec(),
                    here_doc_body: None,
                    line: 1,
                };
                let expanded2 = ExpandedRedirection {
                    kind: RedirectionKind::Write,
                    fd: 1,
                    target: b"b".to_vec(),
                    here_doc_body: None,
                    line: 1,
                };
                let _guard = apply_shell_redirections(&[expanded1, expanded2], false).unwrap();
            },
        );
    }

    #[test]
    fn apply_shell_redirection_read() {
        run_trace(
            trace_entries![
                open(str("a"), _, _) -> fd(10),
                dup2(fd(10), fd(0)) -> fd(0),
                close(fd(10)) -> 0,
            ],
            || {
                let expanded = ExpandedRedirection {
                    kind: RedirectionKind::Read,
                    fd: 0,
                    target: b"a".to_vec(),
                    here_doc_body: None,
                    line: 1,
                };
                apply_shell_redirection(&expanded, false).unwrap();
            },
        );
    }
    #[test]
    fn apply_shell_redirection_readwrite() {
        run_trace(
            trace_entries![
                open(str("file.txt"), _, _) -> fd(10),
                dup2(fd(10), fd(3)) -> fd(3),
                close(fd(10)) -> 0,
            ],
            || {
                let expanded = ExpandedRedirection {
                    kind: RedirectionKind::ReadWrite,
                    fd: 3,
                    target: b"file.txt".to_vec(),
                    here_doc_body: None,
                    line: 1,
                };
                apply_shell_redirection(&expanded, false).unwrap();
            },
        );
    }

    #[test]
    fn apply_shell_redirections_dup_error() {
        run_trace(
            trace_entries![
                fcntl(int(1), _, _) -> err(libc::EMFILE),
            ],
            || {
                let expanded = ExpandedRedirection {
                    kind: RedirectionKind::Write,
                    fd: 1,
                    target: b"file.txt".to_vec(),
                    here_doc_body: None,
                    line: 1,
                };
                let err = apply_shell_redirections(&[expanded], false).unwrap_err();
                assert_eq!(err.errno(), Some(libc::EMFILE));
            },
        );
    }

    #[test]
    fn close_shell_fd_other_error() {
        run_trace(
            trace_entries![
                close(fd(3)) -> err(libc::EIO),
            ],
            || {
                let err = close_shell_fd(3).unwrap_err();
                assert_eq!(err.errno(), Some(libc::EIO));
            },
        );
    }
    #[test]
    fn apply_child_setup_test() {
        run_trace(
            trace_entries![
                setpgid(0, 0) -> 0,
            ],
            || {
                let plan = ProcessGroupPlan::NewGroup;
                apply_child_setup(&[], plan).unwrap();
            },
        );
        run_trace(
            trace_entries![
                setpgid(0, 100) -> 0,
            ],
            || {
                let plan = ProcessGroupPlan::Join(100);
                apply_child_setup(&[], plan).unwrap();
            },
        );
        assert_no_syscalls(|| {
            let plan = ProcessGroupPlan::None;
            apply_child_setup(&[], plan).unwrap();
        });
    }

    #[test]
    fn close_shell_fd_ebadf_ok() {
        run_trace(
            trace_entries![
                close(fd(3)) -> err(libc::EBADF),
            ],
            || {
                close_shell_fd(3).unwrap();
            },
        );
    }

    #[test]
    fn prepare_redirections_read_open_error() {
        run_trace(
            trace_entries![
                open(str("/no/such/file"), _, _) -> err(libc::ENOENT),
            ],
            || {
                let err = prepare_redirections(
                    &[ExpandedRedirection {
                        fd: 0,
                        kind: RedirectionKind::Read,
                        target: b"/no/such/file".to_vec(),
                        here_doc_body: None,
                        line: 0,
                    }],
                    false,
                )
                .expect_err("read open should fail");
                assert_eq!(err.errno(), Some(libc::ENOENT));
            },
        );
    }
}
