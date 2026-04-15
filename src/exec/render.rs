use crate::expand::glob;
use crate::syntax::ast::{
    AndOr, CaseCommand, Command, ForCommand, FunctionDef, HereDoc, IfCommand, ListItem, LogicalOp,
    LoopCommand, LoopKind, Pipeline, Program, RedirectionKind, SimpleCommand, TimedMode,
};

pub(super) fn case_pattern_matches(text: &[u8], pattern: &[u8]) -> bool {
    glob::pattern_matches(text, pattern)
}

#[cfg(test)]
pub(super) fn render_program(program: &Program) -> Vec<u8> {
    let mut buf = Vec::new();
    render_program_into(program, &mut buf);
    buf
}

pub(super) fn render_program_into(program: &Program, buf: &mut Vec<u8>) {
    for (index, item) in program.items.iter().enumerate() {
        if index > 0 {
            buf.push(b'\n');
        }
        render_list_item_into(item, buf);
    }
}

#[cfg(test)]
pub(super) fn render_list_item(item: &ListItem) -> Vec<u8> {
    let mut buf = Vec::new();
    render_list_item_into(item, &mut buf);
    buf
}

pub(super) fn render_list_item_into(item: &ListItem, buf: &mut Vec<u8>) {
    render_and_or_into(&item.and_or, buf);
    if item.asynchronous {
        buf.extend_from_slice(b" &");
    }
}

pub(super) fn render_and_or(and_or: &AndOr) -> Vec<u8> {
    let mut buf = Vec::new();
    render_and_or_into(and_or, &mut buf);
    buf
}

pub(super) fn render_and_or_into(and_or: &AndOr, buf: &mut Vec<u8>) {
    render_pipeline_into(&and_or.first, buf);
    for (op, pipeline) in &and_or.rest {
        match op {
            LogicalOp::And => buf.extend_from_slice(b" && "),
            LogicalOp::Or => buf.extend_from_slice(b" || "),
        }
        render_pipeline_into(pipeline, buf);
    }
}

pub(super) fn render_command(command: &Command) -> Vec<u8> {
    let mut buf = Vec::new();
    render_command_into(command, &mut buf);
    buf
}

pub(super) fn render_command_into(command: &Command, buf: &mut Vec<u8>) {
    match command {
        Command::Simple(simple) => render_simple_into(simple, buf),
        Command::Subshell(program) => {
            buf.push(b'(');
            render_program_into(program, buf);
            buf.push(b')');
        }
        Command::Group(program) => {
            buf.extend_from_slice(b"{ ");
            render_program_into(program, buf);
            buf.extend_from_slice(b"; }");
        }
        Command::FunctionDef(function) => render_function_into(function, buf),
        Command::If(if_command) => render_if_into(if_command, buf),
        Command::Loop(loop_command) => render_loop_into(loop_command, buf),
        Command::For(for_command) => render_for_into(for_command, buf),
        Command::Case(case_command) => render_case_into(case_command, buf),
        Command::Redirected(command, redirections) => {
            render_redirected_command_into(command, redirections, buf);
        }
    }
}

pub(super) fn render_pipeline(pipeline: &Pipeline) -> Vec<u8> {
    let mut buf = Vec::new();
    render_pipeline_into(pipeline, &mut buf);
    buf
}

pub(super) fn render_pipeline_into(pipeline: &Pipeline, buf: &mut Vec<u8>) {
    if pipeline.negated {
        buf.extend_from_slice(b"! ");
    }
    for (i, command) in pipeline.commands.iter().enumerate() {
        if i > 0 {
            buf.extend_from_slice(b" | ");
        }
        render_command_into(command, buf);
    }
}

#[cfg(test)]
pub(super) fn render_function(function: &FunctionDef) -> Vec<u8> {
    let mut buf = Vec::new();
    render_function_into(function, &mut buf);
    buf
}

