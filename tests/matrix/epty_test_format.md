# `.epty` Test Format Reference

This document describes the `.epty` test file format used by `expect_pty`.
Read this before writing or modifying tests.

## File structure

Every `.epty` file starts with a `testsuite` declaration, followed by
requirement directives and test blocks.

```
testsuite "Suite Name"

requirement "REQ-ID" doc="Short description of the requirement."
begin test "descriptive test name"
  script
    echo hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "descriptive test name"
```

- One `testsuite` per file. The name must be unique across all files.
- Comments (`#`) and blank lines are allowed between blocks.
- Trailing whitespace is **forbidden** on any line.

## Requirement directives

```
requirement "SHALL-2-2-1-002" doc="A <backslash> that is not quoted shall..."
```

- Placed immediately before the test(s) that cover them.
- Multiple requirements can precede a single test if the test covers
  several obligations.
- The `doc` attribute is a brief human-readable summary.
- Every requirement referenced in `requirements.json` must appear in
  at least one `.epty` file, and every requirement linked to a test
  must be present in `requirements.json`.

## Non-interactive tests

Non-interactive tests run a shell script and check its output/exit code.
Each test contains a `script` block and an `expect` block.

```
begin test "name"
  script
    <shell code>
  expect
    stdout "pattern"
    stderr "pattern"
    exit_code <expr>
end test "name"
```

### Script block

- `script` is at column 2. Script body is at column 4 (stripped to
  column 0 at runtime).
- The body is taken **verbatim** — no quoting or escaping needed.
- `$SHELL` is set to the target shell (e.g. `/usr/bin/bash --posix`).
- The script runs in a clean environment with `HOME`, `TMPDIR`, `PATH`,
  `SHELL`, `LANG=C`, `LC_ALL=C`, etc.
- Tests run in an isolated sandbox working directory; prefer simple relative
  paths (for example `_temp_file`) over `${TMPDIR:-/tmp}` path construction.
- Avoid explicit cleanup-only commands (`rm -f ...`) unless cleanup behavior
  itself is the subject under test; the runner handles sandbox cleanup.

### Expect block

The `expect` block follows immediately after the script body. It
contains exactly three assertions in this fixed order:

1. `stdout "pattern"` — assert stdout matches regex (full match)
2. `stderr "pattern"` — assert stderr matches regex (full match)
3. `exit_code <expr>` — assert exit code satisfies expression

All three are **required** and must appear in exactly this order.
The parser rejects any other ordering or omission.

#### Assertion tips

- `stdout` does a **full match** — the pattern must match the
  entire stdout (trailing whitespace is trimmed). `stdout "hello"`
  only passes if stdout is exactly `hello`.
- `.` matches any character **except** newline. Use `\n` to match across
  lines: `stdout "line1\nline2"`.
- To match a substring within multi-line output, use `(.|\n)*` to cross
  newlines: `stdout "(.|\n)*pattern(.|\n)*"`.
- `stdout ""` asserts stdout is empty.
- `stderr ""` asserts stderr is empty.
- `stderr ".+"` asserts stderr is non-empty.

#### Exit code expressions

`exit_code` accepts an expression that the actual exit code is
tested against:

| Expression | Meaning |
|---|---|
| `0` | exit code equals 0 (bare integer = exact match) |
| `!=0` | exit code is not 0 |
| `>0` | exit code is greater than 0 |
| `>=128` | exit code is 128 or above |
| `(>128 && <256) \|\| 1` | between 129-255 inclusive, or exactly 1 |

Supported operators: `==`, `!=`, `>`, `<`, `>=`, `<=`.
Combinators: `&&` (and), `||` (or). Parentheses for grouping.
`&&` binds tighter than `||`.

### setenv

Set an environment variable for the test's execution:

```
begin test "locale-sensitive test"
  setenv "LC_ALL" "C.UTF-8"
  script
    ...
  expect
    stdout "..."
    stderr ""
    exit_code 0
end test "locale-sensitive test"
```

`setenv` goes between `begin test` and `script`.

## Interactive (PTY) tests

Interactive tests spawn an actual pseudo-terminal and exchange
keystrokes with the shell. Use these for features that require a TTY
(vi editing, job control, prompts, etc.).

```
begin interactive test "name"
  spawn -i
  expect "\\$ "
  send "echo hello"
  expect "hello"
  expect "\\$ "
  sendeof
  wait
end interactive test "name"
```

Note: `begin interactive test` / `end interactive test` (not plain
`begin test` / `end test`).

### Interactive commands

