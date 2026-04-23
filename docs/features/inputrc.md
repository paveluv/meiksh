# Inputrc File Format

## Status

**Not implemented.** This document is a specification; the inputrc parser described here has no corresponding implementation in meiksh. No file at `$INPUTRC`, `$HOME/.inputrc`, or `/etc/inputrc` is consulted at shell startup, and no `bind -f` builtin is available to load one on demand. Implementation is part of [emacs-editing-mode.md](emacs-editing-mode.md) Stage C as described in the project plan.

## 1. Scope

This document specifies the grammar, escape syntax, recognized variables, and conditional directives accepted by meiksh when it parses an inputrc file. An inputrc file is a text configuration file that configures the emacs editing mode (see [emacs-editing-mode.md](emacs-editing-mode.md)) by binding keys to bindable functions, defining key-to-string macros, and setting editor variables.

No external standard defines the inputrc syntax; the reference implementation is GNU Readline 8.x as documented in the readline and bash manuals. Meiksh implements a deliberate subset. Files that use only the grammar below shall be accepted. Files that use features outside this subset shall produce per-line diagnostics but shall otherwise continue to be parsed so that lines using only the accepted subset still take effect.

### 1.1 Conformance Language

The key words "shall", "shall not", "should", "should not", "may", and "must" in this document are to be interpreted as described in RFC 2119.

### 1.2 Where Inputrc Files Come From

Meiksh reads an inputrc file in three contexts:

1. At shell startup, when emacs mode is enabled, from the locations listed in [emacs-editing-mode.md Section 13.2](emacs-editing-mode.md).
2. When the `bind -f filename` builtin is invoked.
3. Implicitly, line-by-line, when the `bind` builtin receives a single-argument form (the argument is parsed as one inputrc line).

The grammar below applies identically in all three contexts.

## 2. Lexical Structure

### 2.1 Lines

- A line is terminated by a newline (`\n`) or carriage return (`\r`) or end-of-file.
- Line continuation via a trailing backslash is not supported; each line shall be independently parseable.
- Leading and trailing ASCII whitespace (SPACE and TAB) on a line shall be discarded.

### 2.2 Comments

- A line whose first non-whitespace character is `#` shall be treated as a comment and ignored.
- In-line (end-of-line) comments are not supported. A `#` appearing after non-whitespace content on a line is a regular character.

### 2.3 Blank Lines

- A line containing only whitespace shall be ignored.

## 3. Directives

Each non-comment, non-blank line shall be exactly one of the following directives. Parsing of a line that does not match any of these forms shall produce a diagnostic (see Section 7).

1. `set <variable> <value>` - Section 5.
2. `$if <test>`, `$else`, `$endif`, `$include <path>` - Section 6.
3. Key-binding form - Section 4.

## 4. Key Bindings

### 4.1 Two Forms

Key bindings shall use one of two forms:

- **Keyname form**: `<keyname>: <function-or-macro>`
- **Key-sequence form**: `"<quoted-sequence>": <function-or-macro>`

The right-hand side may be either:

- A bindable function name (an identifier consisting of lowercase letters, digits, and hyphens).
- A double-quoted macro string (`"<quoted-string>"`), which shall be re-fed through the editor dispatch loop byte-by-byte when the key is pressed.

### 4.2 Keyname Form

The keyname on the left-hand side shall be one of the following case-insensitive tokens, optionally preceded by a `Control-` or `C-` prefix, a `Meta-` or `M-` prefix, or both. Prefixes shall be applied left to right.

| Token | Byte(s) |
|---|---|
| `Return` / `Newline` / `RET` | `\r` / `\n` |
| `Escape` / `ESC` | `\x1b` |
| `Tab` / `TAB` | `\t` |
| `Rubout` / `DEL` | `\x7f` |
| `Space` / `SPC` | `\x20` |
| `LFD` | `\n` |
| A single printable ASCII character | that byte |

Examples:

```text
Control-a: beginning-of-line
C-a: beginning-of-line
Meta-Rubout: backward-kill-word
M-C-h: backward-kill-word
```

A keyname form shall produce a key sequence of one or two bytes. Keynames that would expand to longer sequences shall be a parse error.

### 4.3 Key-Sequence Form

The quoted sequence on the left-hand side shall be a double-quoted string literal interpreted per the escape syntax in Section 4.5. The resulting byte sequence shall be the key sequence to bind.

Examples:

```text
"\C-a": beginning-of-line
"\e[A": history-search-backward
"\C-x\C-r": re-read-init-file
"\ei": "echo hello\n"
```

### 4.4 Function-Name Right-Hand Side

- A bare identifier on the right-hand side shall be interpreted as a bindable function name.
- Names not in the meiksh bindable-function set (see [emacs-editing-mode.md Section 5](emacs-editing-mode.md)) shall produce a per-line diagnostic and shall not bind anything.
- A trailing newline after the identifier is permitted.

### 4.5 String Escape Syntax

Inside a double-quoted string (both on the left-hand side of a key-sequence form and on the right-hand side of a macro), the following escape sequences shall be recognized.

