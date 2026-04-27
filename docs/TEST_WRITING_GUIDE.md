# Test Writing Guide

> **Authority:** `docs/IMPLEMENTATION_POLICY.md` is the canonical source for
> project-wide rules.  If anything in this guide conflicts with the
> implementation policy, the policy takes priority.

Meiksh has two levels of testing: **unit tests** (in-crate, under `src/`) and
**integration tests** (out-of-crate, under `tests/integration/`).  Use good
judgement to pick the right level for what you are testing.

---

## Unit tests vs. integration tests

| | Unit tests | Integration tests |
|---|---|---|
| **Location** | `#[cfg(test)] mod tests` inside each `src/` file | `tests/integration/*.rs` |
| **Binary** | Runs inside the library crate | Spawns the `meiksh` binary as a child process |
| **Syscalls** | Faked via `crate::sys::test_support` | Real kernel syscalls |
| **Speed** | Fast (no process spawn) | Slower (fork + exec per test) |
| **Run** | `cargo test --lib` | `cargo test --test integration_basic` |

### When to write a unit test

- Testing **internal functions** that are not reachable through the shell CLI
  (private helpers, `pub(crate)` APIs, trait impls, parser internals).
- Testing **exact syscall sequences** — the trace infrastructure lets you
  assert that `open`, `read`, `write`, `close` happen in the right order with
  the right arguments.
- Testing **error paths** that require a specific syscall to fail (`-> err(EBADF)`).
- Testing **pure logic** with no I/O (use `assert_no_syscalls`).
- When the code under test uses `FakeContext`, `test_shell()`, or similar
  in-crate test doubles.

### When to write an integration test

- Testing **end-to-end shell behaviour** as a user would observe it: given a
  script, assert on stdout, stderr, and exit status.
- Testing **POSIX compliance** — the shell must produce the same output as
  specified by the standard for a given input.
- Testing **interaction between subsystems** (parsing + expansion + execution +
  redirection together).
- Testing **edge cases in shell syntax** (heredocs, quoting, line
  continuations, IFS splitting) that are easiest to express as a shell snippet.
- When you need **real file system, pipes, or signal delivery** that the fake
  syscall layer cannot easily simulate.  (Unit tests must be isolated and
  in-memory — see `IMPLEMENTATION_POLICY.md`; integration tests are the
  appropriate place for real OS interactions.)

### Grey areas

Some code paths can be tested at either level.  Prefer the level that gives
you the **shortest, most readable test**.  If a unit test requires an
elaborate trace with 15 syscalls just to reach the line you care about, an
integration test with a two-line shell script is better.  Conversely, if an
integration test requires a fragile temp-file setup to hit one branch in the
expander, a unit test calling the function directly is better.

---

## Part 1 — Unit tests

Unit tests drive the shell or its subsystems under a **fake system interface**
(`crate::sys::test_support`).  A trace is a list of expected syscalls with
argument shapes and return values.  `run_trace(trace, || { … })` replays the
trace for the closure; `fork` entries automatically expand into parent and
child runs.

This section covers the **implemented** helpers, the `trace_entries!` /
`syscall_test!` macros in `src/lib.rs` (test-only), and the conventions every
unit test must follow.

### Golden rule

> **Every `run_trace` call MUST receive its trace from `trace_entries!`.**
>
> Never pass a hand-built `vec![t(…), …]` directly to `run_trace`.  If you
> need dynamic entries (loops, conditionals, `TraceResult` variants the macro
> doesn't cover), build them with `t(…)` / `t_fork(…)` inside a `Vec` and
> **spread** them into `trace_entries!` with `..expr`:
>
> ```rust
> let dynamic = vec![t("getenv", vec![ArgMatcher::Str(b"HOME".to_vec())],
>                      TraceResult::StrVal(b"/users/me".to_vec()))];
> run_trace(
>     trace_entries![
>         open("/tmp/x", _, _) -> fd(3),
>         ..dynamic,
>         close(fd(3)) -> 0,
>     ],
>     || { /* … */ },
> );
> ```

### Quick-start: writing a unit test

#### 1. Choose the right test module

Tests live in `#[cfg(test)] mod tests` blocks inside each source file.  Find
(or create) the `mod tests` closest to the code you are testing.

#### 2. Add imports

**For sys/ modules** (testing syscall wrappers directly):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support::{run_trace, assert_no_syscalls};
    use crate::trace_entries;
    // For locale-sensitive tests, also import:
    // use crate::sys::test_support::{set_test_locale_c, set_test_locale_utf8};

    // …
}
```

**For builtin modules** (testing shell builtins):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::builtin::test_support::{diag, invoke, run_trace, test_shell};
    use crate::trace_entries;

    // …
}
```

