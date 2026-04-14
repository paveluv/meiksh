# Unit tests: syscall trace DSL

Meiksh unit tests often drive the shell or subsystems under a **fake system interface** (`crate::sys::test_support`). The trace lists expected syscalls, argument shapes, and return values. `run_trace(trace, || { ... })` replays the trace for the closure; `fork` entries can expand into parent and child runs automatically.

This document matches the **implemented** helpers and the `trace_entries!` / `syscall_test!` macros in `src/lib.rs` (test-only).

**Implementation note:** `syscall_test!` expands all recursive invocations as `$crate::syscall_test!(...)`, so `trace_entries!` works when called from any crate module (the trace parser is not tied to the call site’s macro scope).

## Core API (unchanged)

- `run_trace(Vec<TraceEntry>, impl Fn())` — run closure with the trace interface; validates order and argument matching.
- `t(syscall, args, result)` — one `TraceEntry` (use when the macro is awkward or values are dynamic).
- `t_fork(result, child_trace)` — `fork` with optional child script.
- `assert_no_syscalls(|| ...)` — panic if any syscall runs.

**FD constants:** prefer `crate::sys::{STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO}` (or `libc` names) in traces instead of raw `0`/`1`/`2` where it aids readability.

## Building traces: `trace_entries!`

Use this macro anywhere you would write `vec![t(...), ...]`:

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
- **Spread:** splice another `Vec<TraceEntry>` with `..other_vec,` (same as `syscall_test!`).

## End-to-end shell tests: `syscall_test!`

```rust
syscall_test! {
    name: my_case,
    args: ["-c", "true"],
    trace: [
        // same entry syntax as trace_entries!
    ],
}
```

## Argument shorthand

In `syscall(...) -> result`, each argument is comma-separated. Recognized forms:

| Syntax | `ArgMatcher` |
|--------|----------------|
| `_` or `any` | `Any` |
| `int(n)` | `Int(n as i64)` |
| `fd(n)` | `Fd(n)` |
| `bytes(expr)` | `Bytes` from `expr` treated as bytes (`&[u8]`, `b"..."`, `vec![…]`, etc.) |
| `str(expr)` | `Str` from byte slice (`str(b"KEY")` for env names) |
| any other expression | `arg_from(expr)` (`IntoArgMatcher`: `i32`, `i64`, `&str` → path string, `&[u8]` → **bytes**, not `Str`) |

**Rule:** use `str(b"...")` when the syscall expects a C string key/path as `Str`, and `bytes(...)` for `write` payloads or `read` buffers.

## Result shorthand

After `->`, one of:

| Syntax | `TraceResult` |
|--------|----------------|
| `err(errno)` | `Err` |
| `bytes(expr)` | `Bytes` |
| `fd(n)` | `Fd` |
| `pid(n)` | `Pid` |
| `fds(r, w)` | `Fds` |
| `cwd("...")` | `CwdBytes` |
| `realpath("...")` | `RealpathBytes` |
| `stat_dir` | `StatDir` |
| `stat_file(mode)` | `StatFile` |
| `stat_file_size(n)` | `StatFileSize` |
| `dir_entry(expr)` | `DirEntryBytes` |
| `status(n)` | `Status` (usually with `waitpid` shorthand below) |
| `stopped_sig(n)` | `StoppedSig` |
| `signaled_sig(n)` | `SignaledSig` |
| `continued` | `ContinuedStatus` |
| `auto` | `Auto` (typical for `write` length = payload size) |
| `void` | `Void` |
| `interrupt(sig)` | `Interrupt` (EINTR-style) |
| `int(n)` | `Int` |
| `_` | `Int(0)` (legacy wildcard) |
| bare literal / expression | `Int(expr as i64)` |

## `waitpid` shorthand

These expand to `waitpid` with `(pid, Any, Any)` argument matching (the third field matches any options the code passes):

```text
waitpid(pid_expr, _) -> status(code)
waitpid(pid_expr, _) -> stopped_sig(sig)
waitpid(pid_expr, _) -> signaled_sig(sig)
waitpid(pid_expr, _) -> continued
```

For non-default option matching, use the generic form, for example:

`waitpid(int(pid), _, int(sys::WUNTRACED)) -> status(7)`.

## `fork`

```text
fork() -> pid(parent_pid), child: [ /* child entries */ ]
```

Child traces use the same entry syntax.

## Stdin read sequences: `stdin_chunks` / `stdin_bytes` / `stdin_repeat`

`pub(crate)` helpers in `crate::sys::test_support` (compiled only for tests):

- `stdin_chunks([b"a", b"b"])` — `read(STDIN_FILENO, _) -> Bytes` for each chunk, then a final EOF `read -> Int(0)`.
- `stdin_bytes(b"one")` — one payload read plus that EOF sequence.
- `stdin_repeat(chunk, n)` — `n` reads of the same payload, then EOF.

Splice with spread when **only** stdin reads are needed in that segment: `trace_entries![ ..stdin_chunks([b"x", b"y"]), ]`. Do **not** use `stdin_chunks` when another syscall (e.g. `open`) must run immediately after the last byte with no EOF read in between—spell those `read(fd(STDIN_FILENO), _) -> bytes(...)` lines explicitly.

## Shell test helpers

- `crate::shell::test_support::t_stderr(msg)` and `capture_forked_trace(status, pid)` are implemented with `trace_entries!` internally; you can still splice their `TraceEntry` values with `..vec![t_stderr(...)],` in a larger trace.

## When to keep `t(...)`

- Dynamic `Vec` construction, loops, or conditional trace lines.
- Very custom `TraceResult` variants (`EnvMap`, `StrVal`, `NullStr`, etc.) not covered by the macro.
- Mixing with helpers like `crate::shell::test_support::t_stderr` or `capture_forked_trace`.

## Rules of thumb

1. **Stdout/stderr `write`:** second arg must be `Bytes`, not `any` (enforced by the trace checker).
2. **Fork:** parent `Pid > 0` entries must include `child: [...]` (`t_fork` or macro `fork`).
3. **Behavior:** changing traces changes only test expectations, not production code — keep semantics identical when migrating style.
