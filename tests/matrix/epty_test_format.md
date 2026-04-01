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
  begin script
    echo hello
  end script
  expect_stdout "hello"
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

```
begin test "name"
  begin script
    <shell code>
  end script
  <assertions>
end test "name"
```

### Script block rules

- `begin script` and `end script` are indented 2 spaces.
- Script body is indented 4 spaces (stripped to column 0 at runtime).
- The body is taken **verbatim** — no quoting or escaping needed.
- `$SHELL` is set to the target shell (e.g. `/usr/bin/bash --posix`).
- The script runs in a clean environment with `HOME`, `TMPDIR`, `PATH`,
  `SHELL`, `LANG=C`, `LC_ALL=C`, etc.
- Tests run in an isolated sandbox working directory; prefer simple relative
  paths (for example `_temp_file`) over `${TMPDIR:-/tmp}` path construction.
- Avoid explicit cleanup-only commands (`rm -f ...`) unless cleanup behavior
  itself is the subject under test; the runner handles sandbox cleanup.

### Available assertions

All patterns are **regex** (see "Regex syntax" below). Patterns are
enclosed in double quotes.

| Assertion | Meaning |
|---|---|
| `expect_stdout "pattern"` | stdout matches regex |
| `expect_stderr "pattern"` | stderr matches regex |
| `expect_stdout_line "pattern"` | at least one stdout line matches |
| `expect_stderr_line "pattern"` | at least one stderr line matches |
| `expect_exit_code N` | exit code equals N |
| `not_expect_stdout "pattern"` | stdout does NOT match regex |
| `not_expect_stderr "pattern"` | stderr does NOT match regex |
| `not_expect_exit_code N` | exit code does NOT equal N |

### Assertion tips

- `expect_stdout "hello"` succeeds if "hello" appears *anywhere* in stdout.
- `expect_stdout "^hello$"` requires stdout to be exactly "hello" (with
  `^` and `$` anchoring the start/end of the full output, **not**
  individual lines).
- Use `\n` in patterns to match across lines: `expect_stdout "line1\nline2"`.
- `expect_exit_code 0` is implicit if omitted — tests pass if exit code
  is 0 and all assertions match. Use `not_expect_exit_code 0` to assert
  failure.
- `expect_stderr ""` asserts stderr is empty.

### setenv

Set an environment variable for the test's execution:

```
begin test "locale-sensitive test"
  setenv "LC_ALL" "test_EPTY.UTF-8"
  begin script
    ...
  end script
  expect_stdout "..."
end test "locale-sensitive test"
```

`setenv` goes between `begin test` and `begin script`.

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
| `not_expect "regex"` | Assert regex does NOT match current output buffer. |
| `not_expect timeout=500ms "regex"` | Watch for duration, fail if matched. |
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
| `.` | Any character |
| `*` | Zero or more of preceding |
| `+` | One or more of preceding |
| `?` | Zero or one of preceding |
| `[abc]` | Character class |
| `[a-z]` | Character range |
| `[[:digit:]]` | POSIX named class |
| `[^abc]` or `[!abc]` | Negated class |
| `(a\|b)` | Alternation group |
| `\` | Escape next character |

**Important**: In expect/not_expect patterns, backslash is passed
through **raw** to the regex engine. There is no string-level escaping.
To match a literal backslash, write `\\`. To match `[`, write `\[`.
To embed a literal `"` in a pattern, double it: `""`.

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
  begin script
    echo $((2 + 3))
  end script
  expect_stdout "5"
end test "arithmetic expansion"
```

### Testing exit codes

```
begin test "false returns non-zero"
  begin script
    false
  end script
  not_expect_exit_code 0
end test "false returns non-zero"
```

### Testing stderr

```
begin test "syntax error produces stderr"
  begin script
    $SHELL -c 'if then' 2>&1
  end script
  not_expect_stderr ""
end test "syntax error produces stderr"
```

### Testing multi-line output

```
begin test "for loop output"
  begin script
    for i in a b c; do echo $i; done
  end script
  expect_stdout "a\nb\nc"
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
