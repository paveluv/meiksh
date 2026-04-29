pub(crate) mod ast;
pub(crate) mod byte_class;
mod declaration_context;
mod token;
pub(crate) mod word_part;

use std::cell::Cell;

use ast::Program;
use token::{Parser, SavedAliasState};

use crate::hash::ShellMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParseError {
    pub(crate) message: Box<[u8]>,
    pub(crate) line: Option<usize>,
}

/// Parse-time options sampled by the lexer. Currently a single
/// boolean (`bash_procsub`) but kept as a struct so future extensions
/// (`bash_arrays`, `bash_compat`, etc.) slot in without changing
/// callsites.
///
/// The slot is captured at the start of each top-level parse and
/// flows through to recursive lexer entries (`$(...)`, here-docs,
/// backtick substitutions) via the [`PARSE_OPTIONS`] thread-local.
/// See `docs/features/process-substitution.md` § 2.3.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ParseOptions {
    pub(crate) bash_procsub: bool,
}

thread_local! {
    /// Thread-local snapshot of [`ParseOptions`] active for the
    /// current parse. Set at the top of [`parse_with_options`] and
    /// restored on the way out so recursive lexer reentries
    /// (`$(...)` body parsed via [`parse`], here-docs, etc.)
    /// observe the same flags as the outer parse without having to
    /// thread them through every free helper.
    ///
    /// `Cell` is sound because parsing is synchronous and
    /// single-threaded; the shell does not parse concurrently from
    /// multiple OS threads.
    static PARSE_OPTIONS: Cell<ParseOptions> = const { Cell::new(ParseOptions { bash_procsub: false }) };
}

/// Borrow the active parse options. Read by the lexer when deciding
/// whether to recognize `<(` / `>(`.
pub(crate) fn current_parse_options() -> ParseOptions {
    PARSE_OPTIONS.with(|c| c.get())
}

pub(crate) fn parse(source: &[u8]) -> Result<Program, ParseError> {
    parse_with_aliases(source, &ShellMap::default())
}

pub(crate) fn parse_with_aliases(
    source: &[u8],
    aliases: &ShellMap<Box<[u8]>, Box<[u8]>>,
) -> Result<Program, ParseError> {
    let mut parser = Parser::new(source, aliases);
    parser.parse_program_until(|_| false, false, false)
}

/// Parse `source` with a freshly-installed [`ParseOptions`] snapshot
/// so the lexer can sample option-gated tokens like `<(` / `>(`.
/// Restores the previous snapshot on return; this is a fully nested
/// stack discipline matching the call graph of the parser.
pub(crate) fn parse_with_options(
    source: &[u8],
    aliases: &ShellMap<Box<[u8]>, Box<[u8]>>,
    options: ParseOptions,
) -> Result<Program, ParseError> {
    let _guard = ParseOptionsGuard::install(options);
    parse_with_aliases(source, aliases)
}

/// RAII guard that installs a [`ParseOptions`] snapshot on
/// construction and restores the previous snapshot on drop. Holding
/// the guard across the lexer's recursive entries is what makes
/// nested `$(...)` parses observe the outer mode.
pub(crate) struct ParseOptionsGuard {
    previous: ParseOptions,
}

impl ParseOptionsGuard {
    pub(crate) fn install(options: ParseOptions) -> Self {
        let previous = PARSE_OPTIONS.with(|c| c.replace(options));
        Self { previous }
    }
}

impl Drop for ParseOptionsGuard {
    fn drop(&mut self) {
        PARSE_OPTIONS.with(|c| c.set(self.previous));
    }
}

pub(crate) struct ParseSession<'src> {
    source: &'src [u8],
    pos: usize,
    line: usize,
    saved_alias: Option<SavedAliasState>,
}

impl<'src> ParseSession<'src> {
    pub(crate) fn new(source: &'src [u8]) -> Result<Self, ParseError> {
        Ok(Self {
            source,
            pos: 0,
            line: 1,
            saved_alias: None,
        })
    }

    /// Convenience wrapper that captures the currently-installed
    /// [`ParseOptions`] from the thread-local. Production callsites
    /// always have access to a [`Shell`] and pass options
    /// explicitly via [`Self::next_command_with_options`]; this
    /// shorter form is kept for tests that don't care about
    /// option-gated tokens.
    #[cfg(test)]
    pub(crate) fn next_command(
        &mut self,
        aliases: &ShellMap<Box<[u8]>, Box<[u8]>>,
    ) -> Result<Option<Program>, ParseError> {
        self.next_command_with_options(aliases, current_parse_options())
    }

    /// Parse the next command with an explicit [`ParseOptions`]
    /// snapshot installed for the duration of this entry. Used by
    /// the REPL to capture the live `bash_procsub` value (and any
    /// future option-gated tokens) at the moment of parsing rather
    /// than at the moment the command is executed — see spec § 2.3.
    pub(crate) fn next_command_with_options(
        &mut self,
        aliases: &ShellMap<Box<[u8]>, Box<[u8]>>,
        options: ParseOptions,
    ) -> Result<Option<Program>, ParseError> {
        let _guard = ParseOptionsGuard::install(options);
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

    #[cfg(test)]
    pub(crate) fn current_line(&self) -> usize {
        self.line
    }

    pub(crate) fn current_pos(&self) -> usize {
        self.pos
    }
}

pub(crate) fn is_name(name: &[u8]) -> bool {
    byte_class::is_name(name)
}

#[cfg(test)]
pub(crate) fn build_heredoc_parts(body: &[u8]) -> Vec<word_part::WordPart> {
    token::build_heredoc_parts(body)
}

#[cfg(test)]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_macros,
    clippy::disallowed_methods
)]
mod tests {
    use super::ast::Command;
    use super::*;

