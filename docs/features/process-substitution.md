# Process Substitution

## Status

**Implemented.** The `bash_procsub` shell option is accepted by `set -o` / `set +o` ([src/shell/options.rs](../../src/shell/options.rs)) and the lexer in [src/syntax/token.rs](../../src/syntax/token.rs) recognizes the `<(` / `>(` openers when the option is on at parse time, balancing the body and parsing it via the same recursive entry used by `$(...)`. With the option off, the lexer emits the option-aware diagnostic from § 9.1. The runtime side lives in [src/shell/proc_substitute.rs](../../src/shell/proc_substitute.rs): each substitution forks a subshell, wires its stdin or stdout through a pipe (or FIFO, see below), and emits the substituted path word; the consuming command's `execute_simple` exit hook in [src/exec/simple.rs](../../src/exec/simple.rs) closes the fd / unlinks the FIFO and reaps the subshell.

Both path representations from § 6 are implemented: the `/dev/fd/N` form (preferred) is selected when the runtime probe in [src/sys/fs.rs](../../src/sys/fs.rs) (`dev_fd_supported`) finds `/dev/fd` is a directory; otherwise the implementation falls back to a named FIFO under `${TMPDIR:-/tmp}/meiksh-procsub.<pid>.<seq>` with permissions `0600`. The probe result is cached on `Shell` for the lifetime of the shell. Lexer, runtime, and cleanup behavior is verified by unit tests colocated with each module and by integration tests in [tests/integration/process_substitution.rs](../../tests/integration/process_substitution.rs); the FIFO-backing branch is unit-tested via the syscall-trace mock since CI runs on Linux where the `/dev/fd` probe always succeeds.

## 1. Scope

This document is the authoritative specification of the process-substitution syntax `<(list)` and `>(list)` recognized by meiksh when the `bash_procsub` shell option is enabled. It defines:

- A feature gate (`bash_procsub`) that distinguishes strict POSIX command-language parsing from the bash-style extended grammar.
- The lexical rules under which `<(` and `>(` are recognized as the opening of a process substitution rather than as a `<` / `>` redirection followed by a `(` subshell.
- The expansion the construct produces — a single path-shaped word — and where in the expansion pipeline that production happens.
- The process model: how the embedded `list` is forked into a subshell, what file descriptors connect the subshell to the consuming command, and how the subshell is reaped.
- The lifetime and cleanup contract for the file descriptors and any FIFOs the implementation may create.
- The interaction with `$!`, `wait`, exit status, redirection operators, here-documents, command substitution, and quote contexts.

POSIX leaves the entire feature unspecified. Bash, ksh93, and zsh all implement the construct with substantially the same observable behavior; the differences are confined to (1) the path the substitution expands to (`/dev/fd/N` versus a named FIFO under `${TMPDIR:-/tmp}`) and (2) the lifetime guarantees around when the subshell is reaped. Meiksh aligns with the bash behavior on Linux because that is the most widely deployed; the FIFO fallback path is specified for portability to systems without `/dev/fd`.

### 1.1 Conformance Language

The key words "shall", "shall not", "should", "should not", "may", and "must" in this document are to be interpreted as described in RFC 2119. Text following a bulleted `shall` requirement constitutes a normative requirement that meiksh conformance tests verify.

### 1.2 De-Facto Reference

Where this document intentionally aligns with existing practice, the de-facto reference is:

- GNU Bash 5.2, `Process Substitution` section of `bash(1)`.
- ksh93, `Process Substitution` in `ksh(1)`.
- zsh 5.9, `PROCESS SUBSTITUTION` in `zshexpn(1)`.

The lexical form, the single-word nature, and the path-expansion result are common to all three. Meiksh follows bash semantics where the three reference shells diverge — most notably, the lifetime of the subshell child (Section 7) and the non-assignment to `$!` (Section 8.2).

### 1.3 Non-Goals

This specification intentionally omits a number of features that exist in the reference shells. The omissions are listed normatively in Section 13. Appendix B describes what it would take to add each omitted feature later. The absence of a feature from this document shall not be interpreted as an oversight.

### 1.4 Companion Policy Documents

This specification is written against two project-wide policy documents, which take precedence over any conflicting guidance implied here:

- [docs/IMPLEMENTATION_POLICY.md](../IMPLEMENTATION_POLICY.md) is the canonical source for project rules that implementation of this spec must follow, in particular: the `libc`-boundary rule (production `libc` calls live only under `src/sys/`); the banned-`std` list (no `std::fs::File`, `std::process::Command`, `std::path::{Path, PathBuf}`, `std::env::var`, `println!` / `format!` / `write!`, and so on; use `Vec<u8>` / `&[u8]` with `sys::` wrappers and `bstr::ByteWriter`); and the prohibition on `cfg(target_os = ...)` outside `src/sys/`. The path-representation choice in Section 6 — `/dev/fd/N` versus FIFO — is a runtime probe through `src/sys/`, not a compile-time `cfg` switch. If the two documents disagree, `IMPLEMENTATION_POLICY.md` wins.
- [docs/TEST_WRITING_GUIDE.md](../TEST_WRITING_GUIDE.md) is the canonical source for how the unit and integration tests that verify this spec shall be structured, including the unit-vs-integration split, the `trace_entries!` / `syscall_test!` conventions for fake-syscall unit tests, and the matrix-test conventions for end-to-end conformance. Every test-shape requirement made in Section 14 of this spec shall be interpreted in light of that guide; if the two documents disagree, `TEST_WRITING_GUIDE.md` wins.

## 2. The `bash_procsub` Option Gate

### 2.1 Definition

- `set -o bash_procsub` shall enable lexical recognition of `<(list)` and `>(list)`.
- `set +o bash_procsub` shall disable it.
- The default value of `bash_procsub` on shell startup shall be off, for both interactive and non-interactive shells. A fresh meiksh invocation rejects `<(...)` and `>(...)` as a syntax error per Section 9.1.
- The reportable options output produced by `set -o` shall include the line `bash_procsub      on` or `bash_procsub      off` reflecting the current state, using the same column formatting used by the other POSIX `set -o` options.
- `bash_procsub` shall not be exposed through a short option letter. The value of `$-` shall not gain a new character when `bash_procsub` is enabled.

### 2.2 Naming Rationale

The option name follows the convention established in [ps1-prompt-extensions.md § 2.1.1](ps1-prompt-extensions.md): each bash-flavored extension to POSIX is gated by its own `bash_*` option, with the suffix encoding the feature in a form the reader can decode without consulting the documentation. `bash_procsub` is a contraction of "bash process substitution"; the choice of `procsub` rather than `process_substitution` follows the precedent set by zsh's and ksh's internal naming and avoids a 25-character option name.

### 2.3 Capture Semantics

- The shell shall sample the `bash_procsub` option at the moment a command line is **parsed**, not when it is executed.
- A command line that contains a `<(` or `>(` token while the option is off at parse time shall be rejected as a syntax error per Section 9.1, even if the option is subsequently turned on before the line would have run.
- A command line that contains a `<(` or `>(` token while the option is on at parse time shall execute the substitution per this specification, even if the option is subsequently turned off before the line is run.

### 2.4 Non-Interactive Shells

- Non-interactive shells shall honor `set -o bash_procsub` identically to interactive shells. Scripts written to depend on process substitution shall begin with `set -o bash_procsub` (or invoke meiksh with an explicit option-set startup file) before the first command that uses the construct.

## 3. Lexical Recognition

### 3.1 Tokens

When `bash_procsub` is on, the lexer shall recognize the two-byte sequences `<(` and `>(` as the opening of a process substitution if and only if all of the following conditions hold:

- The two bytes are immediately adjacent. A `<blank>` between the `<` (or `>`) and the `(` shall not be recognized; the `<` is parsed as a redirection operator and the `(` as the opening of a subshell.
- Neither byte is preceded by an unquoted backslash. `\<(...)` and `\>(...)` shall be expanded as literal text.
- The two bytes are not inside a single-quoted or double-quoted string. Inside `"..."` and `'...'` the sequence is literal text.
- The two bytes are not inside a here-document body. Here-document content is consumed verbatim until the matching delimiter; substitutions inside the body follow the heredoc-quoting rules from POSIX § 2.7.4 and are out of scope for this construct.

### 3.2 Body

- The byte stream after the opening `<(` or `>(` shall be parsed as a complete shell list per the POSIX Shell Command Language § 2.10.2 *List Grammar*.
- The list is terminated by the matching `)` that balances the opening `(`. Parentheses inside command substitution `$(...)`, inside subshells `(...)`, and inside arithmetic `$((...))` count toward the balance the same way they do for command substitution.
- An unterminated list (cursor or end-of-input reached while the parenthesis stack is non-empty) shall produce a syntax error at the position of the opening `<(` or `>(`. Interactive shells in this state shall continue prompting with `PS2` per the POSIX rules for incomplete commands.