**For shell/ or exec/ modules** (testing shell execution, jobs, etc.):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::shell::test_support::test_shell;
    use crate::sys::test_support::{run_trace, assert_no_syscalls};
    use crate::trace_entries;

    // …
}
```

Only import what you use — `ArgMatcher`, `TraceResult`, and `t` are needed
only when building dynamic entries for spread.

#### 3. Write the test

```rust
#[test]
fn setenv_success() {
    run_trace(
        trace_entries![setenv(str(b"MY_KEY"), str(b"my_val")) -> 0],
        || {
            let result = env_set_var(b"MY_KEY", b"my_val");
            assert!(result.is_ok());
        },
    );
}
```

#### 4. Tests that make no syscalls

Use `assert_no_syscalls` for pure logic tests:

```rust
#[test]
fn parse_returns_none_on_empty() {
    assert_no_syscalls(|| {
        assert_eq!(parse_usize(b""), None);
    });
}
```

#### 5. Testing locale-sensitive behaviour

The test interface supports two locales: **C** (byte-level / ASCII-only) and
**C.UTF-8** (full Unicode via Rust's `char` methods).  The default is C.

Use `set_test_locale_c()` and `set_test_locale_utf8()` from
`crate::sys::test_support` to switch locale inside `assert_no_syscalls` or
`run_trace` blocks.  This controls how `decode_char`, `classify_char`,
`encode_char`, `mb_cur_max`, `to_upper`, `to_lower`, and `char_width` behave.

Any code that touches multi-byte characters, character classification,
character counting, IFS splitting, or pattern matching should have **paired
tests** — one under C and one under C.UTF-8 — asserting different results
for the same input.

```rust
use crate::sys::test_support::{assert_no_syscalls, set_test_locale_c, set_test_locale_utf8};

#[test]
fn count_chars_c_vs_utf8() {
    assert_no_syscalls(|| {
        // U+00E9 = 0xC3 0xA9
        set_test_locale_c();
        assert_eq!(count_chars(b"\xc3\xa9"), 2);    // two bytes

        set_test_locale_utf8();
        assert_eq!(count_chars(b"\xc3\xa9"), 1);    // one character
    });
}
```

| Function | C behaviour | C.UTF-8 behaviour |
|---|---|---|
| `decode_char` | `(bytes[0], 1)` — one byte per character | Full UTF-8 decode via `std::str::from_utf8` |
| `classify_char` | Rejects codepoints > 0x7F | Full Unicode (`is_alphabetic()`, etc.) |
| `encode_char` | Single byte if `<= 0x7F`, else 0 | Full UTF-8 encode |
| `mb_cur_max` | 1 | 4 |
| `to_upper` / `to_lower` | ASCII a-z / A-Z only | Full Unicode case mapping |
| `char_width` | 0 for control, 1 otherwise | 0 for control, 1 otherwise |
| `strcoll` | Byte comparison | Byte comparison (same) |
| `decimal_point` | `b'.'` | `b'.'` (same) |

`setup_locale` resets to C.  `reinit_locale` reads `LC_ALL` / `LANG` from
`std::env` and sets UTF-8 if the value contains "UTF-8" or "UTF8"
(case-insensitive); otherwise C.

---

### Core API

| Function | Purpose |
|----------|---------|
| `run_trace(Vec<TraceEntry>, impl Fn())` | Run closure with the trace interface; validates order and argument matching. |
| `t(syscall, args, result)` | Build one `TraceEntry` manually (for spread into `trace_entries!`). |
| `t_fork(result, child_trace)` | `fork` with optional child script. |
| `assert_no_syscalls(\|\| …)` | Panic if any syscall runs during the closure. |
| `set_test_locale_c()` | Switch the test locale to C (byte-level, ASCII-only). This is the default. |
| `set_test_locale_utf8()` | Switch the test locale to C.UTF-8 (full Unicode). |

**Constants:** prefer `crate::sys::constants::{STDIN_FILENO, STDOUT_FILENO,
STDERR_FILENO, EBADF, ...}` in traces instead of raw integers where it aids
readability. Do not import `libc` in tests outside `src/sys/` and
`tests/integration/sys.rs`; use the project constants from `crate::sys` instead.

---

### Building traces with `trace_entries!`

```rust
use crate::trace_entries;