| Command | Description |
|---|---|
| `spawn [flags...]` | Start the target shell. Flags are appended (e.g. `-i` for interactive). |
| `send "text"` | Send text + newline to the PTY. Supports `\"`, `\\`, `\n`, `\r`, `\t`. |
| `sendraw <hex>...` | Send raw bytes. E.g. `sendraw 1b` sends ESC, `sendraw 0a` sends newline. |
| `expect "regex"` | Wait for regex match in PTY output (default timeout 200ms). |
| `expect timeout=2s "regex"` | Wait with custom timeout. |
| `expect_line "regex"` | Wait for a complete line matching regex. |
| `expect_line timeout=1s "regex"` | With custom timeout. |
| `signal SIGNAME` | Send a signal (e.g. `signal SIGTSTP`). |
| `sendeof` | Close the write side of the PTY (sends EOF). |
| `wait` | Wait for child to exit. |
| `wait exitcode=N` | Wait and assert exit code. |
| `sleep <duration>` | Sleep. Must include units: `100ms` or `1s`. |

### Interactive test tips

- Always `expect "\\$ "` (or your prompt regex) before sending the next
  command — this ensures the shell is ready.
- `spawn -i` is the typical invocation for interactive tests.
- `sendraw 1b` sends ESC (for vi-mode). Common hex codes:
  `0a` = newline, `1b` = ESC, `03` = Ctrl-C, `04` = Ctrl-D,
  `1a` = Ctrl-Z.
- End interactive tests with `sendeof` then `wait` (or `wait exitcode=0`).
- Use `sleep 100ms` between `sendraw` sequences to let the shell process
  each keystroke (especially for vi-mode editing).

## Regex syntax

The `.epty` runner uses a built-in regex engine. Supported syntax:

| Syntax | Meaning |
|---|---|
| `.` | Any character except newline |
| `*` | Zero or more of preceding |
| `+` | One or more of preceding |
| `?` | Zero or one of preceding |
| `[abc]` | Character class |
| `[a-z]` | Character range |
| `[[:digit:]]` | POSIX named class |
| `[^abc]` or `[!abc]` | Negated class |
| `(a\|b)` | Alternation group |
| `\` | Escape next character |

**Important**: In patterns, backslash is passed through **raw** to the
regex engine. There is no string-level escaping. To match a literal
backslash, write `\\`. To match `[`, write `\[`. To embed a literal
`"` in a pattern, double it: `""`.

In `send` strings, backslash escapes **are** interpreted: `\"`, `\\`,
`\n`, `\r`, `\t`.

## Script execution modes

Non-interactive tests run via `--script-modes` (default: `dash-c`):

- **dash-c**: `$SHELL -c '<script>'` — passes script as `-c` argument.
- **tempfile**: Writes script to a temp file, runs `$SHELL <file>`.
- **stdin**: Pipes script into `$SHELL` via stdin.

Multiple modes can be specified (`--script-modes dash-c,tempfile`). Each
test runs once per mode. The default (`dash-c`) is sufficient for most
tests.

To run only one test by name from one or more `.epty` files, use
`--test`:

```
cargo run --bin expect_pty -- --shell "/abs/path/to/sh -i" --test "test name" tests/matrix/tests/*.epty
```

## Integrity checking

After writing tests, always validate syntax and matrix integrity:

```
cargo run --bin check_integrity -- tests/matrix
```

This checks:
- Every test referenced in a requirement's `tests` array exists in a
  `.epty` file.
- Every `requirement` directive in `.epty` files has a matching entry in
  `requirements.json`.
- Syntax errors in `.epty` files.

## Common patterns

### Testing stdout output

```
begin test "arithmetic expansion"
  script
    echo $((2 + 3))
  expect
    stdout "5"
    stderr ""
    exit_code 0
end test "arithmetic expansion"
```

### Testing exit codes

```
begin test "false returns non-zero"
  script
    false
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "false returns non-zero"
```

```
begin test "signal exit code range"
  script
    kill -TERM $$
  expect
    stdout ""
    stderr ""
    exit_code >128
end test "signal exit code range"
```

### Testing stderr

```
begin test "syntax error produces stderr"
  script
    $SHELL -c 'if then' 2>&1
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "syntax error produces stderr"
```

### Testing multi-line output

```
begin test "for loop output"
  script
    for i in a b c; do echo $i; done
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "for loop output"
```

### Interactive job control

```
begin interactive test "background job notification"
  spawn -i
  expect "\\$ "
  send "sleep 60 &"
  expect "\\[[[:digit:]]+\\] [[:digit:]]+"
  expect "\\$ "
  send "kill %1"
  expect "\\$ "
  sendeof
  wait
end interactive test "background job notification"
```