pub(super) fn render_function_into(function: &FunctionDef, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&function.name);
    buf.extend_from_slice(b"() ");
    render_pipeline_into(
        &Pipeline {
            negated: false,
            timed: TimedMode::Off,
            commands: vec![(*function.body).clone()].into_boxed_slice(),
        },
        buf,
    );
}

#[cfg(test)]
pub(super) fn render_if(if_command: &IfCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_if_into(if_command, &mut buf);
    buf
}

pub(super) fn render_if_into(if_command: &IfCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"if ");
    render_program_into(&if_command.condition, buf);
    buf.extend_from_slice(b"\nthen\n");
    render_program_into(&if_command.then_branch, buf);
    for branch in &if_command.elif_branches {
        buf.extend_from_slice(b"\nelif ");
        render_program_into(&branch.condition, buf);
        buf.extend_from_slice(b"\nthen\n");
        render_program_into(&branch.body, buf);
    }
    if let Some(else_branch) = &if_command.else_branch {
        buf.extend_from_slice(b"\nelse\n");
        render_program_into(else_branch, buf);
    }
    buf.extend_from_slice(b"\nfi");
}

#[cfg(test)]
pub(super) fn render_loop(loop_command: &LoopCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_loop_into(loop_command, &mut buf);
    buf
}

pub(super) fn render_loop_into(loop_command: &LoopCommand, buf: &mut Vec<u8>) {
    let keyword = match loop_command.kind {
        LoopKind::While => b"while" as &[u8],
        LoopKind::Until => b"until" as &[u8],
    };
    buf.extend_from_slice(keyword);
    buf.push(b' ');
    render_program_into(&loop_command.condition, buf);
    buf.extend_from_slice(b"\ndo\n");
    render_program_into(&loop_command.body, buf);
    buf.extend_from_slice(b"\ndone");
}

#[cfg(test)]
pub(super) fn render_for(for_command: &ForCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_for_into(for_command, &mut buf);
    buf
}

pub(super) fn render_for_into(for_command: &ForCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"for ");
    buf.extend_from_slice(&for_command.name);
    if let Some(items) = &for_command.items {
        buf.extend_from_slice(b" in");
        for item in items {
            buf.push(b' ');
            buf.extend_from_slice(&item.raw);
        }
    }
    buf.extend_from_slice(b"\ndo\n");
    render_program_into(&for_command.body, buf);
    buf.extend_from_slice(b"\ndone");
}

#[cfg(test)]
pub(super) fn render_case(case_command: &CaseCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_case_into(case_command, &mut buf);
    buf
}

pub(super) fn render_case_into(case_command: &CaseCommand, buf: &mut Vec<u8>) {
    buf.extend_from_slice(b"case ");
    buf.extend_from_slice(&case_command.word.raw);
    buf.extend_from_slice(b" in");
    for arm in &case_command.arms {
        buf.push(b'\n');
        for (i, pattern) in arm.patterns.iter().enumerate() {
            if i > 0 {
                buf.extend_from_slice(b" | ");
            }
            buf.extend_from_slice(&pattern.raw);
        }
        buf.extend_from_slice(b")\n");
        render_program_into(&arm.body, buf);
        if arm.fallthrough {
            buf.extend_from_slice(b"\n;&");
        } else {
            buf.extend_from_slice(b"\n;;");
        }
    }
    buf.extend_from_slice(b"\nesac");
}

#[cfg(test)]
pub(super) fn render_simple(simple: &SimpleCommand) -> Vec<u8> {
    let mut buf = Vec::new();
    render_simple_into(simple, &mut buf);
    buf
}

pub(super) fn render_simple_into(simple: &SimpleCommand, buf: &mut Vec<u8>) {
    let mut base = Vec::new();
    for (i, assignment) in simple.assignments.iter().enumerate() {
        if i > 0 {
            base.push(b' ');
        }
        base.extend_from_slice(&assignment.name);
        base.push(b'=');
        base.extend_from_slice(&assignment.value.raw);
    }
    for word in &simple.words {
        if !base.is_empty() {
            base.push(b' ');
        }
        base.extend_from_slice(&word.raw);
    }
    render_command_line_with_redirections_into(base, &simple.redirections, buf);
}

