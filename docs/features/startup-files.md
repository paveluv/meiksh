# Interactive Startup Files

## Status

**Implemented.** The loader lives in [src/interactive/startup.rs](../../src/interactive/startup.rs) and is invoked once by [`Shell::run`](../../src/shell/run.rs) whenever the shell is interactive. Every normative statement of this spec is exercised by the unit tests colocated with the loader (trace-driven coverage of each file-source path, the `MEIKSH_VERSION` marker, `$HOME`/`$ENV` handling, and the setuid guard) and by the `interactive_shell_*` integration tests in [tests/integration/shell_options.rs](../../tests/integration/shell_options.rs) (end-to-end `~/.profile`, `$ENV`, and `MEIKSH_VERSION` sourcing through a real `meiksh -i` subprocess).

## 1. Scope

This document specifies, for meiksh, the exact list of configuration files sourced at shell startup, the order in which they are sourced, and the environment markers established before sourcing. It applies only to shell configuration (that is, files interpreted as meiksh source). Line-editor configuration (`$INPUTRC`, `$HOME/.inputrc`, `/etc/inputrc`) is out of scope and is specified separately in [inputrc.md](inputrc.md).

### 1.1 Conformance Language

The key words "shall", "shall not", "should", "should not", "may", and "must" in this document are to be interpreted as described in RFC 2119.

### 1.2 Relationship to POSIX

POSIX.1-2024 XCU `sh` defines exactly one startup-file obligation for an interactive shell: if the variable `ENV` is set when the shell starts, the shell shall subject its value to parameter expansion, interpret the result as a pathname, and source the file if the pathname is absolute and the file is accessible. Meiksh implements that obligation in full (Section 3.3 below). Everything else in this document is a meiksh extension, following the de-facto convention established by Bourne, ksh, and bash.

## 2. When Startup Files Are Sourced

Startup-file sourcing, as specified in Section 3, shall happen if and only if **all** of the following are true at the moment `Shell::run` is called:

- The shell is interactive — either `-i` was passed on the command line, or standard input is a terminal and no script/`-c` string was supplied.
- The identity guard of Section 4 permits it.

A non-interactive shell, including one invoked with `-c` or as `sh script.sh`, shall not source any of the files in Section 3. POSIX explicitly leaves `$ENV` to implementations for non-interactive shells; meiksh chooses to ignore it there so that scripts run in a predictable, user-independent environment.

## 3. Files and Order

When Section 2 permits sourcing, meiksh shall source the following files, in this order. Each step is independent: a missing file shall be silently skipped; the next step shall still run.

### 3.1 `/etc/profile`

If the path `/etc/profile` exists and is readable, it shall be sourced. This is the traditional system-wide interactive profile. Administrators use it to establish site-wide defaults (`PATH` additions, `umask`, shell banners). Meiksh sources it for every interactive invocation, not just for login shells, because meiksh does not model the login/non-login distinction.

### 3.2 `$HOME/.profile`

If `$HOME` is set to a non-empty, absolute pathname, `$HOME/.profile` shall be resolved by concatenation (inserting a single `/` if `$HOME` does not already end with one) and sourced if the resulting path exists and is readable. If `$HOME` is unset, empty, or not absolute, this step shall be silently skipped; meiksh shall not fall back to `getpwuid(3)`.

### 3.3 `$ENV`

If the variable `ENV` is set, its value shall be subjected to parameter expansion (the same expansion that applies to double-quoted strings in shell source, including `$NAME`, `${NAME}`, and default-value forms). The result shall be interpreted as a pathname. If the pathname is absolute and the file exists and is readable, it shall be sourced. If the pathname is not absolute or the file does not exist, the step shall be silently skipped. Parameter expansion errors shall abort startup with the same exit status as a syntax error in the expanded source.

This step runs after Sections 3.1 and 3.2 specifically so that `/etc/profile` or `$HOME/.profile` may set or override `$ENV` for the current shell, and that redirection shall take effect.

### 3.4 Error Handling

If a file from Section 3 exists but contains a syntactic or semantic error, sourcing shall fail for that file. The failure shall propagate out of startup and terminate the shell with the sourcing file's exit status, exactly as it would for a `. FILE` invocation from the prompt. Meiksh shall not attempt to continue with later files after an earlier one has failed.

## 4. Identity Guard