### 3.3 Disambiguation From Existing Operators

- `<(`: distinguished from `<` followed by a subshell-opening `(` by the no-blank rule in Section 3.1. `cat < (echo hi)` is a redirection from a non-existent file named `(echo hi)`; `cat <(echo hi)` is process substitution. This matches bash and ksh.
- `>(`: distinguished from `>` followed by a `(` similarly. There is no standalone `>(...)` redirection form in POSIX; the `>` is followed by a *word* under § 2.7, and the bare `(` is not a word, so the disambiguation collapses to "is `bash_procsub` on?"
- `<<(`: the lexer shall greedily match the `<<` heredoc operator first. `cat <<(EOF)` is an attempt at a here-document with a parenthesized delimiter and shall be rejected per the heredoc grammar; `cat << (EOF)` would attempt the same thing with whitespace. To use process substitution after a heredoc operator, the user shall insert a blank: `cat <<EOF; cmd <(producer)`.
- `<>(`: the `<>` read-write redirection operator is matched first. `<>(`, with no blank, is `<>` followed by `(` and is not process substitution.

### 3.4 Recognition Inside Substitutions

- Inside an unquoted `$(...)` command substitution, `<(` and `>(` shall be recognized (assuming `bash_procsub` is on).
- Inside a backtick `` `...` `` command substitution, `<(` and `>(` shall be recognized.
- Inside an arithmetic `$((...))` expansion, `<(` and `>(` shall **not** be recognized; the contents are an arithmetic expression where `<` and `>` are comparison operators.

## 4. Word Semantics

### 4.1 Single Word

- Every process-substitution expression shall lex as a single shell word.
- The word's representation in the AST shall be a distinct expansion node (Section 11.2), not a string of literal bytes.
- Field splitting (POSIX § 2.6.5) shall not split a process-substitution word, regardless of `IFS`. The word is opaque to splitting.
- Pathname expansion (POSIX § 2.6.6) shall not be applied to the substitution's expanded path. The `/dev/fd/N` or FIFO path is the literal string the consuming command sees.
- Quote removal (POSIX § 2.6.7) does not apply: the substitution itself was unquoted by Section 3.1; there is nothing to remove.

### 4.2 Concatenation With Surrounding Text

- A process substitution may be juxtaposed with adjacent unquoted bytes. Adjacent text shall concatenate with the expansion result the same way other expansions concatenate. For example, `prefix<(echo hi)suffix` shall expand to a single word `prefix/dev/fd/Nsuffix` (or `prefix<FIFO-path>suffix`).
- This matches bash and ksh. zsh prepends the path with a `=` when used with the `=()` alternative form, which is out of scope here.

### 4.3 Position In A Simple Command

- A process substitution may appear in any position where a word is expected: as `command-name`, as an argument, as the *word* operand of a redirection, or as part of an assignment word's value.
- Using a process-substitution word as `command-name` shall execute the file the path resolves to. Because the substitution opens a pipe, the path is not generally executable, so this usage shall ordinarily fail with `EACCES` or `ENOEXEC` in the same way an unreadable filename would. Implementations shall not special-case the failure.

## 5. Process Model

### 5.1 Subshell

- The shell shall fork a child process to run `list`. The child shall be a subshell as defined in POSIX § 2.13 (Shell Execution Environment): it inherits a copy of the parent's variables, function definitions, signal traps, and shell options.
- The subshell's traps shall be reset to the default disposition for every signal except those the parent has set to ignore, per the POSIX subshell rules.
- The subshell shall execute `list` and exit when `list` completes, with the subshell's exit status set to `list`'s last command's exit status.

### 5.2 File Descriptor Wiring

- Before fork, the shell shall create a pipe via `pipe(2)`.
- For `<(list)`:
  - The subshell shall connect the **write** end of the pipe to its standard output (file descriptor 1).
  - The parent shall hold the **read** end of the pipe as the substitution file descriptor.
- For `>(list)`:
  - The subshell shall connect the **read** end of the pipe to its standard input (file descriptor 0).
  - The parent shall hold the **write** end of the pipe as the substitution file descriptor.