run_trace(
    trace_entries![
        open("/tmp/x", _, _) -> fd(3),
        read(fd(3), _) -> bytes(b"hi"),
        read(fd(3), _) -> 0,
        close(fd(3)) -> 0,
    ],
    || { /* test body */ },
);
```

- Entries are comma-separated; a trailing comma is optional.
- **Spread:** splice another `Vec<TraceEntry>` with `..expr`:

```rust
let extra = stdin_bytes(b"input");
run_trace(
    trace_entries![
        write(fd(1), bytes(b"prompt")) -> auto,
        ..extra,
    ],
    || { /* … */ },
);
```

**Implementation note:** `trace_entries!` expands recursive invocations as
`$crate::syscall_test!(…)`, so the macro works when called from any crate
module.

---

### End-to-end shell tests with `syscall_test!`

```rust
syscall_test! {
    name: my_case,
    args: ["-c", "true"],
    trace: [
        // same entry syntax as trace_entries!
    ],
}
```

---

### Argument shorthand

In `syscall(…) -> result`, each argument is comma-separated:

| Syntax | `ArgMatcher` |
|--------|--------------|
| `_` or `any` | `Any` |
| `int(n)` | `Int(n as i64)` |
| `fd(n)` | `Fd(n)` |
| `bytes(expr)` | `Bytes` — `expr` is any bytes-like: `b"…"`, `&[u8]`, `vec![…]`, `&msg` |
| `str(expr)` | `Str` — from a byte slice (`str(b"KEY")` for env names, paths) |
| any other expression | `arg_from(expr)` via `IntoArgMatcher`: `i32`/`i64` → Int, `&str` → path Str, `&[u8]` → Bytes |

**Rule:** use `str(b"…")` when the syscall expects a C string key/path as
`Str`, and `bytes(…)` for `write` payloads or `read` buffers.

---

### Result shorthand

After `->`, one of:

| Syntax | `TraceResult` |
|--------|--------------|
| `err(errno)` | `Err` |
| `bytes(expr)` | `Bytes` |
| `fd(n)` | `Fd` |
| `pid(n)` | `Pid` |
| `fds(r, w)` | `Fds` |
| `cwd("…")` | `CwdBytes` |
| `realpath("…")` | `RealpathBytes` |
| `stat_dir` | `StatDir` |
| `stat_file(mode)` | `StatFile` |
| `stat_file_size(n)` | `StatFileSize` |
| `stat_fifo` | `StatFifo` |
| `dir_entry(expr)` | `DirEntryBytes` |
| `status(n)` | `Status` (usually with `waitpid` shorthand) |
| `stopped_sig(n)` | `StoppedSig` |
| `signaled_sig(n)` | `SignaledSig` |
| `continued` | `ContinuedStatus` |
| `auto` | `Auto` (typical for `write` — length = payload size) |
| `void` | `Void` |
| `interrupt(sig)` | `Interrupt` (EINTR-style) |
| `int(n)` | `Int` |
| `_` | `Int(0)` (legacy wildcard) |
| bare literal / expression | `Int(expr as i64)` |

---

### `waitpid` shorthand

These expand to `waitpid` with `(pid, Any, Any)` argument matching:

```text
waitpid(pid_expr, _) -> status(code)
waitpid(pid_expr, _) -> stopped_sig(sig)
waitpid(pid_expr, _) -> signaled_sig(sig)
waitpid(pid_expr, _) -> continued
```

For non-default option matching, use the generic form:

```text
waitpid(int(pid), _, int(sys::WUNTRACED)) -> status(7)
```

---

### `fork`

```text
fork() -> pid(parent_pid), child: [ /* child entries */ ]
```

Child traces use the same entry syntax.

---

### Stdin helpers

`pub(crate)` helpers in `crate::sys::test_support` (test-only):

| Helper | Behaviour |
|--------|-----------|
| `stdin_chunks([b"a", b"b"])` | `read(STDIN, _) -> Bytes` for each chunk, then a final EOF `read -> Int(0)`. |
| `stdin_bytes(b"one")` | One payload read plus EOF. |
| `stdin_repeat(chunk, n)` | `n` reads of the same payload, then EOF. |

Splice with spread:

```rust
run_trace(
    trace_entries![..stdin_chunks([b"x", b"y"])],
    || { /* … */ },
);
```

Do **not** use `stdin_chunks` when another syscall must follow the last byte
with no EOF read in between — spell those `read(fd(STDIN_FILENO), _) ->
bytes(…)` lines explicitly.

---

### Shell test helpers

| Helper | Location | Purpose |
|--------|----------|---------|
| `test_shell()` | `crate::shell::test_support` | Minimal `Shell` with defaults (`last_status: 0`, non-interactive). |
| `test_shell()` | `crate::builtin::test_support` | Builtin-specific `Shell` (`last_status: 3`, non-interactive). |
| `invoke(shell, argv)` | `crate::builtin::test_support` | Run a builtin by name, returns `Result<BuiltinOutcome, ShellError>`. |
| `diag(msg)` | `crate::builtin::test_support` | Build `b"meiksh: <msg>\n"` for matching diagnostic stderr writes. |
| `t_stderr(msg)` | `crate::shell::test_support` | One `TraceEntry` for a stderr diagnostic write (for spread). |
| `fake_handle(pid)` | `crate::shell::test_support` | Minimal `ChildHandle` for job tests. |
| `capture_forked_trace(status, pid)` | `crate::shell::test_support` | Pipe + fork + wait trace for command substitution tests (for spread). |

---

### Unit test examples

#### Testing a builtin (echo write error)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::builtin::test_support::{diag, invoke, run_trace, test_shell};
    use crate::sys::constants::EBADF;
    use crate::trace_entries;

    #[test]
    fn echo_write_error_returns_nonzero() {
        let msg = diag(b"echo: write error: Bad file descriptor");
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"hello\n")) -> err(EBADF),
                write(fd(2), bytes(&msg)) -> auto,
            ],
            || {
                let mut shell = test_shell();
                let outcome =
                    invoke(&mut shell, &[b"echo".to_vec(), b"hello".to_vec()])
                        .expect("echo");
                assert!(matches!(outcome, BuiltinOutcome::Status(1)));
            },
        );
    }
}
```