Before sourcing any file from Section 3, meiksh shall compare the process's real and effective user IDs, and the process's real and effective group IDs. If any of these pairs differ (the shell is running setuid or setgid), meiksh shall skip all of Section 3 and proceed directly to the interactive REPL. This matches the POSIX-specified guard for `$ENV` and extends it, conservatively, to `/etc/profile` and `$HOME/.profile` so that a privileged meiksh can never be tricked into executing attacker-controlled system or user state.

The `MEIKSH_VERSION` marker in Section 5 shall still be established under this guard.

## 5. The `MEIKSH_VERSION` Marker Variable

At the beginning of interactive startup, *before* any file from Section 3 is sourced and *before* the identity guard is evaluated, meiksh shall:

1. Set the shell variable `MEIKSH_VERSION` to the crate's SemVer string (for example, `0.1.1`). The exact value is determined at compile time from Cargo's package version.
2. Mark `MEIKSH_VERSION` exported, so that any child process spawned by a startup file — or by any command run later in the interactive session — inherits it via `execve(2)`.

This name mirrors the convention used by every other major interactive shell: `BASH_VERSION`, `ZSH_VERSION`, `KSH_VERSION`, `FISH_VERSION`. Unlike bash's and zsh's markers, `MEIKSH_VERSION` is **exported** by default so that `/etc/profile` and `$HOME/.profile` — which are routinely shared across shells — can detect meiksh from inside the same startup pass that decides what shell-specific configuration to apply.

The marker exists so that portable startup scripts can branch on shell identity. The following idiom shall behave as intended when included in `/etc/profile` or `$HOME/.profile`:

```sh
if [ -n "${MEIKSH_VERSION:-}" ]; then
    # meiksh-specific setup
fi
```

`MEIKSH_VERSION` is deliberately a presence-plus-value signal. Its *value* follows SemVer and will change across meiksh releases, so scripts that want portability shall check only for presence (`[ -n "$MEIKSH_VERSION" ]`). Scripts that need version-aware behavior may parse the value using normal SemVer rules; meiksh shall not change the format of the string within a release.

The user (or a startup file) may `unset MEIKSH_VERSION` after it has been inspected; meiksh shall not restore the variable.

## 6. Files Meiksh Does Not Load

The following files, loaded by one or more of bash, ksh, or zsh at startup, shall not be sourced by meiksh:

- `/etc/bash.bashrc`, `~/.bashrc`, `~/.bash_profile`, `~/.bash_login`, `~/.bash_logout` — bash-specific, tied to bash's login/non-login/interactive-shell distinction, which meiksh does not model.
- `/etc/zprofile`, `/etc/zshrc`, `/etc/zshenv`, `~/.zprofile`, `~/.zshrc`, `~/.zshenv`, `~/.zlogin`, `~/.zlogout` — zsh-specific.
- `/etc/ksh.kshrc`, `~/.kshrc`, `$ENV` when non-interactive — ksh-specific variants.
- `~/.profile` in non-interactive mode — meiksh gates `~/.profile` on interactive mode (Section 2); the traditional POSIX login-profile behavior is not reproduced.

If a user wants bash-style `~/.bashrc` behavior under meiksh, they shall put the equivalent logic in `~/.profile` and gate it on `[ -n "$MEIKSH_VERSION" ]` per Section 5.

## 7. Non-Goals

- Meiksh shall not distinguish login from non-login interactive shells. There is no `--login` flag, no leading-dash argv-0 detection, and no separate login-profile path.
- Meiksh shall not source files based on remote/local heuristics (for example, by consulting `SSH_CONNECTION`).
- Meiksh shall not read a personal `~/.meikshrc`. All user-level startup logic belongs in `~/.profile`, gated on `$MEIKSH_VERSION` if shell-specific.

### 7.1 Internal Test Hook

For the benefit of meiksh's own integration tests, the variable `MEIKSH_SKIP_STARTUP_FILES`, when equal to the exact value `1` at the start of `Shell::run`, shall suppress every file from Section 3 while still establishing the `MEIKSH_VERSION` marker (Section 5). The PTY harness in `tests/integration/interactive_common/` sets this variable so that a developer's real `/etc/profile`, `~/.profile`, or `$ENV` cannot contaminate test assertions. This hook is internal and is deliberately omitted from user-facing documentation; it shall not be relied upon by ordinary scripts.