- After the dup, the original pipe ends shall be closed in both processes.
- Standard error of the subshell shall be inherited from the parent; the implementation shall not redirect stderr unless the user does so explicitly inside `list`.

### 5.3 Multiple Substitutions In One Command

- A single command may contain multiple process substitutions. Each substitution shall be wired through its own pipe, with its own subshell, and its own substitution file descriptor.
- The order of evaluation shall be left-to-right: the leftmost `<(...)` or `>(...)` is forked first, the next is forked next, and so on. This matches bash 5.2.
- All substitutions in a command shall be alive concurrently for the duration of the command.

### 5.4 Inherited File Descriptors

- The subshell shall inherit the parent shell's file descriptor table at the moment of fork, modified only by the standard-input or standard-output dup described in Section 5.2.
- File descriptors opened by the parent for redirection of the consuming command shall not be visible to the subshell; the parent applies its own redirection only after the subshell has been forked.

## 6. Path Representation

### 6.1 Probe

- At shell startup, the implementation shall probe whether the directory `/dev/fd` exists and contains entries that resolve as symbolic references to currently-open file descriptors. The probe shall be a runtime probe; it shall not be a compile-time `cfg(target_os = ...)` switch, per [docs/IMPLEMENTATION_POLICY.md § Portability policy](../IMPLEMENTATION_POLICY.md).
- The probe result shall be cached for the lifetime of the shell.

### 6.2 `/dev/fd` Path

- When the probe in Section 6.1 succeeds, the substituted word shall be the literal byte string `/dev/fd/N`, where N is the substitution file descriptor described in Section 5.2.
- The fd N shall be chosen to be unused at the time of the substitution. The implementation shall not reserve any specific fd range, but shall not collide with the fds 0, 1, 2 nor with fds the user has explicitly requested via `{var}<` (location-style redirection, when implemented).

### 6.3 FIFO Path

- When the probe in Section 6.1 fails, the implementation shall fall back to a named FIFO.
- The FIFO shall be created via `mkfifo(2)` under the directory named by `${TMPDIR:-/tmp}`.
- The FIFO's basename shall be `meiksh-procsub.<pid>.<seq>`, where `<pid>` is the parent shell's process id and `<seq>` is a per-shell incrementing counter that starts at 1.
- The substituted word shall be the FIFO's full path.
- The FIFO shall be created with permissions `0600` (`S_IRUSR | S_IWUSR`).
- The parent and subshell shall coordinate so that the consuming command, on opening the FIFO, blocks until the subshell has the other end open. This is the standard POSIX `mkfifo` rendezvous behavior.

### 6.4 Implementation-Defined But Observable

- Whether the path is `/dev/fd/...` or a FIFO under `$TMPDIR` is implementation-defined. Conforming scripts shall not depend on the form. The path format is observable through `ls`, `printf %s`, and similar utilities, so cross-system tests that string-match the path shall normalize.

## 7. Lifetime And Cleanup

### 7.1 Subshell Lifetime

- The subshell shall be alive concurrently with the consuming command. The shell shall not block on the subshell before launching the consuming command.
- After the consuming command exits, the parent shell shall:
  1. Close the parent-side substitution file descriptor (the read end of the `<(...)` pipe or the write end of the `>(...)` pipe).
  2. Reap the subshell with `waitpid(2)`. If the subshell has not yet exited, the close in step 1 shall arrive at the subshell as either an EOF on its stdin (for `>(...)`) or a `SIGPIPE` on the next write to its stdout (for `<(...)`); typical subshells exit shortly after.
  3. If a FIFO was created (Section 6.3), unlink it.

### 7.2 Subshell Refusing To Exit

- A subshell that ignores `SIGPIPE` and never reads its stdin (for a `<(...)` whose consumer never reads) may not exit promptly. The parent shall reap it with `waitpid(2)` only after the consuming command exits; the parent shall not interpose any timeout or signal of its own.
- An interactive shell whose consuming command was backgrounded shall reap the subshell when the foreground command finishes. There is no "process-substitution job" in `jobs -l`; the subshell is not a foreground job.

### 7.3 Multiple Substitutions

- When multiple substitutions are alive for one command, the parent shall reap them in the order they were forked (left-to-right). This is observable only through diagnostic ordering and shall not affect the consuming command's exit status.

### 7.4 Failure During Setup