#### Testing a syscall wrapper (setenv)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    use crate::sys::test_support::run_trace;
    use crate::trace_entries;

    #[test]
    fn setenv_success() {
        run_trace(
            trace_entries![setenv(str(b"MY_KEY"), str(b"my_val")) -> 0],
            || {
                let result = env_set_var(b"MY_KEY", b"my_val");
                assert!(result.is_ok());
            },
        );
    }
}
```

#### Spreading dynamic entries

When the macro doesn't support a `TraceResult` variant (e.g. `EnvMap`,
`StrVal`, `NullStr`), build entries with `t(…)` and spread them:

```rust
use crate::sys::test_support::{ArgMatcher, TraceResult, run_trace, t};
use crate::trace_entries;

#[test]
fn getenv_found() {
    run_trace(
        trace_entries![
            ..vec![t(
                "getenv",
                vec![ArgMatcher::Str(b"HOME".to_vec())],
                TraceResult::StrVal(b"/home/user".to_vec()),
            )]
        ],
        || {
            let val = env_var(b"HOME");
            assert_eq!(val, Some(b"/home/user".to_vec()));
        },
    );
}
```

#### Pure logic test (no syscalls)

```rust
#[test]
fn parse_hex_values() {
    assert_no_syscalls(|| {
        assert_eq!(parse_hex_i64(b"ff"), Some(255));
        assert_eq!(parse_hex_i64(b""), None);
    });
}
```

---

### When to keep `t(…)` (always inside spread)

- Dynamic `Vec` construction, loops, or conditional trace lines.
- `TraceResult` variants not covered by the macro: `EnvMap`, `StrVal`,
  `NullStr`, `ClockTicks`.
- Mixing with helpers like `t_stderr(…)` or `capture_forked_trace(…)`.

In all these cases, wrap the result in `..vec![…]` or `..helper_fn()` inside
`trace_entries!`.

---

### Unit test rules and conventions

1. **Always `trace_entries!`** — never pass `vec![t(…)]` directly to
   `run_trace`.  Spread dynamic entries with `..expr`.

2. **Stdout/stderr `write`:** the second arg **must** be `bytes(…)`, not `_`.
   The trace checker enforces this.

3. **Fork:** parent `Pid > 0` entries must include `child: […]` (macro `fork`
   or `t_fork`).

4. **Import `trace_entries`** at the module level:
   `use crate::trace_entries;` — it's a `#[macro_export]` so it lives at the
   crate root.