pub(super) fn render_redirections_into(
    redirections: &[crate::syntax::ast::Redirection],
    redir_buf: &mut Vec<u8>,
    heredocs: &mut Vec<Vec<u8>>,
) {
    for (i, redirection) in redirections.iter().enumerate() {
        if i > 0 {
            redir_buf.push(b' ');
        }
        render_redirection_operator_into(redirection, redir_buf);
        if let Some(here_doc) = &redirection.here_doc {
            heredocs.push(render_here_doc_body(here_doc));
        }
    }
}

pub(super) fn render_redirection_operator_into(
    redirection: &crate::syntax::ast::Redirection,
    buf: &mut Vec<u8>,
) {
    if let Some(fd) = redirection.fd {
        crate::bstr::push_i64(buf, fd as i64);
    }
    let op: &[u8] = match redirection.kind {
        RedirectionKind::Read => b"<",
        RedirectionKind::Write => b">",
        RedirectionKind::ClobberWrite => b">|",
        RedirectionKind::Append => b">>",
        RedirectionKind::HereDoc => {
            if redirection
                .here_doc
                .as_ref()
                .is_some_and(|here_doc| here_doc.strip_tabs)
            {
                b"<<-"
            } else {
                b"<<"
            }
        }
        RedirectionKind::ReadWrite => b"<>",
        RedirectionKind::DupInput => b"<&",
        RedirectionKind::DupOutput => b">&",
    };
    buf.extend_from_slice(op);
    buf.extend_from_slice(&redirection.target.raw);
}

pub(super) fn render_here_doc_body(here_doc: &HereDoc) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&here_doc.body);
    if !here_doc.body.ends_with(b"\n") {
        out.push(b'\n');
    }
    out.extend_from_slice(&here_doc.delimiter);
    out
}

pub(super) fn render_command_line_with_redirections_into(
    base: Vec<u8>,
    redirections: &[crate::syntax::ast::Redirection],
    buf: &mut Vec<u8>,
) {
    let mut redir_text = Vec::new();
    let mut heredocs = Vec::new();
    render_redirections_into(redirections, &mut redir_text, &mut heredocs);
    buf.extend_from_slice(&base);
    if !redir_text.is_empty() {
        if !base.is_empty() {
            buf.push(b' ');
        }
        buf.extend_from_slice(&redir_text);
    }
    if !heredocs.is_empty() {
        buf.push(b'\n');
        for (i, hd) in heredocs.iter().enumerate() {
            if i > 0 {
                buf.push(b'\n');
            }
            buf.extend_from_slice(hd);
        }
    }
}