- If `pipe(2)`, `fork(2)`, or `mkfifo(2)` fails, the consuming command shall not be executed. The shell shall print a diagnostic per Section 9.2 and the command's exit status shall be 1 in non-interactive shells; in interactive shells the prompt shall return without running the consumer.
- Substitutions that succeeded before the failing one shall be cleaned up in the order opposite to their creation: file descriptors closed, subshells reaped, FIFOs unlinked.

## 8. Exit Status And Special Parameters

### 8.1 Exit Status

- The exit status of the consuming command shall not be modified by the presence of process substitution. It is the exit status the consumer would have produced with the same path passed as an argument from a regular file.
- The exit status of any substitution subshell shall not be merged into the consumer's exit status. There is no `pipefail`-equivalent for process substitution.

### 8.2 `$!`

- The substitution subshell's process id shall not be assigned to the special parameter `$!`. This matches bash and ksh; zsh does not assign it either.
- A shell expression following the consuming command shall not be able to retrieve the subshell's pid through any current shell parameter. (See Section 13.4 for the `wait $!`-like wishlist item that this spec defers.)

### 8.3 `wait`

- A `wait` builtin invocation with no arguments after a process substitution has been launched shall wait for ordinary background jobs only. It shall not wait for substitution subshells.
- A `wait` invocation with the substitution subshell's pid as an argument is undefined: the user has no portable way to obtain that pid (Section 8.2), so the spec does not give meaning to the construction.

## 9. Diagnostics

### 9.1 Syntax Errors With `bash_procsub` Off

- A command line containing the byte sequence `<(` or `>(` (per the recognition rules of Section 3.1) while `bash_procsub` is off shall produce a syntax error.
- The diagnostic shall be `meiksh: <line>: process substitution requires `set -o bash_procsub'` and shall point at the column of the `<` or `>` character. The line and column shall follow the same conventions as POSIX syntax errors elsewhere in the shell.
- The shell shall not silently fall through to "unknown redirection target". The explicit message exists so users porting bash scripts learn the option name on the first failure.

### 9.2 Resource-Setup Errors

- A failure of `pipe(2)`, `fork(2)`, `mkfifo(2)`, `dup2(2)`, or `unlink(2)` during cleanup shall produce a diagnostic of the form `meiksh: process substitution: <syscall>: <strerror>`, written to standard error.
- A failure during setup shall cause the consuming command not to run, per Section 7.4. A failure during cleanup (e.g., `unlink(2)` of a FIFO that was already removed) shall be reported but shall not change the exit status of the consuming command.

### 9.3 Unterminated List

- An unterminated list inside `<(...)` or `>(...)` shall produce the standard meiksh "unterminated parenthesized list" syntax error, with the column pointing at the opening `<(` or `>(`. This is the same message produced for an unterminated `$(...)` or `(...)` subshell.

## 10. Interaction With Other Subsystems

### 10.1 Quoting

- Process substitution shall be recognized only in unquoted contexts (Section 3.1). Inside `"..."` or `'...'` the bytes are literal.
- Quote removal does not apply to the expanded path; the path is already a literal byte string with no shell metacharacters.

### 10.2 Redirection

- Process substitution shall compose with the standard POSIX redirections. `cmd < <(producer)` is the canonical pattern: the consumer reads from the substitution path through its stdin redirection, with the same effect as if the substitution path were a regular file.
- The `<` operator and the `<(` opening must be separated by a `<blank>`. Without it, the lexer matches the `<<` heredoc operator first per Section 3.3.
- `cmd > >(consumer)` is the symmetric pattern: the producer writes to the substitution path through its stdout redirection.

### 10.3 Pipelines

- A pipeline element may contain process substitutions. Each substitution lives in the consuming pipeline element's process; the substitution's subshell is **not** a member of the pipeline.
- `cmd1 | cmd2 <(producer)` runs `cmd1` and `cmd2` as a two-element pipeline. The `<(producer)` subshell runs alongside the `cmd2` process; it is not connected to `cmd1`'s stdout.

### 10.4 Subshells And Functions

- A process substitution inside a `(...)` subshell, a `{ ...; }` group, or a function body shall behave per this specification, with the substitution subshell forked from the enclosing scope. The cleanup in Section 7 shall happen in the enclosing scope at the point the consuming command exits.
- Returning from a function or exiting a subshell shall not orphan a still-open substitution: the cleanup is tied to the consuming command, not the function or subshell boundary.

