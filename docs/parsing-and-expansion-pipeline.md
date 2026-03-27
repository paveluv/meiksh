# Parsing and Expansion Pipeline

This document describes the complete data flow in `meiksh` from raw shell input to the fully expanded argument lists that the execution engine consumes. It covers every data structure, every intermediate representation, and every transformation stage.

## Overview

The pipeline has three major phases:

```
Raw source text
       │
       ▼
  ┌──────────┐     ┌────────┐     ┌─────────┐
  │ Tokenizer │ ──▶ │ Parser │ ──▶ │   AST   │
  └──────────┘     └────────┘     └─────────┘
  src/syntax.rs    src/syntax.rs   Program, ListItem, ...
       │
       │  (at execution time, per-word)
       ▼
  ┌────────────┐     ┌────────────────┐     ┌────────────┐
  │ expand_raw │ ──▶ │ Field Splitting │ ──▶ │  Pathname  │ ──▶ Vec<String>
  └────────────┘     └────────────────┘     │  Expansion  │
  src/expand.rs      src/expand.rs          └────────────┘
```

Tokenization and parsing happen **once** for a given source string. Expansion happens **at execution time**, word by word, as the executor walks the AST.

---

## Phase 1: Entry Points

The shell receives input through several paths, all of which converge on the same tokenizer/parser.

### Command string (`-c`)

`meiksh -c "echo hello"` stores the string in `options.command_string`. The `Shell::run` method passes it to `run_source`, which calls `execute_source_incrementally`.

### Script file

`meiksh script.sh` reads the entire file into a `String` via `sys::read_file`, then passes it to `run_source`.

### Interactive mode

`interactive::run_loop` reads one line at a time via `read_line()`, then calls `shell.execute_string(&line)`.

### Non-interactive stdin

`run_standard_input` reads byte-by-byte from fd 0, accumulating lines. It attempts trial parses and buffers more input when the parse is incomplete, then calls `run_source_buffer` when complete.

### Dot/source builtin

`shell.source_path(path)` reads the file and calls `execute_string`.

### The convergence point

All paths reach `execute_source_incrementally`:

```rust
fn execute_source_incrementally(&mut self, source: &str) -> Result<i32, ShellError> {
    let mut session = syntax::ParseSession::new(source)?;
    let mut status = 0;
    while let Some(item) = session.next_item(&self.aliases)? {
        status = self.execute_program(&Program { items: vec![item] })?;
        // ...
    }
    Ok(status)
}
```

This creates a `ParseSession`, which tokenizes the source **once**, then yields one `ListItem` at a time. The shell's current alias table is passed each time, so aliases defined by earlier commands take effect on later commands in the same source.

---

## Phase 2: Tokenization

**Function:** `tokenize(source: &str) -> Result<Tokenized, ParseError>`

The tokenizer converts a raw `&str` into a flat stream of tokens. It is a single-pass, character-by-character scanner.

### Intermediate structures

```rust
struct Token {
    kind: TokenKind,
}

struct Tokenized {
    tokens: Vec<Token>,
    here_docs: VecDeque<HereDoc>,
}
```

`Tokenized` bundles the token stream with any here-document bodies collected during tokenization.

### Token kinds

```rust
enum TokenKind {
    Word(String),   // any word — commands, arguments, reserved words, assignments
    Newline,        // \n
    Semi,           // ;
    DSemi,          // ;;
    Amp,            // &
    Pipe,           // |
    AndIf,          // &&
    OrIf,           // ||
    LParen,         // (
    RParen,         // )
    Less,           // <
    Greater,        // >
    DGreat,         // >>
    DLess,          // <<
    DLessDash,      // <<-
    LessAnd,        // <&
    GreatAnd,       // >&
    LessGreat,      // <>
    Clobber,        // >|
    Eof,            // end of input sentinel
}
```

All words — command names, arguments, variable names, reserved words like `if`/`then`/`done` — are the same `Word(String)` variant. The parser differentiates them contextually.

### How the tokenizer works

The tokenizer accumulates characters into a `current: String` buffer. When it encounters a token boundary (whitespace, operator, newline), it flushes the buffer as a `Word` token.