| Escape | Meaning |
|---|---|
| `\\` | Literal backslash |
| `\"` | Literal double quote |
| `\'` | Literal single quote |
| `\a` | Alert / bell (0x07) |
| `\b` | Backspace (0x08) |
| `\d` | Delete (0x7F) |
| `\e` | Escape (0x1B) |
| `\f` | Form feed (0x0C) |
| `\n` | Newline / line feed (0x0A) |
| `\r` | Carriage return (0x0D) |
| `\t` | Horizontal tab (0x09) |
| `\v` | Vertical tab (0x0B) |
| `\NNN` | Byte with the given 1-to-3-digit octal value (values exceeding 0xFF shall be an error) |
| `\xHH` | Byte with the given 1-to-2-digit hexadecimal value |
| `\C-<x>` | Byte `x & 0x1F` (i.e. Control-x). `x` may itself be an escape such as `\M-...` (which expands to an ESC-prefix sequence, and `\C-` then applies to the final byte). |
| `\M-<x>` | Two-byte sequence: `ESC` followed by the byte for `x`. `x` may itself be `\C-<y>` or other escape. |

Any other `\`-prefixed byte shall be an error.

Note: the `bind` builtin (see [emacs-editing-mode.md § 14.5](emacs-editing-mode.md)) additionally tolerates `\E` (uppercase) as a synonym for `\e`, and `^<x>` caret notation, when invoked in its editline-compatibility positional form. Those tolerances are builtin-scoped; inputrc files shall use the escape table above and reject `\E` / `^X` as errors.

### 4.6 Macro Right-Hand Side

A double-quoted string on the right-hand side is a macro. When the bound key sequence is received, meiksh shall feed the bytes of the macro string through the editor dispatch loop one at a time, as if the user had typed them.

Example:

```text
"\C-xg": "git status\n"
```

When `C-x g` is pressed, the bytes `g i t SPACE s t a t u s NEWLINE` shall be dispatched in sequence. Each byte is subject to normal keymap lookup; in particular, a macro string that contains a control byte shall have that byte interpreted as its bound function, not as a literal insert. Macro expansion shall not recurse: a key whose macro contains its own bound key shall produce a diagnostic and the macro shall be truncated at the recursive byte.

## 5. Variables

Variable assignment shall use the form:

```text
set <name> <value>
```

where `<name>` is one of the recognized variable names below and `<value>` is parsed according to the variable's type:

- **Boolean**: the strings `on`, `off`, `true`, `false`, `yes`, `no`, `1`, `0` (case-insensitive). Any other value shall be a diagnostic; the variable shall retain its previous value.
- **Integer**: a decimal integer. Negative values shall be rejected unless the variable documents otherwise.
- **String**: the rest of the line after the variable name, with leading and trailing whitespace trimmed. No quoting is performed.
- **Enumeration**: one of a fixed set of tokens, documented per-variable.

### 5.1 Recognized Variables

| Name | Type | Default | Effect |
|---|---|---|---|
| `bell-style` | Enumeration: `none`, `audible`, `visible` | `audible` | Controls how the editor signals errors. `none` suppresses the bell; `audible` writes `\a` to the terminal; `visible` is treated as `audible` (no visible-bell implementation). |
| `completion-ignore-case` | Boolean | `off` | When on, case differences between the partial word and a completion candidate shall not prevent a match. |
| `completion-map-case` | Boolean | `off` | When on and `completion-ignore-case` is also on, hyphens and underscores shall be treated as equivalent for matching purposes. |
| `show-all-if-ambiguous` | Boolean | `off` | When on, a single `TAB` on a partial word with multiple completions shall list them immediately rather than requiring a second `TAB`. |
| `show-all-if-unmodified` | Boolean | `off` | When on, a single `TAB` that adds no characters shall immediately list candidates. |
| `enable-bracketed-paste` | Boolean | `on` | When on, the editor emits `\e[?2004h` on entry and `\e[?2004l` on exit, and treats content between `\e[200~` and `\e[201~` as a literal insert. |
| `editing-mode` | Enumeration: `emacs`, `vi` | (see below) | Equivalent to `set -o emacs` or `set -o vi` respectively. The default is the mode enabled at the time the inputrc is read. |
| `history-size` | Integer | 500 | Maximum number of entries retained in memory history. The shell's `HISTSIZE` environment variable, when set, takes precedence. |
| `mark-symlinked-directories` | Boolean | `off` | When on, a completed filename that is a symbolic link to a directory shall be marked with a trailing `/`, matching plain directories. |
| `colored-stats` | Boolean | `off` | When on, completion listings shall color filenames by type using the same rules as `ls --color`. |
| `keyseq-timeout` | Integer (milliseconds) | 500 | After receiving an ambiguous prefix (for example, bare `ESC`), the editor shall wait up to this many milliseconds for additional bytes before treating the prefix as a complete key sequence. A value of 0 shall disable the timeout (treat every ambiguous prefix as complete immediately). |
| `comment-begin` | String | `#` | The string inserted by `insert-comment` (not bound by default; provided only for inputrc compatibility). |
| `input-meta` / `meta-flag` | Boolean | `off` | Accepted for inputrc compatibility so that shipped distribution inputrc files load without diagnostics. Meiksh recognizes meta only in its ESC-prefix form (see [emacs-editing-mode.md Section 4](emacs-editing-mode.md)); the value is stored but has no runtime effect. |
| `output-meta` | Boolean | `off` | Accepted for inputrc compatibility; stored without runtime effect. See `input-meta`. |

### 5.2 Unrecognized Variables

Variables not in the table above are listed as non-goals in [emacs-editing-mode.md Section 15](emacs-editing-mode.md). A `set <unknown> ...` directive shall:

- Produce a per-line diagnostic on standard error with the format `meiksh: <file>: line <n>: unknown variable: <unknown>`.
- Not abort parsing; subsequent lines shall be parsed independently.
- Not modify editor state.

## 6. Conditional Directives

### 6.1 `$if` / `$else` / `$endif`

The `$if <test>` directive begins a conditional block terminated by `$endif`, with an optional `$else` branch. Only one of the two branches shall be active per invocation.

- `$if`, `$else`, and `$endif` shall each appear on their own line (leading whitespace is allowed; trailing content is a diagnostic).
- Nesting of conditional blocks shall be supported to any depth.
- The `$if` test shall be one of:

| Test | Evaluates true when |
|---|---|
| `mode=emacs` | The inputrc file is being parsed in the context of emacs editing mode. |
| `mode=vi` | The inputrc file is being parsed in the context of vi editing mode. |
| `term=<name>` | The `TERM` environment variable (captured at the time parsing began) equals `<name>`, OR the portion of `TERM` up to the first `-` equals `<name>`. If `TERM` is unset or empty the test evaluates false. Matching is byte-exact; there is no case folding. |

No other `$if` test is recognized. `$if application=<name>`, `$if variable=value`, and `$if version>=<n>` are non-goals (see [emacs-editing-mode.md Section 15.14](emacs-editing-mode.md)). An unrecognized test shall:

- Produce a per-line diagnostic.
- Treat the block as if the test evaluated to false. The `$else` branch, if present, shall be parsed.

A well-formed `term=<name>` test that simply evaluates false (because the terminal does not match) shall **not** produce a diagnostic; only unrecognized test forms do.

### 6.2 `$include`

```text
$include <path>
```

- `<path>` shall be the rest of the line after `$include` and one or more whitespace characters, with leading and trailing whitespace trimmed. No quoting is performed; a `#` in `<path>` is literal.
- The path may be absolute or relative; relative paths shall be resolved against the directory of the file currently being parsed.
- `$INPUTRC` and `$HOME` shall not be expanded inside the path.
- If the included file cannot be opened, a diagnostic shall be produced and parsing of the including file shall continue at the line after the `$include`.
- Included files shall be parsed with the same grammar.
- `$include` shall not recurse on itself: if a file transitively `$include`s itself, the second inclusion shall produce a diagnostic and shall not re-parse the file.

## 7. Error Handling

- Every parse error shall produce a diagnostic on standard error with the format:

  ```
  meiksh: <file>: line <n>: <diagnostic>
  ```

  where `<file>` is the pathname passed to the parser (or `-` for the `bind` single-argument form), `<n>` is the 1-based line number within `<file>`, and `<diagnostic>` is a short human-readable message.

- Parse errors shall be non-fatal: after reporting the error, the parser shall advance to the next line boundary and continue.

- A file that produces one or more diagnostics shall still be considered "read" for the purpose of determining which of the fallback paths (`$INPUTRC`, `$HOME/.inputrc`, `/etc/inputrc`) to consult; that is, the shell shall not fall through to the next path just because the first one contained errors.

## 8. File Lookup Order

See [emacs-editing-mode.md Section 13.2](emacs-editing-mode.md).

## 9. Non-Normative Appendix - Worked Examples

### 9.1 Minimal Useful Inputrc

```text
# Use TAB twice to list completions; show them on first TAB for ambiguity.
set show-all-if-ambiguous on
set completion-ignore-case on

# Disable the audible bell.
set bell-style none
```

### 9.2 Arrow-Key History Search

```text
# Up/Down arrows search history by the currently typed prefix.
"\e[A": history-search-backward
"\e[B": history-search-forward
```

### 9.3 Macros For Common Commands

```text
# Ctrl-x g runs `git status` and accepts the line.
"\C-xg": "git status\n"

# Meta-i inserts a rare option.
"\ei": "--interactive "
```

### 9.4 Mode-Conditional Block

```text
$if mode=emacs
  # Bind Ctrl-w to a safer backward-kill-word in emacs mode.
  "\C-w": backward-kill-word
$endif

$if mode=vi
  # Nothing; fall through.
$endif
```

### 9.5 Per-Terminal Bindings

```text
# rxvt-family terminals emit a slightly different sequence for
# Ctrl-Right; rebind only when the shell is running under one.
$if term=rxvt
  "\eOc": forward-word
  "\eOd": backward-word
$endif
```

### 9.6 Including A Shared File

```text
# At the top of ~/.inputrc:
$include /etc/meiksh/inputrc.shared

# Personal overrides below:
set bell-style none
```