### 10.5 Job Control

- The substitution subshell shall not appear in the `jobs` builtin's output; it is not a job in the POSIX sense.
- The subshell shall not be placed in a separate process group from the parent shell. It shall run in the same process group as the consuming command, which means a `SIGINT` from the controlling terminal is delivered to it as well as to the consumer.

### 10.6 Traps

- The parent shell's `trap` settings shall not fire on the subshell's exit, per the POSIX subshell rules.
- The subshell shall reset traps to the default disposition for every signal not explicitly ignored by the parent, per Section 5.1.

### 10.7 Restricted Shells

- A restricted shell (`set -r`) shall not enable `bash_procsub` and shall reject any attempt to do so with `meiksh: bash_procsub: restricted`. This matches bash's policy of disabling shell-mutating features in restricted mode.

### 10.8 Interactive Line-Editor Completion

- When `bash_procsub` is on, the position immediately after a `<(` or `>(` opener shall be treated by the line editor's TAB-completion dispatcher as a fresh argv[0] frame: command completion (builtins, aliases, functions, hashed commands, and `$PATH` executables) shall fire there, the same way it does after `$(`, after `(` opening a subshell, after a backtick, after a pipe `|`, or after a list separator `;` / `&`.
- When `bash_procsub` is off, the editor shall **not** treat that position as argv[0]. The line will not parse anyway (Section 9.1), and offering command completion would mislead the user into typing more of an unparseable construct. The dispatcher shall fall back to its non-command-position cascade (variable expansion if the prefix begins with `$`, tilde expansion if it begins with `~`, otherwise filename completion) the same way it would for any other byte sequence the lexer would reject.
- The gate is sampled at the moment TAB is pressed, not when the line was started. Toggling `bash_procsub` mid-edit is permitted; the next TAB observes the new value. This is consistent with how every other editor classification is computed fresh on each keystroke.
- The disambiguation between `<(` (process-substitution opener) and `<` followed by `(` (redirection then subshell — Section 3.3) shall use the same adjacency rule the lexer uses: only the byte immediately before the `(`, with no blank between, counts. `< (cmd` therefore remains an argv[0] position regardless of `bash_procsub`, because the `(` opens a bare subshell, not a process substitution.
- This requirement is implemented in `src/interactive/emacs_editing/functions.rs` (`is_command_position`, which takes the `bash_procsub` flag explicitly) and verified by unit tests `command_position_after_lt_paren_only_when_procsub_on` and `command_position_after_gt_paren_only_when_procsub_on` in the same file.

## 11. Implementation Notes (Non-Normative)

### 11.1 Option Plumbing

- The `bash_procsub` option lives in `src/shell/options.rs` next to `bash_prompts`. The same `set -o` printer and the same long-option parser handle it; no short-letter alias.
- The lexer in `src/syntax/token.rs` reads `shell.options.bash_procsub` at the moment it considers a `<` or `>` byte. Since options are captured at parse time (Section 2.3), the lexer's view of the option is always the value at the start of the current parse.

### 11.2 AST Node

- A new variant `Word::ProcSubstitution { direction: Direction, list: Box<List> }` shall be added to `src/syntax/ast.rs`. The `Direction` enum is `Read` for `<(...)` and `Write` for `>(...)`. The `list` is the same parsed AST as for a `(...)` subshell.
- The expansion stage in `src/expand/word.rs` shall not attempt to render the AST node into bytes itself; it shall delegate to `src/exec/procsub.rs` (new) for the fork+pipe and the path resolution. The expansion result is a single `Vec<u8>` containing the path.

### 11.3 Sys Layer

- `pipe2(2)` with `O_CLOEXEC` shall be added to `src/sys/process.rs` if not already present, with a fallback to `pipe(2)` + `fcntl(F_SETFD, FD_CLOEXEC)`.
- `mkfifo(2)` shall be exposed under `src/sys/fs.rs`. The FIFO-creation tests in [tests/integration/sys.rs](../../tests/integration/sys.rs) cover the libc boundary.
- The `/dev/fd` probe shall use `sys::fs::stat` on `/dev/fd` and check for `S_IFDIR`. The probe's result is cached on `Shell` once.

### 11.4 Lifetime Tracking