5. **FD constants:** use `sys::STDIN_FILENO` / `sys::STDOUT_FILENO` /
   `sys::STDERR_FILENO` in both trace entries and test code for clarity.

6. **Diagnostic messages:** use the `diag(b"…")` helper from
   `crate::builtin::test_support` to build the expected
   `b"meiksh: <msg>\n"` bytes for stderr assertions.

7. **Changing traces only changes expectations** — never production code.
   Keep semantics identical when migrating test style.

8. **Run tests after writing:** `cargo test --lib <module_path>` to run a
   specific module's tests, `cargo test --lib` for the full suite.

9. **Locale-sensitive code needs paired tests.**  Any function that calls
   `decode_char`, `classify_char`, `count_chars`, `first_char_len`,
   `decompose_ifs`, or pattern matching with `?` / character classes must
   have at least one test that runs under both `set_test_locale_c()` and
   `set_test_locale_utf8()`.  The two locales should produce
   **observably different** results for multi-byte input (e.g.
   `"é"` is 2 chars in C, 1 in UTF-8).

10. **Tests that assume UTF-8 must opt in.**  The default test locale is C.
    If a test relies on multi-byte character semantics (e.g. vi editing
    tests), call `set_test_locale_utf8()` at the start of the `run_trace`
    or `assert_no_syscalls` closure.

---

## Part 2 — Integration tests

Integration tests live in `tests/integration/` and exercise the **compiled
`meiksh` binary** as a black box.  Each test spawns the shell, feeds it a
script, and asserts on stdout, stderr, and exit status.

### File layout

```
tests/integration/
├── basic.rs            # entry point (mod declarations, core execution tests)
├── common.rs           # shared helpers (meiksh(), TempDir, run_*)
├── bind_builtin.rs     # bind builtin and readline-compatible key bindings
├── builtins.rs         # builtin command tests
├── control_flow.rs     # if/while/for/case/subshell tests
├── emacs_mode.rs       # emacs editing mode PTY tests
├── expansion.rs        # parameter expansion, globbing, tilde, arithmetic
├── inputrc_parser.rs   # inputrc parser and loader tests
├── interactive.rs      # interactive REPL behavior
├── os_interface.rs     # executable-file and OS-boundary behavior
├── parser_coverage.rs  # tokenizer/parser edge cases
├── prompt.rs           # prompt expansion and prompt-related behavior
├── redirection.rs      # redirections, heredocs, fd manipulation
├── shell_options.rs    # set -e, set -u, set -f, startup files
└── sys.rs              # integration-test-only OS helpers
```

`basic.rs` is the crate root (`#[test]` binary).  It declares the other files
as modules.  Integration-test modules import only the shared helpers they use,
for example `use super::common::{TempDir, meiksh, run_meiksh_with_stdin};`.
Wildcard imports from `common.rs` are not allowed; this follows the import
rules in `IMPLEMENTATION_POLICY.md`.

### Shared helpers (`common.rs`)