| Input character | Behavior |
|---|---|
| Space, tab, carriage return | Flush current word, skip |
| `\n` | Flush word, emit `Newline`, collect pending here-doc bodies |
| `#` at word start | Skip the rest of the line (comment) |
| `'` | Accumulate until closing `'`. Everything inside is literal. |
| `"` | Call `scan_dquote_body`. Inside double quotes, `\`, `$`, and `` ` `` are still active. |
| `\` | Escape the next character (append both to `current`) |
| `$` followed by `(`, `{` | Call `scan_dollar_construct` which tracks nesting to keep the entire `$(...)`, `$((...))`, or `${...}` as one word |
| `` ` `` | Call `scan_backtick_body` to keep the entire backtick substitution as one word |
| `;`, `&`, `\|`, `(`, `)` | Flush word, emit the appropriate operator token. Multi-character operators (`;;`, `&&`, `\|\|`) are recognized by peeking ahead. |
| `<`, `>` | Flush word, emit the appropriate redirection operator. Multi-character operators (`<<`, `<<-`, `<&`, `<>`, `>>`, `>&`, `>\|`) recognized by lookahead. |
| Anything else | Append to `current` |

### Key design decisions

1. **Words are opaque.** The tokenizer preserves `$`, quotes, backslashes, and expansion syntax verbatim inside `Word.raw`. It only tracks nesting to know where word boundaries are.

2. **Here-doc bodies are collected at tokenization time.** When the tokenizer sees `<<` or `<<-`, it queues a pending here-doc. On the next newline, it reads lines until the delimiter is found, storing the body in a `HereDoc` struct. The parser later attaches these to the corresponding `Redirection` nodes.

3. **Alias expansion is deferred to the parser.** The tokenizer does not know about aliases.

---

## Phase 3: Parsing

**Function:** `Parser::parse_program_until(...) -> Result<Program, ParseError>`

The parser consumes the token stream and builds the AST. It is a recursive-descent parser that matches the POSIX shell grammar.

### The Parser struct

```rust
struct Parser {
    tokens: Vec<Token>,
    here_docs: VecDeque<HereDoc>,
    aliases: HashMap<String, String>,
    alias_expand_next_word_at: Option<usize>,
    alias_expansions_remaining: usize,   // depth guard, starts at 1024
    index: usize,
}
```

The parser walks `tokens` using `index`. It consumes `here_docs` as it encounters `HereDoc` redirections. Alias expansion re-tokenizes the alias value and splices new tokens into the stream.

### Grammar hierarchy

The parser's recursive descent mirrors the POSIX grammar:

```
parse_program_until
 └── loop:
      ├── expand_alias_at_command_start
      ├── parse_and_or ──────────────────────── AndOr
      │    └── parse_pipeline ───────────────── Pipeline
      │         ├── consume_bang (negation)
      │         └── parse_command ───────────── Command
      │              ├── "if" → parse_if
      │              ├── "while"/"until" → parse_loop
      │              ├── "for" → parse_for
      │              ├── "case" → parse_case
      │              ├── "(" → parse_subshell
      │              ├── "{" → parse_group
      │              ├── "name()" → function_def
      │              └── otherwise → parse_simple_command
      │                   ├── collect assignments (NAME=VALUE)
      │                   ├── collect words
      │                   └── collect redirections
      ├── consume_amp → asynchronous flag
      └── → ListItem { and_or, asynchronous }
           → Program { items: Vec<ListItem> }
```

### Alias expansion

When the parser encounters a `Word` token at command-start position, it checks the alias table. If matched:

1. The alias value is re-tokenized.
2. The resulting tokens are spliced into the token stream at the current position.
3. If the alias value ends with whitespace, the next word is also eligible for alias expansion.
4. A depth guard (1024) prevents infinite recursion.

---

## The AST

The AST is the output of parsing and the input to execution. Here are all the node types.

### `Program`

The root node. A sequence of list items.

```rust
struct Program {
    items: Vec<ListItem>,
}
```

### `ListItem`

One entry in the command list. The `asynchronous` flag is `true` when terminated by `&`.

```rust
struct ListItem {
    and_or: AndOr,
    asynchronous: bool,
}
```

### `AndOr`

A chain of pipelines joined by `&&` or `||`.

```rust
struct AndOr {
    first: Pipeline,
    rest: Vec<(LogicalOp, Pipeline)>,
}

enum LogicalOp { And, Or }
```

### `Pipeline`

One or more commands joined by `|`. The `negated` flag is `true` when preceded by `!`.

```rust
struct Pipeline {
    negated: bool,
    commands: Vec<Command>,
}
```

### `Command`

The central enum. Every command form in the shell language is a variant:

```rust
enum Command {
    Simple(SimpleCommand),
    Subshell(Program),                          // ( ... )
    Group(Program),                             // { ...; }
    FunctionDef(FunctionDef),                   // name() body
    If(IfCommand),
    Loop(LoopCommand),                          // while/until
    For(ForCommand),
    Case(CaseCommand),
    Redirected(Box<Command>, Vec<Redirection>), // compound command with redirections
}
```

### `SimpleCommand`

The most common command form. Contains prefix assignments, words (command name + arguments), and redirections — all interleaved in the source but separated by the parser.

```rust
struct SimpleCommand {
    assignments: Vec<Assignment>,
    words: Vec<Word>,
    redirections: Vec<Redirection>,
}
```

### `Word`

The bridge between parsing and expansion. A `Word` holds the raw shell text with all quoting and expansion syntax intact.

```rust
struct Word {
    raw: String,
}
```

Examples of `raw` values:

| Shell input | `raw` value |
|---|---|
| `hello` | `hello` |
| `'hello world'` | `'hello world'` |
| `"$HOME/bin"` | `"$HOME/bin"` |
| `${VAR:-default}` | `${VAR:-default}` |
| `` `date` `` | `` `date` `` |
| `$((1+2))` | `$((1+2))` |

The quotes and `$` are **not** interpreted during parsing. They are preserved for the expansion phase.

### `Assignment`

```rust
struct Assignment {
    name: String,     // the variable name (unquoted)
    value: Word,      // the right-hand side (still raw)
}
```

### `Redirection`

```rust
struct Redirection {
    fd: Option<i32>,
    kind: RedirectionKind,
    target: Word,
    here_doc: Option<HereDoc>,
}

enum RedirectionKind {
    Read,         // <
    Write,        // >
    ClobberWrite, // >|
    Append,       // >>
    HereDoc,      // << or <<-
    ReadWrite,    // <>
    DupInput,     // <&
    DupOutput,    // >&
}
```

### `HereDoc`

```rust
struct HereDoc {
    delimiter: String,   // delimiter with quotes stripped
    body: String,        // the inline body text
    expand: bool,        // true unless delimiter was quoted
    strip_tabs: bool,    // true for <<-
}
```

### Compound command types

```rust
struct FunctionDef { name: String, body: Box<Command> }
struct IfCommand { condition: Program, then_branch: Program,
                   elif_branches: Vec<ElifBranch>, else_branch: Option<Program> }
struct ElifBranch { condition: Program, body: Program }
struct LoopCommand { kind: LoopKind, condition: Program, body: Program }
enum LoopKind { While, Until }
struct ForCommand { name: String, items: Option<Vec<Word>>, body: Program }
struct CaseCommand { word: Word, arms: Vec<CaseArm> }
struct CaseArm { patterns: Vec<Word>, body: Program }
```

---

## Phase 4: Expansion

Expansion happens at execution time, not at parse time. The executor calls into `src/expand.rs` to transform `Word` values from the AST into the final `Vec<String>` argument lists.

### The Context trait

Expansion needs access to shell state. Rather than depending on `Shell` directly, it uses a trait:

```rust
trait Context {
    fn env_var(&self, name: &str) -> Option<String>;
    fn special_param(&self, name: char) -> Option<String>;
    fn positional_param(&self, index: usize) -> Option<String>;
    fn positional_params(&self) -> Vec<String>;
    fn set_var(&mut self, name: &str, value: String) -> Result<(), ExpandError>;
    fn nounset_enabled(&self) -> bool;
    fn pathname_expansion_enabled(&self) -> bool;
    fn shell_name(&self) -> &str;
    fn command_substitute(&mut self, command: &str) -> Result<String, ExpandError>;
}
```

`Shell` implements this trait. Tests use a `FakeContext`.

### Public entry points

| Function | Input | Output | Use case |
|---|---|---|---|
| `expand_word(ctx, word)` | `&Word` | `Vec<String>` | General word expansion (command arguments). Full pipeline: expand, field-split, pathname-expand. |
| `expand_words(ctx, words)` | `&[Word]` | `Vec<String>` | Batch version of `expand_word`, concatenating results. |
| `expand_word_text(ctx, word)` | `&Word` | `String` | Single-string expansion. No field splitting, no globbing. Used for assignment values. |
| `expand_here_document(ctx, body)` | `&str` | `String` | Here-doc body expansion. Only `$` and `\` are active; no quotes, no splitting, no globbing. |

### Internal data structures

#### `Segment`

The fundamental unit of intermediate expansion results.

```rust
enum Segment {
    Text(String, bool),   // (content, is_quoted)
    AtBreak,              // field boundary from "$@"
    AtEmpty,              // "$@" with zero positional parameters
}
```

The `bool` in `Text` tracks whether the content is **quoted** (came from `'...'`, `"..."`, `\x`, or `$'...'`). Quoted text is immune to field splitting and pathname expansion. This is how meiksh implements POSIX quote removal — the quote characters themselves are stripped during `expand_raw`, and the quoted/unquoted distinction is carried as metadata on each segment.

#### `Expansion`

The result of expanding a single `$`-expression.

```rust
enum Expansion {
    One(String),              // a single value (most parameters)
    AtFields(Vec<String>),    // separate fields from "$@"
}
```

#### `ExpandedWord`

The intermediate result of `expand_raw` before field splitting.

```rust
struct ExpandedWord {
    segments: Vec<Segment>,
    had_quoted_content: bool,   // any quoting in the original word?
    has_at_expansion: bool,     // was "$@" expanded?
}
```

#### `Field`

The result of field splitting, before pathname expansion.

```rust
struct Field {
    text: String,
    has_unquoted_glob: bool,   // contains unquoted *, ?, or [
}
```

### The expansion pipeline in detail

Here is what happens inside `expand_word`:

```
Word { raw: "\"pre${X}\"suf*" }
              │
              ▼
         expand_raw(ctx, raw)
              │  character-by-character scan
              │  produces Vec<Segment>
              ▼
         ExpandedWord {
           segments: [
             Text("pre", true),        ← from inside "..."
             Text("value", true),      ← ${X} expanded, still in "..."
             Text("suf", false),       ← outside quotes
             Text("*", false),         ← unquoted glob char
           ],
           had_quoted_content: true,
           has_at_expansion: false,
         }
              │
              ├─── has_at_expansion? ──▶ expand_word_with_at_fields()
              │                          (split at AtBreak markers)
              │
              ├─── segments empty + had_quoted_content? ──▶ vec![""]
              │                                             (empty quotes = one empty field)
              │
              ├─── segments empty + !had_quoted_content? ──▶ vec![]
              │                                              (truly empty = no fields)
              │
              ├─── all segments quoted? ──▶ flatten_segments()
              │                             vec!["prevaluesuf*"]
              │                             (no splitting, no globbing)
              │
              └─── has unquoted segments:
                        │
                        ▼
                   split_fields_from_segments(segments, IFS)
                        │  IFS defaults to " \t\n"
                        │  quoted text: never a separator
                        │  unquoted IFS chars: field boundaries
                        ▼
                   Vec<Field> [
                     Field { text: "prevaluesuf*", has_unquoted_glob: true }
                   ]
                        │
                        ▼
                   for each field with has_unquoted_glob:
                     expand_pathname(text) → sorted matches, or literal if none
                        │
                        ▼
                   Vec<String> — the final result
```

### `expand_raw`: the character-by-character scanner

`expand_raw` walks `Word.raw` one character at a time and builds `Vec<Segment>`:

| Character | Context | Behavior |
|---|---|---|
| `'` | unquoted | Sets `had_quoted_content`. Collects everything until closing `'` as **quoted** text. No interpretation inside. |
| `"` | unquoted | Sets `had_quoted_content`. Enters double-quote mode. Inside, `\`, `$`, and `` ` `` are active; everything else is **quoted** text. |
| `\` | unquoted | Next character becomes **quoted** text (immune to splitting/globbing). |
| `\` | inside `"..."` | Only `$`, `` ` ``, `"`, `\`, and newline are escaped. Other `\x` sequences produce both characters as quoted text. |
| `$` | unquoted | Calls `expand_dollar(ctx, chars, quoted=false)`. Result is **unquoted** (subject to splitting/globbing). Exception: `$'...'` produces **quoted** text. |
| `$` | inside `"..."` | Calls `expand_dollar(ctx, chars, quoted=true)`. Result is **quoted**. This is where `$@` can return `AtFields`. |
| `` ` `` | either | Extracts the backtick command via `scan_backtick_command`, calls `ctx.command_substitute()`, strips trailing newlines. Result is quoted if inside `"..."`, unquoted otherwise. |
| `~` | position 0, unquoted | Tilde expansion: replaces with `$HOME`. The result is **unquoted**. |
| anything else | either | Appended to the current segment with the current quotedness. |

### `expand_dollar`: the `$`-expression dispatcher

`expand_dollar` takes a character slice starting at `$` and returns `(Expansion, chars_consumed)`:

| Pattern | Result |
|---|---|
| `$'...'` | ANSI-C quoting (`\n`, `\t`, `\x41`, `\u0041`, etc.) → `One(string)` |
| `${...}` | Braced parameter expansion (default, assign, error, alternate, length, pattern removal) → `One(string)` |
| `$((...))` | Arithmetic expansion (`+`, `-`, `*`, `/`, `%`) → `One(string)` |
| `$(...)` | Command substitution → `One(string)` |
| `$@` (quoted) | `AtFields(ctx.positional_params())` — each parameter is a separate field |
| `$@` (unquoted) | Positional params joined with space → `One(string)` |
| `$*` (quoted) | Positional params joined with IFS[0] → `One(string)` |
| `$*` (unquoted) | Positional params joined with space → `One(string)` |
| `$?`, `$$`, `$!`, `$#`, `$-` | Special parameters → `One(string)` |
| `$0` | Shell name → `One(string)` |
| `$1`–`$9` | Single-digit positional parameters → `One(string)` |
| `$name` | Named variable lookup → `One(string)` |
| `$` followed by anything else | Literal `"$"` → `One("$")` |

### `apply_expansion`: converting `Expansion` to segments

```rust
fn apply_expansion(segments, expansion, quoted, has_at) {
    match expansion {
        One(s) => push_segment(segments, s, quoted),
        AtFields(params) => {
            *has_at = true;
            if params.is_empty() {
                segments.push(AtEmpty);
            } else {
                for (i, param) in params.into_iter().enumerate() {
                    if i > 0 { segments.push(AtBreak); }
                    push_segment(segments, param, true);
                }
            }
        }
    }
}
```

`AtFields` inserts `AtBreak` markers between each positional parameter so that `expand_word_with_at_fields` can later split on them.

### Field splitting

`split_fields_from_segments` implements POSIX IFS field splitting:

1. **IFS empty:** No splitting. All segments are flattened into one field.
2. **IFS set:** Characters are classified as IFS-whitespace (`' '`, `'\t'`, `'\n'`) or IFS-other (everything else in IFS).
3. Each `Segment::Text` is flattened to `Vec<(char, bool)>` (character + is-quoted).
4. Walk through characters:
   - **Quoted characters** are never separators; they accumulate into the current field.
   - **Unquoted IFS-other characters** always delimit. Two consecutive IFS-other chars produce an empty field between them.
   - **Unquoted IFS-whitespace characters** delimit only when there is accumulated content. Multiple consecutive whitespace chars act as one delimiter.
5. Each resulting `Field` tracks `has_unquoted_glob` (set if any unquoted `*`, `?`, or `[` was seen).

### Pathname expansion

`expand_pathname` takes a pattern string and returns sorted matches:

1. If no glob characters (`*`, `?`, `[`) are present, returns the pattern as-is.
2. Splits the pattern by `/` into path segments.
3. Recursively expands each segment by reading directories and matching entries against the pattern using `pattern_matches`.
4. Dotfiles are hidden unless the pattern segment starts with `.`.
5. Results are sorted alphabetically.
6. If no matches are found, the original literal pattern is returned.

### Quote removal

Quote removal in meiksh is **implicit** rather than being a separate pass. When `expand_raw` processes quote characters (`'`, `"`, `\`), it strips them and stores the enclosed content as `Segment::Text(content, quoted=true)`. By the time segments are flattened into final strings, the quote characters are already gone. The `bool` on each segment only affects field splitting and globbing decisions — it does not survive into the output.

The helper `push_segment` coalesces adjacent segments with the same quotedness:

```rust
fn push_segment(segments, text, quoted) {
    if let Some(Text(last, last_quoted)) = segments.last_mut() {
        if *last_quoted == quoted {
            last.push_str(&text);
            return;
        }
    }
    segments.push(Text(text, quoted));
}
```

And `flatten_segments` discards the quoted flag entirely:

```rust
fn flatten_segments(segments) -> String {
    segments.iter()
        .filter_map(|seg| match seg { Text(part, _) => Some(part.as_str()), _ => None })
        .collect()
}
```

---

## Complete Pipeline Summary

```
                    ┌─────────────────────────────────────────────┐
                    │         Raw source text (&str)              │
                    └──────────────────┬──────────────────────────┘
                                       │
                    PHASE 1 ───────────┼──────────────────────────
                                       │
                    ┌──────────────────┴──────────────────────────┐
                    │  tokenize(source)                           │
                    │  Single-pass, char-by-char scan.            │
                    │  Tracks nesting for $(), ${}, ``, quotes.   │
                    │  Collects here-doc bodies on newlines.      │
                    │                                             │
                    │  Output: Tokenized {                        │
                    │    tokens: Vec<Token>,     ← flat stream    │
                    │    here_docs: VecDeque<HereDoc>,            │
                    │  }                                          │
                    └──────────────────┬──────────────────────────┘
                                       │
                    PHASE 2 ───────────┼──────────────────────────
                                       │
                    ┌──────────────────┴──────────────────────────┐
                    │  Parser::parse_program_until(...)            │
                    │  Recursive descent, POSIX grammar.          │
                    │  Alias expansion splices tokens inline.     │
                    │  Reserved words recognized contextually.    │
                    │                                             │
                    │  Output: Program { items: Vec<ListItem> }   │
                    └──────────────────┬──────────────────────────┘
                                       │
              The AST contains Word { raw } nodes with unexpanded text.
              Everything below happens at execution time, per-word.
                                       │
                    PHASE 3 ───────────┼──────────────────────────
                                       │
                    ┌──────────────────┴──────────────────────────┐
                    │  expand_raw(ctx, &word.raw)                 │
                    │  Char-by-char scan of the raw word text.    │
                    │  Processes: ' " \ $ ` ~                     │
                    │  Calls expand_dollar for $-expressions.     │
                    │  Calls command_substitute for $() and ``.   │
                    │                                             │
                    │  Output: ExpandedWord {                     │
                    │    segments: Vec<Segment>,                  │
                    │    had_quoted_content: bool,                │
                    │    has_at_expansion: bool,                  │
                    │  }                                          │
                    └──────────────────┬──────────────────────────┘
                                       │
               ┌───────────────────────┼──────────────────────────┐
               │                       │                          │
      has "$@"?              all quoted?              has unquoted?
               │                       │                          │
               ▼                       ▼                          ▼
    split on AtBreak         flatten_segments       split_fields_from_segments
    markers                  → one string           (IFS-based splitting)
               │                       │                          │
               │                       │                          ▼
               │                       │               Vec<Field { text,
               │                       │                 has_unquoted_glob }>
               │                       │                          │
               │                       │                          ▼
               │                       │               expand_pathname (globbing)
               │                       │               for fields with unquoted
               │                       │               glob characters
               │                       │                          │
               └───────────────────────┴──────────────────────────┘
                                       │
                                       ▼
                              Vec<String>
                              (final expanded arguments)
```