pub(super) fn render_redirected_command_into(
    command: &Command,
    redirections: &[crate::syntax::ast::Redirection],
    buf: &mut Vec<u8>,
) {
    let base = render_command(command);
    render_command_line_with_redirections_into(base, redirections, buf);
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use crate::exec::test_support::parse_test;
    use crate::shell::state::Shell;
    use crate::syntax::ast::{Assignment, HereDoc, Redirection, SimpleCommand, Word};
    use crate::sys::test_support::assert_no_syscalls;

    #[test]
    fn render_simple_handles_redirection_syntax() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: Some(5),
                        kind: RedirectionKind::ReadWrite,
                        target: Word {
                            raw: b"rw".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(0),
                        kind: RedirectionKind::DupInput,
                        target: Word {
                            raw: b"5".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                    Redirection {
                        fd: Some(1),
                        kind: RedirectionKind::DupOutput,
                        target: Word {
                            raw: b"-".to_vec().into(),
                            line: 0,
                        },
                        here_doc: None,
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            assert!(rendered.windows(4).any(|w| w == b"5<>r"));
            assert!(rendered.windows(4).any(|w| w == b"0<&5"));
            assert!(rendered.windows(4).any(|w| w == b"1>&-"));
        });
    }

    #[test]
    fn render_helpers_cover_program_function_if_loop_simple_pipeline() {
        assert_no_syscalls(|| {
            let program = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"true".to_vec().into(),
                                    line: 0,
                                }]
                                .into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false,
                    line: 0,
                }]
                .into_boxed_slice(),
            };

            let function = FunctionDef {
                name: b"greet".to_vec().into(),
                body: Box::new(Command::Group(program.clone())),
            };
            let if_command = IfCommand {
                condition: program.clone(),
                then_branch: program.clone(),
                elif_branches: Vec::new().into_boxed_slice(),

                else_branch: None,
            };
            let loop_command = LoopCommand {
                kind: LoopKind::While,
                condition: program.clone(),
                body: program.clone(),
            };
            assert!(render_program(&program).windows(4).any(|w| w == b"true"));
            assert!(
                render_function(&function)
                    .windows(7)
                    .any(|w| w == b"greet()")
            );
            assert!(render_if(&if_command).starts_with(b"if "));
            assert!(render_loop(&loop_command).starts_with(b"while "));

            let simple = SimpleCommand {
                assignments: vec![Assignment {
                    name: b"X".to_vec().into(),
                    value: Word {
                        raw: b"1".to_vec().into(),
                        line: 0,
                    },
                }]
                .into_boxed_slice(),
                words: vec![Word {
                    raw: b"echo".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word {
                        raw: b"out".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            };
            assert_eq!(render_simple(&simple), b"X=1 echo >out");

            let multi_assign = SimpleCommand {
                assignments: vec![
                    Assignment {
                        name: b"A".to_vec().into(),
                        value: Word {
                            raw: b"1".to_vec().into(),
                            line: 0,
                        },
                    },
                    Assignment {
                        name: b"B".to_vec().into(),
                        value: Word {
                            raw: b"2".to_vec().into(),
                            line: 0,
                        },
                    },
                ]
                .into_boxed_slice(),
                words: vec![].into_boxed_slice(),

                redirections: vec![].into_boxed_slice(),
            };
            assert_eq!(render_simple(&multi_assign), b"A=1 B=2");

            let pipeline = Pipeline {
                negated: true,
                timed: TimedMode::Off,
                commands: vec![
                    Command::Subshell(program.clone()),
                    Command::Group(program.clone()),
                    Command::FunctionDef(function),
                    Command::If(if_command),
                    Command::Loop(loop_command),
                ]
                .into_boxed_slice(),
            };
            assert!(render_pipeline(&pipeline).starts_with(b"! "));
        });
    }

    #[test]
    fn render_program_handles_async_and_heredoc_items() {
        assert_no_syscalls(|| {
            let async_program = Program {
                items: vec![
                    ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: b"true".to_vec().into(),
                                        line: 0,
                                    }]
                                    .into_boxed_slice(),
                                    ..SimpleCommand::default()
                                })]
                                .into_boxed_slice(),
                            },
                            rest: Vec::new().into_boxed_slice(),
                        },
                        asynchronous: true,
                        line: 0,
                    },
                    ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: b"false".to_vec().into(),
                                        line: 0,
                                    }]
                                    .into_boxed_slice(),
                                    ..SimpleCommand::default()
                                })]
                                .into_boxed_slice(),
                            },
                            rest: Vec::new().into_boxed_slice(),
                        },
                        asynchronous: false,
                        line: 0,
                    },
                ]
                .into_boxed_slice(),
            };
            assert_eq!(render_list_item(&async_program.items[0]), b"true &");
            assert_eq!(render_program(&async_program), b"true &\nfalse");

            let heredoc_program = parse_test(": <<EOF\nhello\nEOF\n").expect("parse heredoc");
            assert_eq!(render_program(&heredoc_program), b": <<EOF\nhello\nEOF");
        });
    }

    #[test]
    fn render_and_or_produces_correct_output() {
        assert_no_syscalls(|| {
            let render = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"true".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    })]
                    .into_boxed_slice(),
                },
                rest: vec![(
                    LogicalOp::And,
                    Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: b"false".to_vec().into(),
                                line: 0,
                            }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                )]
                .into_boxed_slice(),
            });
            assert!(render.windows(2).any(|w| w == b"&&"));
        });
    }

    #[test]
    fn case_pattern_matching_covers_wildcards_and_classes() {
        assert_no_syscalls(|| {
            assert!(case_pattern_matches(b"beta", b"b*"));
            assert!(case_pattern_matches(b"beta", b"b?t[ab]"));
            assert!(case_pattern_matches(b"x", b"[!ab]"));
            assert!(case_pattern_matches(b"*", b"\\*"));
            assert!(case_pattern_matches(b"-", b"[\\-]"));
            assert!(case_pattern_matches(b"b", b"[a-c]"));
            assert!(!case_pattern_matches(b"[", b"[a"));
            assert!(!case_pattern_matches(b"x", b"["));
            assert!(!case_pattern_matches(b"beta", b"a*"));
            assert!(!case_pattern_matches(b"a", b"[!ab]"));

            assert!(case_pattern_matches(b"a", b"[[:alpha:]]"));
            assert!(case_pattern_matches(b"Z", b"[[:alpha:]]"));
            assert!(!case_pattern_matches(b"5", b"[[:alpha:]]"));
            assert!(case_pattern_matches(b"3", b"[[:alnum:]]"));
            assert!(!case_pattern_matches(b"!", b"[[:alnum:]]"));
            assert!(case_pattern_matches(b" ", b"[[:blank:]]"));
            assert!(case_pattern_matches(b"\t", b"[[:blank:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:blank:]]"));
            assert!(case_pattern_matches(b"\x01", b"[[:cntrl:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:cntrl:]]"));
            assert!(case_pattern_matches(b"9", b"[[:digit:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:digit:]]"));
            assert!(case_pattern_matches(b"!", b"[[:graph:]]"));
            assert!(!case_pattern_matches(b" ", b"[[:graph:]]"));
            assert!(case_pattern_matches(b"a", b"[[:lower:]]"));
            assert!(!case_pattern_matches(b"A", b"[[:lower:]]"));
            assert!(case_pattern_matches(b" ", b"[[:print:]]"));
            assert!(case_pattern_matches(b"a", b"[[:print:]]"));
            assert!(!case_pattern_matches(b"\x01", b"[[:print:]]"));
            assert!(case_pattern_matches(b".", b"[[:punct:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:punct:]]"));
            assert!(case_pattern_matches(b"\n", b"[[:space:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:space:]]"));
            assert!(case_pattern_matches(b"A", b"[[:upper:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:upper:]]"));
            assert!(case_pattern_matches(b"f", b"[[:xdigit:]]"));
            assert!(!case_pattern_matches(b"g", b"[[:xdigit:]]"));
            assert!(!case_pattern_matches(b"a", b"[[:bogus:]]"));
            assert!(case_pattern_matches(b"x", b"[[:x]"));
        });
    }

    #[test]
    fn render_and_or_with_logical_or() {
        assert_no_syscalls(|| {
            let rendered = render_and_or(&AndOr {
                first: Pipeline {
                    negated: false,
                    timed: TimedMode::Off,
                    commands: vec![Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"false".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    })]
                    .into_boxed_slice(),
                },
                rest: vec![(
                    LogicalOp::Or,
                    Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![Command::Simple(SimpleCommand {
                            words: vec![Word {
                                raw: b"true".to_vec().into(),
                                line: 0,
                            }]
                            .into_boxed_slice(),
                            ..SimpleCommand::default()
                        })]
                        .into_boxed_slice(),
                    },
                )]
                .into_boxed_slice(),
            });
            assert_eq!(rendered, b"false || true");
        });
    }

    #[test]
    fn render_command_for_and_case() {
        assert_no_syscalls(|| {
            let for_cmd = Command::For(ForCommand {
                name: b"x".to_vec().into(),
                items: Some(
                    vec![Word {
                        raw: b"a".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                ),
                body: Program {
                    items: vec![ListItem {
                        and_or: AndOr {
                            first: Pipeline {
                                negated: false,
                                timed: TimedMode::Off,
                                commands: vec![Command::Simple(SimpleCommand {
                                    words: vec![Word {
                                        raw: b"echo".to_vec().into(),
                                        line: 0,
                                    }]
                                    .into_boxed_slice(),
                                    ..SimpleCommand::default()
                                })]
                                .into_boxed_slice(),
                            },
                            rest: Vec::new().into_boxed_slice(),
                        },
                        asynchronous: false,
                        line: 0,
                    }]
                    .into_boxed_slice(),
                },
            });
            let rendered = render_command(&for_cmd);
            assert_eq!(rendered, b"for x in a\ndo\necho\ndone");

            let case_cmd = Command::Case(CaseCommand {
                word: Word {
                    raw: b"val".to_vec().into(),
                    line: 0,
                },
                arms: vec![crate::syntax::ast::CaseArm {
                    patterns: vec![Word {
                        raw: b"a".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                    body: Program {
                        items: vec![ListItem {
                            and_or: AndOr {
                                first: Pipeline {
                                    negated: false,
                                    timed: TimedMode::Off,
                                    commands: vec![Command::Simple(SimpleCommand {
                                        words: vec![Word {
                                            raw: b"echo".to_vec().into(),
                                            line: 0,
                                        }]
                                        .into_boxed_slice(),
                                        ..SimpleCommand::default()
                                    })]
                                    .into_boxed_slice(),
                                },
                                rest: Vec::new().into_boxed_slice(),
                            },
                            asynchronous: false,
                            line: 0,
                        }]
                        .into_boxed_slice(),
                    },
                    fallthrough: false,
                }]
                .into_boxed_slice(),
            });
            let rendered = render_command(&case_cmd);
            assert!(rendered.starts_with(b"case val in"));
        });
    }

    #[test]
    fn render_if_with_elif_and_else() {
        assert_no_syscalls(|| {
            let true_program = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"true".to_vec().into(),
                                    line: 0,
                                }]
                                .into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false,
                    line: 0,
                }]
                .into_boxed_slice(),
            };
            let if_cmd = IfCommand {
                condition: true_program.clone(),
                then_branch: true_program.clone(),
                elif_branches: vec![crate::syntax::ast::ElifBranch {
                    condition: true_program.clone(),
                    body: true_program.clone(),
                }]
                .into_boxed_slice(),
                else_branch: Some(true_program),
            };
            let rendered = render_if(&if_cmd);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.contains("elif "));
            assert!(text.contains("\nthen\n"));
            assert!(text.contains("\nelse\n"));
            assert!(text.ends_with("\nfi"));
        });
    }

    #[test]
    fn render_loop_until() {
        assert_no_syscalls(|| {
            let prog = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"false".to_vec().into(),
                                    line: 0,
                                }]
                                .into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false,
                    line: 0,
                }]
                .into_boxed_slice(),
            };
            let loop_cmd = LoopCommand {
                kind: LoopKind::Until,
                condition: prog.clone(),
                body: prog,
            };
            let rendered = render_loop(&loop_cmd);
            assert!(rendered.starts_with(b"until "));
            assert!(rendered.ends_with(b"\ndone"));
        });
    }

    #[test]
    fn render_for_with_items_and_without() {
        assert_no_syscalls(|| {
            let body = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"echo".to_vec().into(),
                                    line: 0,
                                }]
                                .into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false,
                    line: 0,
                }]
                .into_boxed_slice(),
            };

            let with_items = ForCommand {
                name: b"item".to_vec().into(),
                items: Some(
                    vec![
                        Word {
                            raw: b"a".to_vec().into(),
                            line: 0,
                        },
                        Word {
                            raw: b"b".to_vec().into(),
                            line: 0,
                        },
                        Word {
                            raw: b"c".to_vec().into(),
                            line: 0,
                        },
                    ]
                    .into_boxed_slice(),
                ),
                body: body.clone(),
            };
            let rendered = render_for(&with_items);
            assert_eq!(rendered, b"for item in a b c\ndo\necho\ndone");

            let without_items = ForCommand {
                name: b"arg".to_vec().into(),
                items: None,
                body,
            };
            let rendered = render_for(&without_items);
            assert_eq!(rendered, b"for arg\ndo\necho\ndone");
        });
    }

    #[test]
    fn render_case_with_fallthrough_and_multi_pattern() {
        assert_no_syscalls(|| {
            let body = Program {
                items: vec![ListItem {
                    and_or: AndOr {
                        first: Pipeline {
                            negated: false,
                            timed: TimedMode::Off,
                            commands: vec![Command::Simple(SimpleCommand {
                                words: vec![Word {
                                    raw: b"echo".to_vec().into(),
                                    line: 0,
                                }]
                                .into_boxed_slice(),
                                ..SimpleCommand::default()
                            })]
                            .into_boxed_slice(),
                        },
                        rest: Vec::new().into_boxed_slice(),
                    },
                    asynchronous: false,
                    line: 0,
                }]
                .into_boxed_slice(),
            };
            let case_cmd = CaseCommand {
                word: Word {
                    raw: b"val".to_vec().into(),
                    line: 0,
                },
                arms: vec![
                    crate::syntax::ast::CaseArm {
                        patterns: vec![
                            Word {
                                raw: b"a".to_vec().into(),
                                line: 0,
                            },
                            Word {
                                raw: b"b".to_vec().into(),
                                line: 0,
                            },
                        ]
                        .into_boxed_slice(),
                        body: body.clone(),
                        fallthrough: true,
                    },
                    crate::syntax::ast::CaseArm {
                        patterns: vec![Word {
                            raw: b"c".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        body,
                        fallthrough: false,
                    },
                ]
                .into_boxed_slice(),
            };
            let rendered = render_case(&case_cmd);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.starts_with("case val in"));
            assert!(text.contains("a | b)"));
            assert!(text.contains("\n;&"));
            assert!(text.contains("c)"));
            assert!(text.contains("\n;;"));
            assert!(text.ends_with("\nesac"));
        });
    }

    #[test]
    fn render_redirection_operators_read_clobber_append_heredoc_strip() {
        assert_no_syscalls(|| {
            let read_redir = Redirection {
                fd: None,
                kind: RedirectionKind::Read,
                target: Word {
                    raw: b"input.txt".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            let mut buf = Vec::new();
            render_redirection_operator_into(&read_redir, &mut buf);
            assert_eq!(buf, b"<input.txt");

            let clobber_redir = Redirection {
                fd: Some(1),
                kind: RedirectionKind::ClobberWrite,
                target: Word {
                    raw: b"out.txt".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            buf.clear();
            render_redirection_operator_into(&clobber_redir, &mut buf);
            assert_eq!(buf, b"1>|out.txt");

            let append_redir = Redirection {
                fd: Some(2),
                kind: RedirectionKind::Append,
                target: Word {
                    raw: b"log".to_vec().into(),
                    line: 0,
                },
                here_doc: None,
            };
            buf.clear();
            render_redirection_operator_into(&append_redir, &mut buf);
            assert_eq!(buf, b"2>>log");

            let heredoc_strip = Redirection {
                fd: None,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: b"EOF".to_vec().into(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: b"EOF".to_vec().into(),
                    body: b"content\n".to_vec().into(),
                    expand: false,
                    strip_tabs: true,
                    body_line: 0,
                }),
            };
            buf.clear();
            render_redirection_operator_into(&heredoc_strip, &mut buf);
            assert_eq!(buf, b"<<-EOF");

            let heredoc_no_strip = Redirection {
                fd: None,
                kind: RedirectionKind::HereDoc,
                target: Word {
                    raw: b"END".to_vec().into(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: b"END".to_vec().into(),
                    body: b"stuff\n".to_vec().into(),
                    expand: false,
                    strip_tabs: false,
                    body_line: 0,
                }),
            };
            buf.clear();
            render_redirection_operator_into(&heredoc_no_strip, &mut buf);
            assert_eq!(buf, b"<<END");
        });
    }

    #[test]
    fn render_here_doc_body_appends_newline_when_missing() {
        assert_no_syscalls(|| {
            let with_newline = HereDoc {
                delimiter: b"EOF".to_vec().into(),
                body: b"hello\n".to_vec().into(),
                expand: false,
                strip_tabs: false,
                body_line: 0,
            };
            assert_eq!(render_here_doc_body(&with_newline), b"hello\nEOF");

            let without_newline = HereDoc {
                delimiter: b"EOF".to_vec().into(),
                body: b"hello".to_vec().into(),
                expand: false,
                strip_tabs: false,
                body_line: 0,
            };
            assert_eq!(render_here_doc_body(&without_newline), b"hello\nEOF");
        });
    }

    #[test]
    fn render_simple_with_multiple_heredocs() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![Word {
                    raw: b"cat".to_vec().into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                redirections: vec![
                    Redirection {
                        fd: None,
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF1".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF1".to_vec().into(),
                            body: b"first\n".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    },
                    Redirection {
                        fd: Some(3),
                        kind: RedirectionKind::HereDoc,
                        target: Word {
                            raw: b"EOF2".to_vec().into(),
                            line: 0,
                        },
                        here_doc: Some(HereDoc {
                            delimiter: b"EOF2".to_vec().into(),
                            body: b"second".to_vec().into(),
                            expand: false,
                            strip_tabs: false,
                            body_line: 0,
                        }),
                    },
                ]
                .into_boxed_slice(),
                ..SimpleCommand::default()
            };
            let rendered = render_simple(&simple);
            let text = std::str::from_utf8(&rendered).unwrap();
            assert!(text.starts_with("cat <<EOF1 3<<EOF2\n"));
            assert!(text.contains("first\nEOF1\n"));
            assert!(text.contains("second\nEOF2"));
        });
    }

    #[test]
    fn render_pipeline_multiple_commands() {
        assert_no_syscalls(|| {
            let pipeline = Pipeline {
                negated: false,
                timed: TimedMode::Off,
                commands: vec![
                    Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"cat".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    }),
                    Command::Simple(SimpleCommand {
                        words: vec![Word {
                            raw: b"grep".to_vec().into(),
                            line: 0,
                        }]
                        .into_boxed_slice(),
                        ..SimpleCommand::default()
                    }),
                ]
                .into_boxed_slice(),
            };
            let rendered = render_pipeline(&pipeline);
            assert_eq!(rendered, b"cat | grep");
        });
    }

    #[test]
    fn render_redirected_command() {
        assert_no_syscalls(|| {
            let cmd = Command::Redirected(
                Box::new(Command::Simple(SimpleCommand {
                    words: vec![Word {
                        raw: b"echo".to_vec().into(),
                        line: 0,
                    }]
                    .into_boxed_slice(),
                    ..SimpleCommand::default()
                })),
                vec![Redirection {
                    fd: None,
                    kind: RedirectionKind::Write,
                    target: Word {
                        raw: b"out.txt".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            );
            let rendered = render_command(&cmd);
            assert_eq!(rendered, b"echo >out.txt");
        });
    }

    #[test]
    fn render_command_line_redirections_only() {
        assert_no_syscalls(|| {
            let simple = SimpleCommand {
                words: vec![].into_boxed_slice(),
                assignments: vec![].into_boxed_slice(),
                redirections: vec![Redirection {
                    fd: Some(2),
                    kind: RedirectionKind::Append,
                    target: Word {
                        raw: b"err.log".to_vec().into(),
                        line: 0,
                    },
                    here_doc: None,
                }]
                .into_boxed_slice(),
            };
            let rendered = render_simple(&simple);
            assert_eq!(rendered, b"2>>err.log");
        });
    }
}