- `Shell` shall gain a field `procsub_active: Vec<ProcSubLease>` where `ProcSubLease` carries the parent-side fd, the subshell pid, and an optional FIFO path. The leases are pushed when a substitution is forked and drained at the end of the consuming command.
- Cleanup runs in `src/exec/simple.rs` after the consuming command's `waitpid`, before the next prompt is drawn.

### 11.5 No `cfg(target_os)`

- Per [docs/IMPLEMENTATION_POLICY.md § Portability policy](../IMPLEMENTATION_POLICY.md), `/dev/fd` availability is determined at runtime, not at compile time. The `cfg(unix)` guard on the whole `sys` module is unaffected; this construct is Unix-only and does not need a per-OS branch.

## 12. Interaction With Other Subsystems (Forward References)

- **Coproc / `coproc` builtin** — this spec does not introduce `coproc`. See Section 13.1.
- **`bash_arrays`** — when arrays land, a process substitution shall be permitted as an array element. The single-word rule in Section 4.1 makes this trivial.
- **`bash_prompts`** — `PS1` / `PS2` are not parsed as shell input; process substitution is not recognized inside prompt strings, regardless of `bash_procsub`.
- **`set -o vi` / `set -o emacs`** — interactive editing of a line containing `<(...)` is unaffected; the shell only parses the line on accept.

## 13. Non-Goals

### 13.1 `coproc` Builtin

- The bash `coproc` builtin and ksh `|&` co-process operator are out of scope. They share infrastructure with process substitution (subshell + pipe) but expose the channel as a pair of variables `${COPROC[0]}` and `${COPROC[1]}` rather than as a path. Adding `coproc` later would reuse the lease tracking from Section 11.4.

### 13.2 zsh `=()` Form

- zsh's `=()` form runs the list, captures its stdout to a temporary file, and substitutes the file's path. Bash and ksh do not provide it; the substitution this spec defines uses pipes / FIFOs and is non-buffering.

### 13.3 Reading The Substitution's Exit Status

- Bash, ksh, and zsh all decline to expose the substitution subshell's exit status. This spec does the same. Users who need the exit status shall use a regular pipeline or a temporary file.

### 13.4 `$!` Assignment

- The substitution subshell's pid shall not be assigned to `$!` (Section 8.2). Bash made this decision deliberately and the spec inherits it.

### 13.5 Process Substitution Inside Arithmetic

- `<(` is not recognized inside `$((...))` (Section 3.4). This matches bash and is not negotiable: the arithmetic grammar uses `<` and `>` as relational operators, and overloading them would force a context-dependent token stream.

### 13.6 Process Substitution In `case` Patterns

- The `word` operand of a `case` statement is matched as a pattern, not as a path. A process substitution in that position shall be lexed normally but the pattern match against the resulting `/dev/fd/N` path is rarely useful. This spec does not forbid it but does not document it as a use case.

### 13.7 Versioned Compat Modes

- Following the convention in [ps1-prompt-extensions.md § 13.7](ps1-prompt-extensions.md), this spec does not introduce a `BASH_COMPAT` versioning knob for the substitution syntax. Once `bash_procsub` is on, behavior is fixed by this document.

### 13.8 Short Option Letter

- `bash_procsub` shall not be exposed through a short option letter. The shell already has more short letters than fingers, and short letters are reserved for POSIX-mandated options.

## 14. Testing

This section is normative.

### 14.1 Unit Tests