| Helper | Purpose |
|--------|---------|
| `meiksh()` | Returns the path to the compiled `meiksh` binary (`env!("CARGO_BIN_EXE_meiksh")`). |
| `TempDir::new(prefix)` | Creates a temporary directory; cleaned up on `Drop`. |
| `run_meiksh_with_stdin(script, stdin)` | Runs `meiksh -c <script>` with piped stdin, returns `Output`. |
| `run_meiksh_with_nonblocking_stdin(stdin)` | Runs `meiksh` reading from a non-blocking stdin pipe. |
| `run_interactive(input)` | Runs `meiksh -i` with piped input. |

### Quick-start: writing an integration test

#### 1. Pick the right module

Place the test in the module that matches its topic:

- Expansion behaviour → `expansion.rs`
- Builtin command → `builtins.rs`
- Redirect / heredoc → `redirection.rs`
- Control flow (`if`, `while`, `for`, `case`) → `control_flow.rs`
- Parser edge case → `parser_coverage.rs`
- Shell options (`set -e`, etc.) → `shell_options.rs`
- General / doesn't fit → `basic.rs`

#### 2. Write the test

The simplest pattern — run a `-c` script and assert on output:

```rust
#[test]
fn parameter_length_operator() {
    let out = Command::new(meiksh())
        .args(["-c", "x=hello; printf '%s' \"${#x}\""])
        .output()
        .expect("run meiksh");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5");
}
```

#### 3. Tests that need stdin

Use `run_meiksh_with_stdin` when the script reads from standard input:

```rust
#[test]
fn read_builtin_from_stdin() {
    let out = run_meiksh_with_stdin("read line; printf '%s' \"$line\"", b"hello\n");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hello");
}
```

For cases where you need to pipe raw bytes without `run_meiksh_with_stdin`
adding `-c` (e.g. testing the shell reading a script from stdin), use
`Command` directly:

```rust
#[test]
fn shell_reads_script_from_stdin() {
    let mut child = Command::new(meiksh())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child.stdin.take().unwrap().write_all(b"printf hi\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"hi");
}
```

#### 4. Tests that need the file system

Use `TempDir` for tests that create files or directories:

```rust
#[test]
fn glob_expands_in_directory() {
    let dir = TempDir::new("meiksh-glob");
    fs::write(dir.join("a.txt"), "").unwrap();
    fs::write(dir.join("b.txt"), "").unwrap();

    let out = Command::new(meiksh())
        .current_dir(dir.path())
        .args(["-c", "printf '%s|' *.txt"])
        .output()
        .expect("run meiksh");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "a.txt|b.txt|");
}
```

#### 5. Tests that expect failure

Assert on `!out.status.success()` and/or check stderr:

```rust
#[test]
fn syntax_error_produces_diagnostic() {
    let out = Command::new(meiksh())
        .args(["-c", "if"])
        .output()
        .expect("run meiksh");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("syntax error"));
}
```

---

### Integration test rules and conventions

1. **Use `meiksh()` for the binary path** — never hardcode a path.  The
   helper uses `env!("CARGO_BIN_EXE_meiksh")` which Cargo sets automatically.

2. **Prefer `-c` over stdin** for simple scripts.  It avoids line-buffering
   surprises (the shell reads stdin line by line, so `\<newline>` continuations
   at the end of a line won't see the next line until it's buffered).

3. **Clean up temp files** — always use `TempDir` (auto-cleaned on drop)
   rather than writing to fixed paths.

4. **Assert on all three channels** when relevant: `status.success()`,
   `stdout`, and `stderr`.  At minimum, always check the exit status.

5. **Use `printf` over `echo`** in test scripts.  `printf '%s'` avoids
   trailing newline ambiguity and is more portable.

6. **Keep scripts minimal** — test one behaviour per test.  Long multi-line
   scripts are harder to debug when they fail.

7. **Name tests descriptively** — the name should describe the behaviour
   being verified, not the implementation detail.  Good:
   `parameter_plus_op_set_returns_word`.  Bad: `test_line_597`.

8. **Run tests after writing:** `cargo test --test integration_basic` for the
   integration suite, or `cargo test` for everything.
