mod ast;
mod token;

use std::collections::HashMap;
use std::fmt;

pub use ast::*;
use token::{Parser, SavedAliasState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: Box<str>,
    pub line: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &str) -> Result<Program, ParseError> {
    parse_with_aliases(source, &HashMap::new())
}

pub fn parse_with_aliases(
    source: &str,
    aliases: &HashMap<Box<str>, Box<str>>,
) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source, aliases);
    parser.parse_program_until(|_| false, false, false)
}

pub struct ParseSession<'src> {
    source: &'src str,
    pos: usize,
    line: usize,
    saved_alias: Option<SavedAliasState>,
}

impl<'src> ParseSession<'src> {
    pub fn new(source: &'src str) -> Result<Self, ParseError> {
        Ok(Self {
            source,
            pos: 0,
            line: 1,
            saved_alias: None,
        })
    }

    pub fn next_command(
        &mut self,
        aliases: &HashMap<Box<str>, Box<str>>,
    ) -> Result<Option<Program>, ParseError> {
        let mut parser = Parser::new_at(self.source, self.pos, self.line, aliases);

        if let Some(saved) = self.saved_alias.take() {
            parser.restore_alias_state(saved);
        }

        let result = parser.next_complete_command();

        self.pos = parser.source_pos();
        self.line = parser.line;
        self.saved_alias = parser.save_alias_state();

        result
    }

    pub fn current_line(&self) -> usize {
        self.line
    }
}

pub fn is_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && token::BYTE_CLASS[bytes[0] as usize] & token::BC_NAME_START != 0
        && bytes[1..]
            .iter()
            .fold(0xFFu8, |acc, &b| acc & token::BYTE_CLASS[b as usize])
            & token::BC_NAME_CONT
            != 0
}

#[cfg(test)]
mod tests {
    use super::token::{Token, alias_has_trailing_blank, parse_here_doc_delimiter};
    use super::*;

    fn parse_test(source: &str) -> Result<Program, ParseError> {
        parse(source)
    }

    fn parse_with_aliases_test(
        source: &str,
        aliases: &HashMap<Box<str>, Box<str>>,
    ) -> Result<Program, ParseError> {
        parse_with_aliases(source, aliases)
    }

    #[test]
    fn parses_simple_pipeline() {
        let program = parse_test("echo hi | wc -c").expect("parse");
        assert_eq!(program.items.len(), 1);
        assert_eq!(program.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn parses_assignments_and_groups() {
        let program = parse_test("FOO=bar echo \"$FOO\"").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.assignments.len() == 1 && &*cmd.words[0].raw == "echo"
        ));
    }

    #[test]
    fn parses_logical_and_subshell_forms() {
        let program = parse_test("(echo ok) && echo done || echo fail").expect("parse");
        let and_or = &program.items[0].and_or;
        assert_eq!(and_or.rest.len(), 2);
        assert!(matches!(
            and_or.first.commands.first(),
            Some(Command::Subshell(_))
        ));

        let linebreak_and_or =
            parse_test("true &&\n echo done ||\n echo fail").expect("parse linebreak and-or");
        assert_eq!(linebreak_and_or.items[0].and_or.rest.len(), 2);
    }