- The lexer's recognition rules (Section 3) shall be covered by unit tests in `src/syntax/token.rs`, including: blank between `<` and `(` not recognized; recognition inside `$(...)` and backticks; non-recognition inside `'...'`, `"..."`, `$((...))`, and after backslash.
- The path-probe logic (Section 6.1) shall be covered by unit tests in `src/sys/fs.rs` that drive the probe through the `trace_entries!` mock.
- The lease-tracking logic (Section 11.4) shall be covered by unit tests in `src/exec/procsub.rs` that mock `pipe2`, `fork`, `dup2`, `mkfifo`, `unlink`, and `waitpid` per [docs/TEST_WRITING_GUIDE.md § Syscall trace model](../TEST_WRITING_GUIDE.md#syscall-trace-model).

### 14.2 Integration Tests

- End-to-end behavior shall be covered by integration tests in `tests/integration/process_substitution.rs` (new file). At minimum:
  - `<(...)` with a producer that writes a known string, consumed by `cat`.
  - `>(...)` with a consumer that reads stdin, fed by `echo`.
  - Multiple substitutions in one command, asserting all four arguments distinct.
  - Substitution inside a pipeline.
  - Substitution inside a function body.
  - Cleanup: assertion that no `meiksh-procsub.*` FIFOs remain in `$TMPDIR` after the command exits.
  - `$!` not modified by substitution.
  - Syntax error when `bash_procsub` is off.

### 14.3 Matrix Tests

- Matrix tests under `tests/matrix/non_posix/process-substitution.md` shall reference this spec and verify each numbered requirement in Sections 2 through 9. The matrix tests run the built shell binary as a black box per [docs/IMPLEMENTATION_POLICY.md § Test Policy](../IMPLEMENTATION_POLICY.md#test-policy).

### 14.4 Coverage

- The 99.5% production-line coverage floor shall hold. The implementation shall not introduce code paths that are unreachable through the unit and integration suites combined; any defensive arm that the type checker requires but no test can reach shall be removed per [docs/IMPLEMENTATION_POLICY.md § Coverage Policy](../IMPLEMENTATION_POLICY.md#coverage-policy).

## Appendix A — Comparison With Reference Shells

### A.1 Behavior Matrix

| Aspect | meiksh (this spec) | bash 5.2 | ksh93 | zsh 5.9 |
|---|---|---|---|---|
| Option gate | `set -o bash_procsub` | always on (off in POSIX mode) | always on | always on |
| `/dev/fd` preferred | yes when probed | yes | yes | yes |
| FIFO fallback | yes, `${TMPDIR:-/tmp}/meiksh-procsub.<pid>.<seq>` | yes, `/tmp/sh-np.<XX>` | yes, `/tmp/ksh.<pid>.<seq>` | yes |
| Subshell pid in `$!` | no | no | no | no |
| Multiple per command | yes, all concurrent | yes | yes | yes |
| Recognized inside `$((...))` | no | no | no | no |
| Trap reset in subshell | yes (POSIX § 2.13) | yes | yes | yes |
| `=(...)` zsh form | no | no | no | yes |
| `coproc` builtin | no (Section 13.1) | yes | no (uses `\|&`) | no |

### A.2 Sample Sessions

```sh
# Diff two command outputs without temp files:
$ set -o bash_procsub
$ diff <(printf 'a\nb\n') <(printf 'a\nc\n')
2c2
< b
---
> c

# Tee to two consumers:
$ set -o bash_procsub
$ echo hello | tee >(grep h) >(wc -c) >/dev/null
hello
6
```

### A.3 Sample Errors

```sh
# bash_procsub off (default):
$ diff <(echo a) <(echo b)
meiksh: 1: process substitution requires `set -o bash_procsub'

# Blank between `<` and `(`:
$ set -o bash_procsub
$ cat < (echo hi)
meiksh: 1: syntax error near unexpected token `('
```

## Appendix B — Path To Full Bash Process-Substitution Parity

Each numbered package is self-contained and can be implemented in any order after the core spec in Sections 1 through 14 lands.

### B.1 Package 1 — `coproc` Builtin

- New builtin `coproc [name] command [redirections]`.
- Reuses the lease tracking from Section 11.4.
- Exposes the substitution channel as `${name[0]}` (read fd) and `${name[1]}` (write fd) where `name` defaults to `COPROC`.
- Estimated work: 2 weeks (parser + builtin + array dependency).

### B.2 Package 2 — `wait $procsub_pid`

- New shell parameter `${PROCSUB!}` that records the pid of the most recently launched substitution subshell.
- `wait` shall accept the pid and return the subshell's exit status.
- Diverges from bash; matches no reference shell. Worth doing only if user demand is high.

### B.3 Package 3 — `=( list )` Buffered Form

- zsh's `=(list)`: run `list`, capture stdout to a temp file, substitute the file's path. The file is unlinked when the consumer exits.
- Symmetric `=(>list)` for the write direction is not in zsh and would not be added.
- Estimated work: 1 week (one new AST variant, one new exec path; reuses cleanup).

### B.4 Package 4 — Bash `BASH_COMPAT` Interactions

- A `BASH_COMPAT=42` environment knob that re-enables a pre-bash-4.3 quirk where the substitution subshell's stderr was attached to the parent's stderr only when the parent's stderr was not a terminal. Almost no one needs this; it is listed for completeness.
