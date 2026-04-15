# Unit Test Writing Guide

Meiksh unit tests drive the shell or its subsystems under a **fake system
interface** (`crate::sys::test_support`).  A trace is a list of expected
syscalls with argument shapes and return values.  `run_trace(trace, || { … })`
replays the trace for the closure; `fork` entries automatically expand into
parent and child runs.

This document covers the **implemented** helpers, the `trace_entries!` /
`syscall_test!` macros in `src/lib.rs` (test-only), and the conventions every
test must follow.

---

## Golden rule

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

---

## Quick-start: writing a test from scratch

### 1. Choose the right test module

Tests live in `#[cfg(test)] mod tests` blocks inside each source file.  Find
(or create) the `mod tests` closest to the code you are testing.

### 2. Add imports

**For sys/ modules** (testing syscall wrappers directly):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::test_support::{run_trace, assert_no_syscalls};
    use crate::trace_entries;

    // …
}
```

**For builtin modules** (testing shell builtins):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;   // run_trace, test_shell, invoke, diag
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

### 3. Write the test

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

### 4. Tests that make no syscalls

Use `assert_no_syscalls` for pure logic tests:

```rust
#[test]
fn parse_returns_none_on_empty() {
    assert_no_syscalls(|| {
        assert_eq!(parse_usize(b""), None);
    });
}
```

---

## Core API

| Function | Purpose |
|----------|---------|
| `run_trace(Vec<TraceEntry>, impl Fn())` | Run closure with the trace interface; validates order and argument matching. |
| `t(syscall, args, result)` | Build one `TraceEntry` manually (for spread into `trace_entries!`). |
| `t_fork(result, child_trace)` | `fork` with optional child script. |
| `assert_no_syscalls(\|\| …)` | Panic if any syscall runs during the closure. |

**FD constants:** prefer `crate::sys::{STDIN_FILENO, STDOUT_FILENO,
STDERR_FILENO}` (or `libc` names) in traces instead of raw `0`/`1`/`2` where
it aids readability.

---

## Building traces with `trace_entries!`

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

## End-to-end shell tests with `syscall_test!`

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

## Argument shorthand

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

## Result shorthand

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

## `waitpid` shorthand

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

## `fork`

```text
fork() -> pid(parent_pid), child: [ /* child entries */ ]
```

Child traces use the same entry syntax.

---

## Stdin helpers

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

## Shell test helpers

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

## Complete examples

### Testing a builtin (echo write error)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin::test_support::*;
    use crate::trace_entries;

    #[test]
    fn echo_write_error_returns_nonzero() {
        let msg = diag(b"echo: write error: Bad file descriptor");
        run_trace(
            trace_entries![
                write(fd(1), bytes(b"hello\n")) -> err(libc::EBADF),
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

### Testing a syscall wrapper (setenv)

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

### Spreading dynamic entries

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

### Pure logic test (no syscalls)

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

## When to keep `t(…)` (always inside spread)

- Dynamic `Vec` construction, loops, or conditional trace lines.
- `TraceResult` variants not covered by the macro: `EnvMap`, `StrVal`,
  `NullStr`, `ClockTicks`.
- Mixing with helpers like `t_stderr(…)` or `capture_forked_trace(…)`.

In all these cases, wrap the result in `..vec![…]` or `..helper_fn()` inside
`trace_entries!`.

---

## Rules and conventions

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