    #[test]
    fn tokenizes_terminated_single_quotes() {
        let program = parse_test("echo 'ok'").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == "'ok'"
        ));
    }

    #[test]
    fn parses_case_arm_without_trailing_dsemi_before_esac() {
        let program = parse_test("case x in x) esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_cmd) if case_cmd.arms.len() == 1
        ));
    }

    #[test]
    fn parses_heredoc_operator_shape() {
        let program = parse_test("cat <<EOF\nhello $USER\nEOF\n").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 1
                    && cmd.redirections[0].kind == RedirectionKind::HereDoc
                    && &*cmd.redirections[0].target.raw == "EOF"
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some("hello $USER\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(true)
        ));

        let quoted = parse_test("cat <<'EOF'\n$USER\nEOF\n").expect("parse");
        assert!(matches!(
            &quoted.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.delimiter) == Some("EOF")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.expand) == Some(false)
        ));

        let tab_stripped = parse_test("cat <<-\tEOF\n\tone\n\tEOF\n").expect("parse");
        assert!(matches!(
            &tab_stripped.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].here_doc.as_ref().map(|doc| &*doc.body) == Some("\tone\n")
                    && cmd.redirections[0].here_doc.as_ref().map(|doc| doc.strip_tabs) == Some(true)
        ));
    }

    #[test]
    fn parses_extended_redirection_forms() {
        let program = parse_test("cat 3<in 2>out 4>>log 5<>rw 0<&3 1>&2 2>|force").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 7
                    && cmd.redirections[0].fd == Some(3)
                    && cmd.redirections[0].kind == RedirectionKind::Read
                    && cmd.redirections[1].fd == Some(2)
                    && cmd.redirections[1].kind == RedirectionKind::Write
                    && cmd.redirections[2].fd == Some(4)
                    && cmd.redirections[2].kind == RedirectionKind::Append
                    && cmd.redirections[3].fd == Some(5)
                    && cmd.redirections[3].kind == RedirectionKind::ReadWrite
                    && cmd.redirections[4].kind == RedirectionKind::DupInput
                    && cmd.redirections[5].kind == RedirectionKind::DupOutput
                    && cmd.redirections[6].kind == RedirectionKind::ClobberWrite
        ));
    }

    #[test]
    fn parses_redirections_on_compound_commands() {
        let program = parse_test("{ echo hi; } >out").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Redirected(inner, redirections)
                if matches!(inner.as_ref(), Command::Group(_))
                    && redirections.len() == 1
                    && redirections[0].kind == RedirectionKind::Write
                    && &*redirections[0].target.raw == "out"
        ));

        let not_a_group = parse_test("{echo hi; }").expect("parse brace word");
        assert!(matches!(
            &not_a_group.items[0].and_or.first.commands[0],
            Command::Simple(simple) if &*simple.words[0].raw == "{echo"
        ));

        let closer_literal = parse_test("echo }").expect("parse literal closer");
        assert!(matches!(
            &closer_literal.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["echo", "}"]
        ));
    }

    #[test]
    fn parses_function_definition() {
        let program = parse_test("greet() { echo hi; }").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(function) if &*function.name == "greet"
        ));
        assert!(parse_test("if() { echo hi; }").is_err());
    }

    #[test]
    fn parses_negated_async_pipeline() {
        let program = parse_test("! echo ok | wc -c &").expect("parse");
        let item = &program.items[0];
        assert!(item.asynchronous);
        assert!(item.and_or.first.negated);
        assert_eq!(item.and_or.first.commands.len(), 2);

        let linebreak_pipeline =
            parse_test("printf ok |\n wc -c").expect("parse linebreak pipeline");
        assert_eq!(linebreak_pipeline.items[0].and_or.first.commands.len(), 2);
    }

    #[test]
    fn rejects_invalid_empty_command() {
        let error = parse_test("| wc").expect_err("parse should fail");
        assert_eq!(&*error.message, "expected command");

        let error = parse_test("echo hi | ! cat").expect_err("bang after pipe should fail");
        assert_eq!(&*error.message, "expected command");
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let error = parse_test("echo 'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated single quote");
    }

    #[test]
    fn rejects_unterminated_dollar_single_quote() {
        let error = parse_test("echo $'unterminated").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated dollar-single-quotes");
        let error = parse_test(r"echo $'backslash at end\").expect_err("parse should fail");
        assert_eq!(&*error.message, "unterminated dollar-single-quotes");
    }

    #[test]
    fn parses_if_with_elif_and_else() {
        let program =
            parse_test("if true; then echo yes; elif false; then echo no; else echo maybe; fi")
                .expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(if_command)
                if if_command.elif_branches.len() == 1 && if_command.else_branch.is_some()
        ));

        let simple_if = parse_test("if true; then echo yes; fi").expect("parse");
        assert!(matches!(
            &simple_if.items[0].and_or.first.commands[0],
            Command::If(if_command) if if_command.else_branch.is_none()
        ));
    }

    #[test]
    fn parses_while_and_until_loops() {
        let while_program = parse_test("while true; do echo yes; done").expect("parse");
        assert!(matches!(
            while_program.items[0].and_or.first.commands[0],
            Command::Loop(LoopCommand {
                kind: LoopKind::While,
                ..
            })
        ));

        let until_program = parse_test("until false; do echo yes; done").expect("parse");
        assert!(matches!(
            until_program.items[0].and_or.first.commands[0],
            Command::Loop(LoopCommand {
                kind: LoopKind::Until,
                ..
            })
        ));
    }

    #[test]
    fn parses_for_loops() {
        let explicit = parse_test("for item in a b c; do echo $item; done").expect("parse");
        assert!(matches!(
            &explicit.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if &*for_command.name == "item" && for_command.items.as_ref().map(|s| s.len()) == Some(3)
        ));

        let positional = parse_test("for item; do echo $item; done").expect("parse");
        assert!(matches!(
            &positional.items[0].and_or.first.commands[0],
            Command::For(for_command) if &*for_command.name == "item" && for_command.items.is_none()
        ));

        let linebreak_before_in =
            parse_test("for item\nin a b; do echo $item; done").expect("parse linebreak before in");
        assert!(matches!(
            &linebreak_before_in.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if &*for_command.name == "item"
                    && for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                        == Some(vec!["a", "b"])
        ));

        let reserved_words_as_items = parse_test("for item in do done; do echo $item; done")
            .expect("parse reserved words in wordlist");
        assert!(matches!(
            &reserved_words_as_items.items[0].and_or.first.commands[0],
            Command::For(for_command)
                if for_command.items.as_ref().map(|items| items.iter().map(|word| &*word.raw).collect::<Vec<_>>())
                    == Some(vec!["do", "done"])
        ));
    }

    #[test]
    fn parses_case_commands() {
        let program =
            parse_test("case $name in foo|bar) echo hit ;; baz) echo miss ;; esac").expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(case_command)
                if &*case_command.word.raw == "$name"
                    && case_command.arms.len() == 2
                    && case_command.arms[0].patterns.len() == 2
        ));

        let with_optional_paren = parse_test("case x in (x) echo ok ;; esac").expect("parse");
        assert!(matches!(
            &with_optional_paren.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let with_linebreak_before_in =
            parse_test("case x\nin\nx) echo ok ;;\nesac").expect("parse case linebreak");
        assert!(matches!(
            &with_linebreak_before_in.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.len() == 1
        ));

        let empty_after_linebreak =
            parse_test("case x\nin\nesac").expect("parse empty case after linebreak");
        assert!(matches!(
            &empty_after_linebreak.items[0].and_or.first.commands[0],
            Command::Case(case_command) if case_command.arms.is_empty()
        ));

        let fallthrough =
            parse_test("case a in a) echo one ;& b) echo two ;; esac").expect("parse fallthrough");
        if let Command::Case(c) = &fallthrough.items[0].and_or.first.commands[0] {
            assert_eq!(c.arms.len(), 2);
            assert!(c.arms[0].fallthrough);
            assert!(!c.arms[1].fallthrough);
        } else {
            panic!("expected Case");
        }

        let ft_clone = fallthrough.items[0].and_or.first.commands[0].clone();
        assert!(matches!(ft_clone, Command::Case(ref cc) if cc.arms[0].fallthrough));
    }

    #[test]
    fn parser_covers_misc_error_and_token_paths() {
        assert_eq!(
            format!(
                "{}",
                ParseError {
                    message: "x".into(),
                    line: None,
                }
            ),
            "x"
        );
        assert!(parse_test("echo \"unterminated").is_err());
        assert!(parse_test("cat <").is_err());
        assert!(parse_test("for 1 in a; do echo hi; done").is_err());
        assert!(parse_test("for item in ; do echo hi; done").is_ok());
        assert!(parse_test("case x in ; esac").is_err());
        assert!(parse_test("cat <<EOF").is_err());
        assert!(parse_test("echo 2>&").is_err());
        assert!(parse_test("if true; echo hi; fi").is_err());
        assert!(parse_test("while true; echo hi; done").is_err());
        assert!(parse_test("# comment only\n").is_ok());
        assert!(parse_test("echo foo\\ bar").is_ok());
        assert!(parse_test("echo \"a\\\"b\"").is_ok());
        assert!(parse_test("printf ok |\n wc -c").is_ok());
        assert!(parse_test("true &&\n echo ok").is_ok());
        assert!(parse_test("false ||\n echo ok").is_ok());
    }

    #[test]
    fn parse_session_uses_updated_aliases_between_commands() {
        let mut session = ParseSession::new("alias setok='printf ok'\nsetok\n").expect("session");
        let first = session
            .next_command(&HashMap::new())
            .expect("first cmd")
            .expect("some cmd");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(
            first.items[0].and_or.first.commands[0],
            Command::Simple(_)
        ));

        let second = session
            .next_command(&HashMap::from([(
                Box::from("setok"),
                Box::from("printf ok"),
            )]))
            .expect("second cmd")
            .expect("some cmd");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "ok"]
        ));

        assert!(
            session
                .next_command(&HashMap::new())
                .expect("eof")
                .is_none()
        );
    }

    #[test]
    fn alias_expansion_in_simple_commands() {
        let mut aliases = HashMap::new();
        aliases.insert("say".into(), "printf hi".into());
        let program = parse_with_aliases_test("say", &aliases).expect("parse alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "hi"]
        ));

        let mut aliases = HashMap::new();
        aliases.insert("cond".into(), "if".into());
        let program = parse_with_aliases_test("cond true; then echo ok; fi", &aliases)
            .expect("parse reserved alias");
        assert!(matches!(
            program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn standalone_bang_is_context_sensitive() {
        let program = parse_test("echo !").expect("parse echo bang");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["echo", "!"]
        ));

        let program = parse_test("!true").expect("parse bang word");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["!true"]
        ));

        let program = parse_test("! true").expect("parse negation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn trailing_blank_aliases_expand_next_simple_command_word() {
        let mut aliases = HashMap::new();
        aliases.insert("say".into(), "printf %s ".into());
        aliases.insert("word".into(), "ok".into());
        let program = parse_with_aliases_test("say word", &aliases).expect("parse chained alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["printf", "%s", "ok"]
        ));
    }

    #[test]
    fn self_referential_aliases_do_not_loop_indefinitely() {
        let mut aliases = HashMap::new();
        aliases.insert("loop".into(), "loop ".into());
        let program = parse_with_aliases_test("loop ok", &aliases).expect("self alias");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec!["loop", "ok"]
        ));
        assert!(alias_has_trailing_blank("value "));
        assert!(!alias_has_trailing_blank("value"));
    }

    #[test]
    fn alias_expansion_after_assignment_and_redirection() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".into(), "echo aliased".into());
        let program =
            parse_with_aliases_test("VAR=value foo", &aliases).expect("alias after assignment");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.assignments.len() == 1
        ));

        let program =
            parse_with_aliases_test("</dev/null foo", &aliases).expect("alias after redirection");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec!["echo", "aliased"]
                    && simple.redirections.len() == 1
        ));
    }

    #[test]
    fn lparen_after_simple_command_is_syntax_error() {
        let mut aliases = HashMap::new();
        aliases.insert("foo".into(), "echo aliased".into());
        let err = parse_with_aliases_test("foo () { true; }", &aliases).unwrap_err();
        assert!(
            err.message.contains("("),
            "error should mention '(': {}",
            err.message
        );
    }

    #[test]
    fn tokenizer_keeps_dollar_paren_as_single_word() {
        let program = parse_test("echo $(cmd arg)").expect("parse dollar paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "$(cmd arg)"
        ));
    }

    #[test]
    fn tokenizer_keeps_dollar_double_paren_as_single_word() {
        let program = parse_test("echo $((1 + 2))").expect("parse dollar arith");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "$((1 + 2))"
        ));

        let nested = parse_test("echo $((1 + (2 * 3)))").expect("parse nested arith");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if &*cmd.words[1].raw == "$((1 + (2 * 3)))"
        ));

        let error = parse_test("echo $((1 + 2").expect_err("unterminated arith");
        assert_eq!(&*error.message, "unterminated arithmetic expansion");
    }

    #[test]
    fn tokenizer_keeps_dollar_brace_as_single_word() {
        let program = parse_test("echo ${VAR:-default}").expect("parse dollar brace");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "${VAR:-default}"
        ));

        let nested = parse_test("echo ${VAR:-${INNER}}").expect("parse nested brace");
        assert!(matches!(
            &nested.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-${INNER}}"
        ));
    }

    #[test]
    fn tokenizer_keeps_backtick_as_single_word() {
        let program = parse_test("echo `cmd arg`").expect("parse backtick");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.words.len() == 2 && &*cmd.words[1].raw == "`cmd arg`"
        ));

        let error = parse_test("echo `unterminated").expect_err("unterminated backtick");
        assert_eq!(&*error.message, "unterminated backquote");
    }

    #[test]
    fn tokenizer_nested_constructs_in_brace_body() {
        let program = parse_test("echo ${VAR:-'a}b'}").expect("parse brace sq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-'a}b'}"
        ));

        let program = parse_test("echo ${VAR:-\"a}b\"}").expect("parse brace dq");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-\"a}b\"}"
        ));

        let program = parse_test("echo ${VAR:-\\}}").expect("parse brace escaped");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "${VAR:-\\}}"
        ));

        let error = parse_test("echo ${VAR:-unclosed").expect_err("unterminated brace body");
        assert_eq!(&*error.message, "unterminated parameter expansion");

        let error = parse_test("echo $(unclosed").expect_err("unterminated paren body");
        assert_eq!(&*error.message, "unterminated command substitution");
    }

    #[test]
    fn tokenizer_emits_io_number_for_adjacent_digits() {
        let p = parse_test("2>err echo ok").expect("parse 2>err");
        assert!(matches!(
            &p.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections.len() == 1
                    && cmd.redirections[0].fd == Some(2)
                    && cmd.redirections[0].kind == RedirectionKind::Write
        ));

        let p = parse_test("0<in echo ok").expect("parse 0<in");
        assert!(matches!(
            &p.items[0].and_or.first.commands[0],
            Command::Simple(cmd)
                if cmd.redirections[0].fd == Some(0)
                    && cmd.redirections[0].kind == RedirectionKind::Read
        ));
    }

    #[test]
    fn backslash_newline_continuation() {
        let program = parse_test("echo hel\\\nlo").expect("parse continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2 && &*cmd.words[1].raw == "hello"
        ));
    }

    #[test]
    fn if_empty_condition_is_parse_error() {
        let error = parse_test("if then fi").expect_err("empty if condition");
        assert!(error.message.contains("expected command list after 'if'"));
    }

    #[test]
    fn elif_empty_condition_is_parse_error() {
        let error =
            parse_test("if true; then true; elif then true; fi").expect_err("empty elif condition");
        assert!(error.message.contains("expected command list after 'elif'"));
    }

    #[test]
    fn while_empty_condition_is_parse_error() {
        let error = parse_test("while do true; done").expect_err("empty while condition");
        assert!(
            error
                .message
                .contains("expected command list after 'while'")
        );
    }

    #[test]
    fn until_empty_condition_is_parse_error() {
        let error = parse_test("until do true; done").expect_err("empty until condition");
        assert!(
            error
                .message
                .contains("expected command list after 'until'")
        );
    }

    #[test]
    fn time_default_pipeline() {
        let program = parse_test("time echo hello").expect("parse time default");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Default);
        assert!(!pipeline.negated);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn time_posix_pipeline() {
        let program = parse_test("time -p echo hello").expect("parse time -p");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.timed, TimedMode::Posix);
        assert!(
            matches!(&pipeline.commands[0], Command::Simple(cmd) if &*cmd.words[0].raw == "echo")
        );
    }

    #[test]
    fn function_keyword_basic() {
        let program = parse_test("function foo { echo hi; }").expect("parse function keyword");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_with_parens() {
        let program =
            parse_test("function foo() { echo hi; }").expect("parse function keyword parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(fd) if &*fd.name == "foo"
        ));
    }

    #[test]
    fn function_keyword_invalid_name() {
        let error = parse_test("function 123").expect_err("bad function name");
        assert_eq!(&*error.message, "expected function name");
    }

    #[test]
    fn clone_covers_all_command_variants() {
        let simple = Command::Simple(SimpleCommand {
            assignments: vec![Assignment {
                name: "X".into(),
                value: Word {
                    raw: "1".into(),
                    line: 0,
                },
            }]
            .into_boxed_slice(),
            words: vec![Word {
                raw: "echo".into(),
                line: 0,
            }]
            .into_boxed_slice(),
            redirections: vec![Redirection {
                fd: Some(2),
                kind: RedirectionKind::Write,
                target: Word {
                    raw: "err".into(),
                    line: 0,
                },
                here_doc: None,
            }]
            .into_boxed_slice(),
        });
        let s = simple.clone();
        assert!(matches!(&s, Command::Simple(sc) if &*sc.words[0].raw == "echo"));

        let subshell = Command::Subshell(Program {
            items: vec![ListItem {
                and_or: AndOr {
                    first: Pipeline {
                        negated: false,
                        timed: TimedMode::Off,
                        commands: vec![s.clone()].into_boxed_slice(),
                    },
                    rest: vec![].into_boxed_slice(),
                },
                asynchronous: false,
                line: 0,
            }]
            .into_boxed_slice(),
        });
        assert!(matches!(subshell.clone(), Command::Subshell(_)));

        let group = Command::Group(Program {
            items: vec![].into_boxed_slice(),
        });
        assert!(matches!(group.clone(), Command::Group(_)));

        let func = Command::FunctionDef(FunctionDef {
            name: "f".into(),
            body: Box::new(s.clone()),
        });
        assert!(matches!(&func, Command::FunctionDef(fd) if &*fd.name == "f"));

        let if_cmd = Command::If(IfCommand {
            condition: Program {
                items: vec![].into_boxed_slice(),
            },
            then_branch: Program {
                items: vec![].into_boxed_slice(),
            },
            elif_branches: vec![ElifBranch {
                condition: Program {
                    items: vec![].into_boxed_slice(),
                },
                body: Program {
                    items: vec![].into_boxed_slice(),
                },
            }]
            .into_boxed_slice(),
            else_branch: Some(Program {
                items: vec![].into_boxed_slice(),
            }),
        });
        assert!(matches!(if_cmd, Command::If(_)));

        let loop_cmd = Command::Loop(LoopCommand {
            kind: LoopKind::While,
            condition: Program {
                items: vec![].into_boxed_slice(),
            },
            body: Program {
                items: vec![].into_boxed_slice(),
            },
        });
        assert!(matches!(loop_cmd, Command::Loop(_)));

        let for_cmd = Command::For(ForCommand {
            name: "i".into(),
            items: Some(
                vec![Word {
                    raw: "a".into(),
                    line: 0,
                }]
                .into_boxed_slice(),
            ),
            body: Program {
                items: vec![].into_boxed_slice(),
            },
        });
        assert!(matches!(&for_cmd, Command::For(fc) if &*fc.name == "i"));

        let case_cmd = Command::Case(CaseCommand {
            word: Word {
                raw: "x".into(),
                line: 0,
            },
            arms: vec![CaseArm {
                patterns: vec![Word {
                    raw: "*".into(),
                    line: 0,
                }]
                .into_boxed_slice(),
                body: Program {
                    items: vec![].into_boxed_slice(),
                },
                fallthrough: false,
            }]
            .into_boxed_slice(),
        });
        assert!(matches!(case_cmd, Command::Case(_)));

        let redir = Command::Redirected(
            Box::new(s.clone()),
            vec![Redirection {
                fd: None,
                kind: RedirectionKind::Write,
                target: Word {
                    raw: "out".into(),
                    line: 0,
                },
                here_doc: Some(HereDoc {
                    delimiter: "EOF".into(),
                    body: "test\n".into(),
                    expand: true,
                    strip_tabs: false,
                    body_line: 0,
                }),
            }]
            .into_boxed_slice(),
        );
        assert!(matches!(redir, Command::Redirected(_, _)));
    }

    #[test]
    fn alias_expansion_produces_non_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert("both".into(), "echo a; echo b".into());
        let program =
            parse_with_aliases_test("both", &aliases).expect("parse alias with semicolon");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn alias_expansion_interns_reserved_word_tokens() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "myif".into(),
            "if true; then echo ok; elif false; then echo no; else echo fb; fi".into(),
        );
        let program =
            parse_with_aliases_test("myif", &aliases).expect("alias if/then/elif/else/fi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("mywhile".into(), "while false; do echo loop; done".into());
        let program = parse_with_aliases_test("mywhile", &aliases).expect("alias while/do/done");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myuntil".into(), "until true; do echo u; done".into());
        let program = parse_with_aliases_test("myuntil", &aliases).expect("alias until");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myfor".into(), "for x in a b; do echo $x; done".into());
        let program = parse_with_aliases_test("myfor", &aliases).expect("alias for/in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("mycase".into(), "case x in a) echo a;; esac".into());
        let program = parse_with_aliases_test("mycase", &aliases).expect("alias case/esac");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myfn".into(), "function myfunc { echo hi; }".into());
        let program = parse_with_aliases_test("myfn", &aliases).expect("alias function/{/}");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(_)
        ));

        let mut aliases = HashMap::new();
        aliases.insert("myneg".into(), "! true".into());
        let program = parse_with_aliases_test("myneg", &aliases).expect("alias bang");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn alias_not_expanded_in_reserved_word_position() {
        let mut aliases = HashMap::new();
        aliases.insert("bla".into(), "in".into());
        let result = parse_with_aliases_test("for i bla a b c; do echo $i; done", &aliases);
        assert!(
            result.is_err(),
            "alias should not expand to 'in' in reserved word position"
        );
    }

    #[test]
    fn alias_shadows_keyword_at_command_position() {
        // POSIX 2024 §2.3.1: "it is unspecified whether the TOKEN is subject
        // to alias substitution" when it would also be a reserved word.
        // We choose alias-first (like bash/zsh).
        let mut aliases = HashMap::new();
        aliases.insert("if".into(), "hello".into());
        let program = parse_with_aliases_test("if", &aliases).expect("parse");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(s) if s.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == vec!["hello"]
        ));
    }

    #[test]
    fn backslash_newline_mid_word_produces_stripped_form() {
        let program = parse_test("ec\\\nho ok").expect("continuation in command");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == "echo" && &*cmd.words[1].raw == "ok"
        ));

        let program = parse_test("echo a\\\nb\\\nc").expect("multiple continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "abc"
        ));
    }

    #[test]
    fn backslash_newline_before_comment_does_not_start_comment() {
        let program = parse_test("a\\\n#b").expect("continuation before hash");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[0].raw == "a#b"
        ));
    }

    #[test]
    fn backslash_newline_before_operator_resets() {
        let program = parse_test("echo a\\\nb; echo c").expect("continuation before semi");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn backslash_newline_inside_double_quotes_preserved() {
        let program = parse_test("echo \"ab\\\ncd\"").expect("dquote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "\"ab\\\ncd\""
        ));
    }

    #[test]
    fn backslash_newline_inside_single_quotes_preserved() {
        let program = parse_test("echo 'ab\\\ncd'").expect("squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_backticks_preserved() {
        let program = parse_test("echo `ab\\\ncd`").expect("backtick continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "`ab\\\ncd`"
        ));
    }

    #[test]
    fn backslash_newline_inside_dollar_single_quote_preserved() {
        let program = parse_test("echo $'ab\\\ncd'").expect("dollar-squote continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "$'ab\\\ncd'"
        ));
    }

    #[test]
    fn backslash_newline_inside_command_substitution_preserved() {
        let program = parse_test("echo $(ab\\\ncd)").expect("cmdsub continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "$(ab\\\ncd)"
        ));
    }

    #[test]
    fn backslash_newline_mixed_unquoted_and_dquoted() {
        let program = parse_test("echo hel\\\nlo\"wor\\\nld\"").expect("mixed continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if &*cmd.words[1].raw == "hello\"wor\\\nld\""
        ));
    }

    #[test]
    fn heredoc_delimiter_helpers() {
        assert_eq!(parse_here_doc_delimiter("\"EOF\""), ("EOF".into(), false));
        assert_eq!(parse_here_doc_delimiter("\\EOF"), ("EOF".into(), false));
    }

    #[test]
    fn heredoc_delimiter_backslash_inside_double_quotes() {
        assert_eq!(
            parse_here_doc_delimiter("\"ab\\\"cd\""),
            ("ab\"cd".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\\b\""),
            ("a\\b".into(), false)
        );
        assert_eq!(parse_here_doc_delimiter("\"a\\$b\""), ("a$b".into(), false));
    }

    #[test]
    fn heredoc_delimiter_dollar_single_quote() {
        assert_eq!(parse_here_doc_delimiter("$'EOF'"), ("EOF".into(), false));
        assert_eq!(
            parse_here_doc_delimiter("$'ab\\'cd'"),
            ("ab'cd".into(), false)
        );
    }

    #[test]
    fn arithmetic_expansion_with_quoting() {
        let program = parse_test("echo $(( 1 + 2 ))").expect("basic arithmetic");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program = parse_test("echo $(( \")\" ))").expect("arithmetic with quoted paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn consume_scan_covers_dollar_single_quote_and_default_in_subshell() {
        let program = parse_test("echo $(echo $'hi' done)").expect("dollar-sq in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let program = parse_test("echo $(echo $VAR done)").expect("bare dollar in paren");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));

        let err = parse_test("echo $(echo 'unterminated)").expect_err("sq in paren");
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn is_name_basic() {
        assert!(is_name("FOO"));
        assert!(is_name("_bar"));
        assert!(is_name("a1"));
        assert!(!is_name(""));
        assert!(!is_name("1abc"));
    }

    #[test]
    fn aliases_basic() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("ls".into(), "ls --color".into());
        aliases.insert("ll".into(), "ls -la".into());

        assert_eq!(aliases.get("ls").map(|s| &**s), Some("ls --color"));
        assert_eq!(aliases.get("ll").map(|s| &**s), Some("ls -la"));
        assert_eq!(aliases.get("xyz"), None);
    }

    #[test]
    fn multi_line_alias_produces_separate_commands() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("both".into(), "echo a\necho b".into());
        let mut session = ParseSession::new("both\necho c").unwrap();

        let first = session
            .next_command(&aliases)
            .expect("first")
            .expect("some");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(
            &first.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "a"]
        ));

        let second = session
            .next_command(&aliases)
            .expect("second")
            .expect("some");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "b"]
        ));

        let third = session
            .next_command(&aliases)
            .expect("third")
            .expect("some");
        assert_eq!(third.items.len(), 1);
        assert!(matches!(
            &third.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "c"]
        ));

        assert!(session.next_command(&aliases).expect("eof").is_none());
    }

    #[test]
    fn heredoc_delimiter_backslash_preserves_non_special_in_dquotes() {
        assert_eq!(
            parse_here_doc_delimiter("\"E\\OF\""),
            ("E\\OF".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\nb\""),
            ("a\\nb".into(), false)
        );

        assert_eq!(parse_here_doc_delimiter("\"a\\$b\""), ("a$b".into(), false));
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\\b\""),
            ("a\\b".into(), false)
        );
        assert_eq!(
            parse_here_doc_delimiter("\"a\\\"b\""),
            ("a\"b".into(), false)
        );
        assert_eq!(parse_here_doc_delimiter("\"a\\`b\""), ("a`b".into(), false));
    }

    #[test]
    fn io_number_recognised_inside_alias() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("redir".into(), "echo hello 2>/dev/null".into());
        let program = parse_with_aliases_test("redir", &aliases).expect("alias with IO number");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if {
                let has_echo = cmd.words.iter().any(|w| &*w.raw == "echo");
                let has_redir_fd2 = cmd.redirections.iter().any(|r|
                    r.fd == Some(2) && r.kind == RedirectionKind::Write
                );

                let no_word_2 = !cmd.words.iter().any(|w| &*w.raw == "2");
                has_echo && has_redir_fd2 && no_word_2
            }
        ));
    }

    #[test]
    fn comment_with_close_paren_inside_command_substitution() {
        let program =
            parse_test("echo $(echo hello # )\necho world\n)").expect("comment with ) in $(...)");

        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn backslash_newline_continuation_in_alias() {
        let mut aliases: HashMap<Box<str>, Box<str>> = HashMap::new();
        aliases.insert("foo".into(), "echo hell\\\no".into());
        let program = parse_with_aliases_test("foo", &aliases).expect("alias with continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == ["echo", "hello"]
        ));
    }

    #[test]
    fn heredoc_quoted_delimiter_no_continuation() {
        let program = parse_test("cat <<'EOF'\nEO\\\nF\nreal body\nEOF\n")
            .expect("quoted heredoc with backslash line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            _ => panic!("expected simple command"),
        };
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.body, "EO\\\nF\nreal body\n");
        assert!(!doc.expand);
    }

    #[test]
    fn backslash_newline_before_comment_in_command_substitution() {
        let program = parse_test("echo $(echo foo \\\n# comment with )\necho bar)\n")
            .expect("continuation before comment in $(...)");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn heredoc_body_inside_alias_expansion() {
        let aliases: HashMap<Box<str>, Box<str>> = [("x".into(), "cat <<EOF\nhello\nEOF".into())]
            .into_iter()
            .collect();
        let program = parse_with_aliases_test("x\n", &aliases).expect("heredoc inside alias");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 1, "word count");
        assert_eq!(&*cmd.words[0].raw, "cat");
        assert_eq!(cmd.redirections.len(), 1, "redirection count");
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0]
            .here_doc
            .as_ref()
            .expect("heredoc body should be present");
        assert_eq!(&*doc.body, "hello\n");
        assert_eq!(&*doc.delimiter, "EOF");
        assert!(doc.expand);
    }

    #[test]
    fn continuation_splits_keyword_if() {
        let program =
            parse_test("i\\\nf true; then echo ha; fi\n").expect("if split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_then() {
        let program =
            parse_test("if true; th\\\nen echo ha; fi\n").expect("then split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.then_branch.items.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_while() {
        let program =
            parse_test("wh\\\nile false; do :; done\n").expect("while split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_do() {
        let program =
            parse_test("while false; d\\\no :; done\n").expect("do split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_done() {
        let program =
            parse_test("while false; do :; do\\\nne\n").expect("done split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_for() {
        let program =
            parse_test("fo\\\nr i in a; do echo $i; done\n").expect("for split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_case() {
        let program =
            parse_test("cas\\\ne x in x) echo y;; esac\n").expect("case split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_alias_name() {
        let aliases: HashMap<Box<str>, Box<str>> = [("foo".into(), "echo aliased".into())]
            .into_iter()
            .collect();
        let program =
            parse_with_aliases_test("fo\\\no\n", &aliases).expect("alias split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[0].raw, "echo");
        assert_eq!(&*cmd.words[1].raw, "aliased");
    }

    #[test]
    fn continuation_in_word() {
        let program = parse_test("echo he\\\nllo\n").expect("word continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[1].raw, "hello");
    }

    #[test]
    fn continuation_splits_double_semicolon() {
        let program =
            parse_test("case x in x) echo y;\\\n;esac\n").expect(";; split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(cmd) => cmd,
            other => panic!("expected Case, got {other:?}"),
        };
        assert_eq!(cmd.arms.len(), 1);
        assert!(!cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_and_if() {
        let program = parse_test("true &\\\n& echo ok\n").expect("&& split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::And);
    }

    #[test]
    fn continuation_splits_or_if() {
        let program = parse_test("false |\\\n| echo ok\n").expect("|| split by continuation");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::Or);
    }

    #[test]
    fn continuation_splits_heredoc_operator() {
        let program = parse_test("cat <\\\n<EOF\nhello\nEOF\n").expect("<< split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections.len(), 1);
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.body, "hello\n");
    }

    #[test]
    fn continuation_splits_append_operator() {
        let program = parse_test("echo hi >\\\n> /dev/null\n").expect(">> split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections.len(), 1);
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::Append);
    }

    #[test]
    fn multiple_continuations_in_keyword() {
        let program = parse_test("w\\\nh\\\ni\\\nl\\\ne false; do :; done\n")
            .expect("while with many continuations");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_at_start_of_input() {
        let program = parse_test("\\\nif true; then echo ha; fi\n").expect("continuation at start");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_esac() {
        let program =
            parse_test("case x in x) echo y;; es\\\nac\n").expect("esac split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_elif() {
        let program = parse_test("if false; then :; el\\\nif true; then echo ok; fi\n")
            .expect("elif split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert_eq!(cmd.elif_branches.len(), 1);
    }

    #[test]
    fn continuation_splits_keyword_else() {
        let program = parse_test("if false; then :; el\\\nse echo ok; fi\n")
            .expect("else split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::If(c) => c,
            other => panic!("expected If, got {other:?}"),
        };
        assert!(cmd.else_branch.is_some());
    }

    #[test]
    fn continuation_splits_keyword_fi() {
        let program =
            parse_test("if true; then echo ok; f\\\ni\n").expect("fi split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::If(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_in() {
        let program =
            parse_test("for i i\\\nn a b; do echo $i; done\n").expect("in split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(_)
        ));
    }

    #[test]
    fn continuation_splits_keyword_until() {
        let program = parse_test("un\\\ntil false; do echo ok; break; done\n")
            .expect("until split by continuation");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Loop(_)
        ));
    }

    #[test]
    fn continuation_splits_dup_input() {
        let program = parse_test("cat <\\\n&0 < /dev/null\n").expect("<& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupInput)
        );
    }

    #[test]
    fn continuation_splits_dup_output() {
        let program = parse_test("echo hi >\\\n&2\n").expect(">& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupOutput)
        );
    }

    #[test]
    fn continuation_splits_read_write() {
        let program = parse_test("echo ok <\\\n> /dev/null\n").expect("<> split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ReadWrite)
        );
    }

    #[test]
    fn continuation_splits_clobber_write() {
        let program = parse_test("echo ok >\\\n| /dev/null\n").expect(">| split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ClobberWrite)
        );
    }

    #[test]
    fn continuation_splits_heredoc_strip_tabs() {
        let program =
            parse_test("cat <\\\n<-EOF\n\thello\n\tEOF\n").expect("<<- split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.redirections[0].kind, RedirectionKind::HereDoc);
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert!(doc.strip_tabs);
    }

    #[test]
    fn continuation_splits_semi_amp() {
        let program = parse_test("case x in x) echo first;\\\n& y) echo second;; esac\n")
            .expect(";& split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected Case, got {other:?}"),
        };
        assert!(cmd.arms[0].fallthrough);
    }

    #[test]
    fn continuation_splits_bang_negation() {
        let program = parse_test("!\\\n true\n").expect("! with continuation");
        assert!(program.items[0].and_or.first.negated);
    }

    #[test]
    fn continuation_splits_heredoc_delimiter_word() {
        let program = parse_test("cat <<EO\\\nF\nhello\nEOF\n")
            .expect("heredoc delimiter split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        let doc = cmd.redirections[0].here_doc.as_ref().expect("heredoc body");
        assert_eq!(&*doc.delimiter, "EOF");
        assert_eq!(&*doc.body, "hello\n");
    }

    #[test]
    fn continuation_splits_assignment() {
        let program =
            parse_test("x\\\n=hello echo $x\n").expect("assignment split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.assignments.len(), 1);
        assert_eq!(&*cmd.assignments[0].name, "x");
    }

    #[test]
    fn continuation_splits_io_number() {
        let program =
            parse_test("echo ok 2\\\n>/dev/null\n").expect("IO number split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.fd == Some(2)));
    }

    #[test]
    fn continuation_inside_double_quotes() {
        let program =
            parse_test("echo \"he\\\nllo\"\n").expect("continuation inside double quotes");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_inside_backticks() {
        let program = parse_test("echo `echo he\\\nllo`\n").expect("continuation inside backticks");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_arith_close() {
        let program = parse_test("echo $((1+2)\\\n)\n").expect("arith close split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with("$(("));
    }

    #[test]
    fn continuation_splits_dollar_paren() {
        let program = parse_test("echo $\\\n(echo inner)\n").expect("$( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn continuation_splits_dollar_brace() {
        let program = parse_test("x=hello; echo $\\\n{x}\n").expect("${ split by continuation");
        let cmd = match &program.items[1].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert_eq!(&*cmd.words[1].raw, "${x}");
    }

    #[test]
    fn continuation_splits_dollar_double_paren() {
        let program = parse_test("echo $(\\\n(1+2))\n").expect("$(( split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
        assert!(cmd.words[1].raw.starts_with("$(("));
    }

    #[test]
    fn continuation_splits_dollar_single_quote() {
        let program = parse_test("echo $\\\n'hello'\n").expect("$' split by continuation");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(cmd.words.len(), 2);
    }

    #[test]
    fn arithmetic_unmatched_close_paren() {
        let program = parse_test("echo $(( 1 ) + 2 ))").expect("arithmetic with unmatched )");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.len() == 2
        ));
    }

    #[test]
    fn heredoc_with_operators_on_same_line() {
        let program =
            parse_test("cat <<EOF | wc -l\nhello\nEOF\n").expect("heredoc with pipe on line");
        let pipeline = &program.items[0].and_or.first;
        assert_eq!(pipeline.commands.len(), 2);
    }

    #[test]
    fn heredoc_with_redirect_on_same_line() {
        let program =
            parse_test("cat <<EOF >out\nhello\nEOF\n").expect("heredoc with redirect on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
    }

    #[test]
    fn heredoc_with_append_redirect_on_same_line() {
        let program =
            parse_test("cat <<EOF >>out\nhello\nEOF\n").expect("heredoc with append on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::Append)
        );
    }

    #[test]
    fn heredoc_with_fd_dup_on_same_line() {
        let program = parse_test("cat <<EOF 2>&1\nhello\nEOF\n").expect("heredoc with dup on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupOutput)
        );
    }

    #[test]
    fn heredoc_with_clobber_on_same_line() {
        let program =
            parse_test("cat <<EOF >|out\nhello\nEOF\n").expect("heredoc with clobber on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ClobberWrite)
        );
    }

    #[test]
    fn heredoc_with_and_on_same_line() {
        let program =
            parse_test("cat <<EOF && echo ok\nhello\nEOF\n").expect("heredoc with && on line");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::And);
    }

    #[test]
    fn heredoc_with_or_on_same_line() {
        let program =
            parse_test("cat <<EOF || echo fail\nhello\nEOF\n").expect("heredoc with || on line");
        assert_eq!(program.items[0].and_or.rest.len(), 1);
        assert_eq!(program.items[0].and_or.rest[0].0, LogicalOp::Or);
    }

    #[test]
    fn heredoc_with_semicolon_on_same_line() {
        let program =
            parse_test("cat <<EOF ; echo after\nhello\nEOF\n").expect("heredoc with ; on line");
        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn heredoc_with_background_on_same_line() {
        let program =
            parse_test("cat <<EOF & echo after\nhello\nEOF\n").expect("heredoc with & on line");
        assert!(program.items[0].asynchronous);
    }

    #[test]
    fn heredoc_with_word_on_same_line() {
        let program = parse_test("cmd <<EOF arg\nhello\nEOF\n").expect("heredoc with word on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[1].raw, "arg");
    }

    #[test]
    fn heredoc_with_io_number_on_same_line() {
        let program =
            parse_test("cmd <<EOF 2>err\nhello\nEOF\n").expect("heredoc with io number on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.iter().any(|r| r.fd == Some(2)));
    }

    #[test]
    fn multiple_heredocs_on_same_line() {
        let program = parse_test("cat <<A <<B\nbody1\nA\nbody2\nB\n")
            .expect("multiple heredocs on same line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
    }

    #[test]
    fn heredoc_with_less_and_on_same_line() {
        let program = parse_test("cat <<EOF <&3\nhello\nEOF\n").expect("heredoc with <& on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::DupInput)
        );
    }

    #[test]
    fn heredoc_with_less_great_on_same_line() {
        let program =
            parse_test("cat <<EOF <>file\nhello\nEOF\n").expect("heredoc with <> on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::ReadWrite)
        );
    }

    #[test]
    fn heredoc_with_less_on_same_line() {
        let program = parse_test("cat <<EOF <input\nhello\nEOF\n").expect("heredoc with < on line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(
            cmd.redirections
                .iter()
                .any(|r| r.kind == RedirectionKind::Read)
        );
    }

    #[test]
    fn heredoc_with_dsemi_on_same_line() {
        let program =
            parse_test("case x in\nx) cat <<EOF ;;\nhello\nEOF\nesac\n").expect("heredoc dsemi");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Case(_)
        ));
    }

    #[test]
    fn heredoc_with_strip_tabs_on_subsequent() {
        let program = parse_test("cat <<A <<-B\nbody1\nA\n\tbody2\nB\n")
            .expect("heredoc with <<- on same line");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.redirections.len() >= 2);
        assert!(
            cmd.redirections[1]
                .here_doc
                .as_ref()
                .map_or(false, |h| h.strip_tabs)
        );
    }

    #[test]
    fn heredoc_continuation_in_body() {
        let program = parse_test("cat <<EOF\nline\\\ncontinued\nEOF\n")
            .expect("heredoc continuation in body");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        let body = &cmd.redirections[0].here_doc.as_ref().unwrap().body;
        assert!(body.contains("line\\\n"));
    }

    #[test]
    fn reclassify_queued_alias_on_heredoc_line() {
        let mut aliases = HashMap::new();
        aliases.insert("myalias".into(), "echo hello".into());
        let program = parse_with_aliases_test("cat <<EOF | myalias\nbody\nEOF\n", &aliases)
            .expect("alias on heredoc line");
        let pipeline = &program.items[0].and_or.first;
        assert!(pipeline.commands.len() >= 2);
    }

    #[test]
    fn single_quote_in_alias_expansion() {
        let mut aliases = HashMap::new();
        aliases.insert("sq".into(), "echo 'hello world'".into());
        let program = parse_with_aliases_test("sq", &aliases).expect("alias with single quote");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words.iter().any(|w| w.raw.contains("hello")));
    }

    #[test]
    fn consume_paren_body_with_nested_parens() {
        let program = parse_test("echo $( (echo inner) )").expect("nested parens in $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("inner"));
    }

    #[test]
    fn consume_paren_body_with_comment() {
        let program =
            parse_test("echo $(# comment\necho ok)").expect("comment inside command subst");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("ok"));
    }

    #[test]
    fn consume_paren_body_with_backslash() {
        let program =
            parse_test("echo $(printf '%s\\n' hello)").expect("backslash inside command subst");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("\\n"));
    }

    #[test]
    fn consume_quoted_element_backtick() {
        let program = parse_test("echo \"`echo inner`\"").expect("backtick in double quote");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("`echo inner`"));
    }

    #[test]
    fn dollar_construct_in_double_quotes() {
        let program = parse_test("echo \"$(echo inner)\"").expect("dollar-paren in double quotes");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("$(echo inner)"));
    }

    #[test]
    fn scan_raw_word_trailing_backslash_eof() {
        let program = parse_test("echo \\").expect("trailing backslash at eof");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words.iter().any(|w| *w.raw == *"\\"));
    }

    #[test]
    fn scan_raw_word_continuation_at_word_start_produces_word() {
        let program = parse_test("\\\necho hello").expect("continuation at start then word");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert_eq!(&*cmd.words[0].raw, "echo");
    }

    #[test]
    fn error_expected_close_brace() {
        let result = parse_test("{ echo ok");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("}"));
    }

    #[test]
    fn error_expected_close_paren_subshell() {
        let result = parse_test("(echo ok");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains(")"));
    }

    #[test]
    fn error_unexpected_lparen_after_word() {
        let result = parse_test("echo hello(");
        assert!(result.is_err());
    }

    #[test]
    fn function_def_via_word_lparen_rparen() {
        let program = parse_test("myfn() { echo hi; }").expect("function def");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == "myfn"
        ));
    }

    #[test]
    fn command_starts_with_redirect() {
        let program = parse_test(">out echo hello").expect("redirect before command");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(!cmd.redirections.is_empty());
        assert!(!cmd.words.is_empty());
    }

    #[test]
    fn heredoc_delimiter_parse_dollar_single_quote() {
        let (delim, expand) = parse_here_doc_delimiter("$'EOF'");
        assert_eq!(&*delim, "EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_parse_double_quoted() {
        let (delim, expand) = parse_here_doc_delimiter("\"EOF\"");
        assert_eq!(&*delim, "EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_parse_backslash_escape() {
        let (delim, expand) = parse_here_doc_delimiter("E\\OF");
        assert_eq!(&*delim, "EOF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_double_quote_with_backslash() {
        let (delim, expand) = parse_here_doc_delimiter("\"E\\$OF\"");
        assert_eq!(&*delim, "E$OF");
        assert!(!expand);
    }

    #[test]
    fn heredoc_delimiter_dollar_single_quote_with_escape() {
        let (delim, expand) = parse_here_doc_delimiter("$'E\\'OF'");
        assert_eq!(&*delim, "E'OF");
        assert!(!expand);
    }

    #[test]
    fn parse_session_saves_alias_state() {
        let source = "ls\nls\n";
        let mut session = ParseSession::new(source).unwrap();
        let mut aliases = HashMap::new();
        aliases.insert("ls".into(), "ls --color ".into());
        let r1 = session.next_command(&aliases).unwrap();
        assert!(r1.is_some());
        let r2 = session.next_command(&aliases).unwrap();
        assert!(r2.is_some());
    }

    #[test]
    fn case_with_semi_amp_fallthrough() {
        let program =
            parse_test("case x in\nx) echo ok ;&\ny) echo y ;;\nesac\n").expect("case fallthrough");
        let case = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected case, got {other:?}"),
        };
        assert!(case.arms[0].fallthrough);
    }

    #[test]
    fn case_with_plain_semi_before_esac() {
        let result = parse_test("case x in\nx) echo ok ;\nesac\n");
        assert!(
            result.is_err()
                || matches!(
                    result.as_ref().unwrap().items[0].and_or.first.commands[0],
                    Command::Case(_)
                )
        );
    }

    #[test]
    fn for_loop_without_in_clause() {
        let program = parse_test("for x do echo $x; done").expect("for loop without in");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(f) if f.items.is_none()
        ));
    }

    #[test]
    fn for_loop_with_non_word_after_in() {
        let program = parse_test("for x in ; do echo $x; done").expect("for in with empty list");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::For(f) if f.items.as_ref().map_or(false, |i| i.is_empty())
        ));
    }

    #[test]
    fn function_keyword_def() {
        let program = parse_test("function myfn { echo hi; }").expect("function keyword def");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == "myfn"
        ));
    }

    #[test]
    fn function_keyword_def_with_optional_parens() {
        let program =
            parse_test("function myfn() { echo hi; }").expect("function keyword with parens");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == "myfn"
        ));
    }

    #[test]
    fn heredoc_with_paren_on_same_line() {
        let program =
            parse_test("(cat <<EOF\nhello\nEOF\n)").expect("heredoc with paren on same line");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Subshell(_)
        ));
    }

    #[test]
    fn parse_session_current_line() {
        let source = "echo hello\necho world\n";
        let session = ParseSession::new(source).unwrap();
        assert_eq!(session.current_line(), 1);
    }

    #[test]
    fn heredoc_with_semi_amp_on_same_line() {
        let program = parse_test("case x in\nx) cat <<EOF ;&\nhello\nEOF\ny) echo y ;;\nesac\n")
            .expect("heredoc semiamp on same line");
        let case = match &program.items[0].and_or.first.commands[0] {
            Command::Case(c) => c,
            other => panic!("expected case, got {other:?}"),
        };
        assert!(case.arms[0].fallthrough);
    }

    #[test]
    fn heredoc_with_rparen_on_same_line() {
        let program =
            parse_test("(cat <<EOF )\nhello\nEOF\n").expect("heredoc with rparen on same line");
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::Subshell(_)
        ));
    }

    #[test]
    fn heredoc_with_lparen_on_same_line() {
        let program = parse_test("cat <<EOF ; (echo sub)\nhello\nEOF\n")
            .expect("heredoc with lparen on same line");
        assert!(
            program.items.len() >= 2 || {
                let pipeline = &program.items[0].and_or.first;
                pipeline.commands.len() >= 1
            }
        );
    }

    #[test]
    fn heredoc_empty_delimiter_error() {
        let result = parse_test("cat << \nhello\n");
        assert!(result.is_err());
    }

    #[test]
    fn reclassify_queued_keyword_on_heredoc_line() {
        let program = parse_test("cat <<EOF ; if true; then echo yes; fi\nbody\nEOF\n")
            .expect("keyword on heredoc line");
        assert!(program.items.len() >= 2);
    }

    #[test]
    fn reclassify_queued_alias_with_trailing_blank() {
        let mut aliases = HashMap::new();
        aliases.insert("ll".into(), "ls -l ".into());
        let program = parse_with_aliases_test("cat <<EOF | ll foo\nbody\nEOF\n", &aliases)
            .expect("alias with trailing blank on heredoc line");
        let pipeline = &program.items[0].and_or.first;
        assert!(pipeline.commands.len() >= 2);
    }

    #[test]
    fn consume_paren_body_backslash_non_newline() {
        let program =
            parse_test("echo $(printf '\\t')").expect("backslash non-newline in paren body");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("\\t"));
    }

    #[test]
    fn error_case_expected_word() {
        let result = parse_test("case ; in esac");
        assert!(result.is_err());
    }

    #[test]
    fn error_for_expected_variable_name() {
        let result = parse_test("for ; do echo hi; done");
        assert!(result.is_err());
    }

    #[test]
    fn error_case_pattern_missing_rparen() {
        let result = parse_test("case x in\nx echo ;;\nesac\n");
        assert!(result.is_err());
    }

    #[test]
    fn error_function_keyword_expected_name() {
        let result = parse_test("function ; { echo hi; }");
        assert!(result.is_err());
    }

    #[test]
    fn error_function_keyword_invalid_name() {
        let result = parse_test("function 123 { echo hi; }");
        assert!(result.is_err());
    }

    #[test]
    fn split_assignment_non_name_returns_none() {
        use super::ast::split_assignment;
        assert!(split_assignment("123=val").is_none());
    }

    #[test]
    fn list_item_partial_eq() {
        let p1 = parse_test("echo a").unwrap();
        let p2 = parse_test("echo a").unwrap();
        assert_eq!(p1.items[0], p2.items[0]);
    }

    #[test]
    fn word_partial_eq() {
        let p1 = parse_test("echo hello").unwrap();
        let p2 = parse_test("echo hello").unwrap();
        let cmd1 = match &p1.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        };
        let cmd2 = match &p2.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        };
        assert_eq!(cmd1.words[0], cmd2.words[0]);
    }

    #[test]
    fn heredoc_partial_eq() {
        let p1 = parse_test("cat <<EOF\nhello\nEOF\n").unwrap();
        let p2 = parse_test("cat <<EOF\nhello\nEOF\n").unwrap();
        let r1 = &match &p1.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        }
        .redirections[0]
            .here_doc;
        let r2 = &match &p2.items[0].and_or.first.commands[0] {
            Command::Simple(c) => c,
            _ => panic!(),
        }
        .redirections[0]
            .here_doc;
        assert_eq!(r1, r2);
    }

    #[test]
    fn error_redirect_only_no_command() {
        let result = parse_test("< ;");
        assert!(result.is_err());
    }

    #[test]
    fn consume_paren_body_backslash_escape_non_newline() {
        let program = parse_test("echo $(echo a\\b)").expect("backslash escape inside $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("\\b"));
    }

    #[test]
    fn consume_paren_body_backtick_inside_command_substitution() {
        let program = parse_test("echo $(echo `echo inner`)").expect("backtick inside $()");
        let cmd = match &program.items[0].and_or.first.commands[0] {
            Command::Simple(cmd) => cmd,
            other => panic!("expected simple command, got {other:?}"),
        };
        assert!(cmd.words[1].raw.contains("`echo inner`"));
    }

    #[test]
    fn display_name_for_keywords_and_word() {
        use super::token::Token;
        assert_eq!(&*Token::If.display_name(), "if");
        assert_eq!(&*Token::Then.display_name(), "then");
        assert_eq!(&*Token::Else.display_name(), "else");
        assert_eq!(&*Token::Elif.display_name(), "elif");
        assert_eq!(&*Token::Fi.display_name(), "fi");
        assert_eq!(&*Token::Do.display_name(), "do");
        assert_eq!(&*Token::Done.display_name(), "done");
        assert_eq!(&*Token::Case.display_name(), "case");
        assert_eq!(&*Token::Esac.display_name(), "esac");
        assert_eq!(&*Token::In.display_name(), "in");
        assert_eq!(&*Token::While.display_name(), "while");
        assert_eq!(&*Token::Until.display_name(), "until");
        assert_eq!(&*Token::For.display_name(), "for");
        assert_eq!(&*Token::Function.display_name(), "function");
        assert_eq!(&*Token::Bang.display_name(), "!");
        assert_eq!(&*Token::LBrace.display_name(), "{");
        assert_eq!(&*Token::RBrace.display_name(), "}");
        assert_eq!(&*Token::Word("foo".into()).display_name(), "word");
    }

    #[test]
    fn token_into_word_some_and_none() {
        assert_eq!(Token::Word("hi".into()).into_word(), Some(Box::from("hi")));
        assert_eq!(Token::Eof.into_word(), None);
        assert_eq!(Token::Semi.into_word(), None);
    }

    #[test]
    fn nested_alias_pop_exhausted_layers_break() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("outer"), Box::from("inner rest"));
        aliases.insert(Box::from("inner"), Box::from("echo"));
        let program =
            parse_with_aliases_test("outer\n", &aliases).expect("nested alias should parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, "echo");
            assert_eq!(&*cmd.words[1].raw, "rest");
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn skip_continuations_pushback_between_operators() {
        let program = parse_test("echo a >\\\nout\n").expect("continuation in redirect");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn unterminated_single_quote_in_alias_layer() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("bad"), Box::from("echo 'unterminated"));
        let result = parse_with_aliases_test("bad\n", &aliases);
        assert!(result.is_err());
    }

    #[test]
    fn empty_heredoc_delimiter_error() {
        let result = parse_test("cat << \n");
        assert!(result.is_err());
    }

    #[test]
    fn heredoc_line_huge_ionumber_becomes_word() {
        let big = "99999999999999999999";
        let src = format!("cat <<EOF {big}hello\nbody\nEOF\n");
        let result = parse_test(&src);
        assert!(result.is_ok());
    }

    #[test]
    fn produce_word_eof_on_empty_prefix_and_delim() {
        let program = parse_test("echo $()\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(cmd.words.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn case_fall_through_semi_amp() {
        let program = parse_test("case x in a) echo a ;& b) echo b ;; esac").expect("parse");
        if let Command::Case(c) = &program.items[0].and_or.first.commands[0] {
            assert!(c.arms[0].fallthrough);
            assert!(!c.arms[1].fallthrough);
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn case_semi_before_non_esac_error() {
        let result = parse_test("case x in a) echo a ; b) echo b ;; esac");
        assert!(result.is_err());
    }

    #[test]
    fn case_arm_without_separator_before_esac() {
        let program = parse_test("case x in a) echo a\nesac").expect("parse");
        if let Command::Case(c) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(c.arms.len(), 1);
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn for_loop_break_on_non_word() {
        let program = parse_test("for i in a b; do echo $i; done").expect("parse");
        if let Command::For(f) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(f.items.as_ref().unwrap().len(), 2);
        } else {
            panic!("expected for command");
        }
    }

    #[test]
    fn syntax_error_unexpected_lparen_after_name_without_rparen() {
        let result = parse_test("foo(bar");
        assert!(result.is_err());
    }

    #[test]
    fn empty_command_at_if_position() {
        let result = parse_test("if ; then true; fi");
        assert!(result.is_err());
    }

    #[test]
    fn for_loop_non_word_token_breaks_word_list() {
        let program = parse_test("for i in a b\ndo echo $i; done").expect("parse");
        if let Command::For(f) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(f.items.as_ref().unwrap().len(), 2);
        } else {
            panic!("expected for command");
        }
    }

    #[test]
    fn next_complete_command_eof() {
        let aliases = HashMap::new();
        let mut session = ParseSession::new("echo hi").expect("session");
        let cmd = session.next_command(&aliases).expect("first cmd");
        assert!(cmd.is_some());
        let cmd2 = session.next_command(&aliases).expect("eof");
        assert!(cmd2.is_none());
    }

    #[test]
    fn next_complete_command_empty_line_returns_none() {
        let aliases = HashMap::new();
        let mut session = ParseSession::new("\n").expect("session");
        let cmd = session.next_command(&aliases).expect("newline only");
        assert!(cmd.is_none());
    }

    #[test]
    fn reclassify_queued_token_trailing_blank_alias_on_heredoc_line() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("mycmd"), Box::from("echo "));
        aliases.insert(Box::from("myarg"), Box::from("hello"));
        let program =
            parse_with_aliases_test("cat <<EOF mycmd myarg\nbody\nEOF\n", &aliases).expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn heredoc_line_skip_continuations_between_tokens() {
        let program = parse_test("cat <<EOF >\\\nout\nbody\nEOF\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn backslash_newline_at_word_start_before_delim() {
        let program = parse_test("echo \\\n\n").expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn skip_continuations_pushback_on_heredoc_line() {
        let program = parse_test("cat <<EOF >\\out\nbody\nEOF\n").expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert!(!cmd.redirections.is_empty());
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn heredoc_line_large_number_not_ionumber() {
        let big = "999999999999";
        let src = format!("cat <<EOF {big}word\nbody\nEOF\n");
        let program = parse_test(&src).expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn empty_heredoc_delimiter_after_ltlt() {
        let result = parse_test("cat <<\n");
        assert!(result.is_err());
    }

    #[test]
    fn reclassify_trailing_blank_alias_on_heredoc_line() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("myalias"), Box::from("echo "));
        let program = parse_with_aliases_test("cat <<EOF myalias world\nbody\nEOF\n", &aliases)
            .expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn produce_word_returns_eof_on_empty_continuation() {
        let program = parse_test("echo \\\n  \n").expect("parse");
        assert_eq!(program.items.len(), 1);
    }

    #[test]
    fn alias_trailing_blank_triggers_reclassify_on_heredoc_line() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("A"), Box::from("cat "));
        aliases.insert(Box::from("B"), Box::from("extra"));
        let program = parse_with_aliases_test("A <<EOF B\nhello\nEOF\n", &aliases).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, "cat");
            assert_eq!(&*cmd.words[1].raw, "extra");
            assert_eq!(cmd.redirections.len(), 1);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn second_heredoc_empty_delimiter_error() {
        assert!(parse_test("cat <<EOF <<\nhello\nEOF\n").is_err());
    }

    #[test]
    fn heredoc_line_overflow_ionumber_becomes_word() {
        let src = "cat <<EOF 99999999999>out\nhello\nEOF\n";
        let program = parse_test(src).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[1].raw, "99999999999");
            assert_eq!(cmd.redirections.len(), 2);
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn alias_expanding_to_blanks_produces_eof_in_produce_word() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("A"), Box::from("   "));
        let program = parse_with_aliases_test("A ; echo done\n", &aliases).expect("parse");
        assert!(program.items.is_empty());
    }

    #[test]
    fn alias_ineligible_word_on_heredoc_line_skips_expansion() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("A"), Box::from("cat "));
        aliases.insert(Box::from("'B'"), Box::from("extra"));
        let program =
            parse_with_aliases_test("A <<EOF 'B'\nhello\nEOF\n", &aliases).expect("parse");
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, "cat");
            assert_eq!(&*cmd.words[1].raw, "'B'");
        } else {
            panic!("expected simple command");
        }
    }

    #[test]
    fn unterminated_case_arm_breaks_loop() {
        assert!(parse_test("case x in x) echo hi").is_err());
    }

    #[test]
    fn trailing_semicolon_is_valid() {
        let source = "true;\n";
        let mut session = ParseSession::new(source).unwrap();
        let aliases = HashMap::new();
        let p = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p.items.len(), 1);
        assert!(session.next_command(&aliases).unwrap().is_none());
    }

    #[test]
    fn semicolon_then_newline_then_command() {
        let source = "echo a;\necho b\n";
        let mut session = ParseSession::new(source).unwrap();
        let aliases = HashMap::new();
        let p1 = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p1.items.len(), 1);
        let p2 = session.next_command(&aliases).unwrap().expect("second cmd");
        assert_eq!(p2.items.len(), 1);
    }

    #[test]
    fn comment_after_semicolon_is_ignored() {
        let source = "echo a;#comment\necho b\n";
        let mut session = ParseSession::new(source).unwrap();
        let aliases = HashMap::new();
        let p1 = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p1.items.len(), 1);
        let p2 = session.next_command(&aliases).unwrap().expect("second cmd");
        assert_eq!(p2.items.len(), 1);
    }

    #[test]
    fn case_pattern_accepts_keyword_word() {
        let program = parse_test("case if in if) echo ok;; esac").expect("parse");
        assert_eq!(program.items.len(), 1);
        if let Command::Case(case) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*case.arms[0].patterns[0].raw, "if");
        } else {
            panic!("expected case command");
        }
    }

    #[test]
    fn function_keyword_without_parens() {
        let program = parse_test("function foo { echo hi; }").expect("parse");
        assert_eq!(program.items.len(), 1);
        assert!(matches!(
            &program.items[0].and_or.first.commands[0],
            Command::FunctionDef(f) if &*f.name == "foo"
        ));
    }

    #[test]
    fn self_referential_alias_does_not_loop() {
        let mut aliases = HashMap::new();
        aliases.insert(Box::from("a"), Box::from("a"));
        let program =
            parse_with_aliases_test("a\n", &aliases).expect("self-referential alias should parse");
        assert_eq!(program.items.len(), 1);
        if let Command::Simple(cmd) = &program.items[0].and_or.first.commands[0] {
            assert_eq!(&*cmd.words[0].raw, "a");
        } else {
            panic!("expected simple command");
        }
    }
}