    fn bx(s: &[u8]) -> Box<[u8]> {
        s.to_vec().into_boxed_slice()
    }

    fn alias_map(pairs: &[(&[u8], &[u8])]) -> ShellMap<Box<[u8]>, Box<[u8]>> {
        pairs.iter().map(|(k, v)| (bx(k), bx(v))).collect()
    }

    #[test]
    fn is_name_basic() {
        assert!(is_name(b"FOO"));
        assert!(is_name(b"_bar"));
        assert!(is_name(b"a1"));
        assert!(!is_name(b""));
        assert!(!is_name(b"1abc"));
    }

    #[test]
    fn aliases_basic() {
        let mut aliases: ShellMap<Box<[u8]>, Box<[u8]>> = ShellMap::default();
        aliases.insert(bx(b"ls"), bx(b"ls --color"));
        aliases.insert(bx(b"ll"), bx(b"ls -la"));

        assert_eq!(
            aliases.get(&b"ls"[..]).map(|s| &**s),
            Some(&b"ls --color"[..])
        );
        assert_eq!(aliases.get(&b"ll"[..]).map(|s| &**s), Some(&b"ls -la"[..]));
        assert_eq!(aliases.get(&b"xyz"[..]), None);
    }

    #[test]
    fn parse_session_uses_updated_aliases_between_commands() {
        let mut session = ParseSession::new(b"alias setok='printf ok'\nsetok\n").expect("session");
        let first = session
            .next_command(&ShellMap::default())
            .expect("first cmd")
            .expect("some cmd");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(
            first.items[0].and_or.first.commands[0],
            Command::Simple(_)
        ));

        let second = session
            .next_command(&ShellMap::from_iter([(bx(b"setok"), bx(b"printf ok"))]))
            .expect("second cmd")
            .expect("some cmd");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(simple)
                if simple.words.iter().map(|word| &*word.raw).collect::<Vec<_>>() == vec![&b"printf"[..], &b"ok"[..]]
        ));

        assert!(
            session
                .next_command(&ShellMap::default())
                .expect("eof")
                .is_none()
        );
    }

    #[test]
    fn multi_line_alias_produces_separate_commands() {
        let aliases: ShellMap<Box<[u8]>, Box<[u8]>> = alias_map(&[(b"both", b"echo a\necho b")]);
        let mut session = ParseSession::new(b"both\necho c").unwrap();

        let first = session
            .next_command(&aliases)
            .expect("first")
            .expect("some");
        assert_eq!(first.items.len(), 1);
        assert!(matches!(
            &first.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == [&b"echo"[..], &b"a"[..]]
        ));

        let second = session
            .next_command(&aliases)
            .expect("second")
            .expect("some");
        assert_eq!(second.items.len(), 1);
        assert!(matches!(
            &second.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == [&b"echo"[..], &b"b"[..]]
        ));

        let third = session
            .next_command(&aliases)
            .expect("third")
            .expect("some");
        assert_eq!(third.items.len(), 1);
        assert!(matches!(
            &third.items[0].and_or.first.commands[0],
            Command::Simple(cmd) if cmd.words.iter().map(|w| &*w.raw).collect::<Vec<_>>() == [&b"echo"[..], &b"c"[..]]
        ));

        assert!(session.next_command(&aliases).expect("eof").is_none());
    }

    #[test]
    fn parse_session_saves_alias_state() {
        let mut session = ParseSession::new(b"ls\nls\n").unwrap();
        let aliases = alias_map(&[(b"ls", b"ls --color ")]);
        let r1 = session.next_command(&aliases).unwrap();
        assert!(r1.is_some());
        let r2 = session.next_command(&aliases).unwrap();
        assert!(r2.is_some());
    }

    #[test]
    fn parse_session_current_line() {
        let session = ParseSession::new(b"echo hello\necho world\n").unwrap();
        assert_eq!(session.current_line(), 1);
    }

    #[test]
    fn next_complete_command_eof() {
        let aliases = ShellMap::default();
        let mut session = ParseSession::new(b"echo hi").expect("session");
        let cmd = session.next_command(&aliases).expect("first cmd");
        assert!(cmd.is_some());
        let cmd2 = session.next_command(&aliases).expect("eof");
        assert!(cmd2.is_none());
    }

    #[test]
    fn next_complete_command_empty_line_returns_none() {
        let aliases = ShellMap::default();
        let mut session = ParseSession::new(b"\n").expect("session");
        let cmd = session.next_command(&aliases).expect("newline only");
        assert!(cmd.is_none());
    }

    #[test]
    fn trailing_semicolon_is_valid() {
        let mut session = ParseSession::new(b"true;\n").unwrap();
        let aliases = ShellMap::default();
        let p = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p.items.len(), 1);
        assert!(session.next_command(&aliases).unwrap().is_none());
    }

    #[test]
    fn semicolon_then_newline_then_command() {
        let mut session = ParseSession::new(b"echo a;\necho b\n").unwrap();
        let aliases = ShellMap::default();
        let p1 = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p1.items.len(), 1);
        let p2 = session.next_command(&aliases).unwrap().expect("second cmd");
        assert_eq!(p2.items.len(), 1);
    }

    #[test]
    fn comment_after_semicolon_is_ignored() {
        let mut session = ParseSession::new(b"echo a;#comment\necho b\n").unwrap();
        let aliases = ShellMap::default();
        let p1 = session.next_command(&aliases).unwrap().expect("first cmd");
        assert_eq!(p1.items.len(), 1);
        let p2 = session.next_command(&aliases).unwrap().expect("second cmd");
        assert_eq!(p2.items.len(), 1);
    }
}
