# Meiksh POSIX Traceability Matrix

This document maps the core POSIX shell requirements that `meiksh` is targeting to code and test locations in this repository.

## Standards Baseline

- Primary semantic target: POSIX Issue 8 / IEEE Std 1003.1-2024
- Compatibility watchlist: POSIX Issue 7 / POSIX.1-2017 behavior still exercised by current certification-era suites
- System interface baseline: process, fd, signal, process-group, and terminal APIs needed by `sh`

## Normative References In Repo

- `docs/posix/issue8/shell-command-language.html`
- `docs/posix/issue8/sh-utility.html`
- `docs/posix/issue8/shell-rationale.html`
- `docs/posix/issue7/shell-command-language.html`
- `docs/posix/issue7/sh-utility.html`
- `docs/posix/validation/posix-test-suites.html`
- Shell builtin utility pages:
  - `docs/posix/utilities/alias.html`
  - `docs/posix/utilities/bg.html`
  - `docs/posix/utilities/break.html`
  - `docs/posix/utilities/cd.html`
  - `docs/posix/utilities/command.html`
  - `docs/posix/utilities/continue.html`
  - `docs/posix/utilities/dot.html`
  - `docs/posix/utilities/eval.html`
  - `docs/posix/utilities/exec.html`
  - `docs/posix/utilities/exit.html`
  - `docs/posix/utilities/export.html`
  - `docs/posix/utilities/fg.html`
  - `docs/posix/utilities/jobs.html`
  - `docs/posix/utilities/pwd.html`
  - `docs/posix/utilities/read.html`
  - `docs/posix/utilities/readonly.html`
  - `docs/posix/utilities/return.html`
  - `docs/posix/utilities/set.html`
  - `docs/posix/utilities/shift.html`
  - `docs/posix/utilities/times.html`
  - `docs/posix/utilities/trap.html`
  - `docs/posix/utilities/umask.html`
  - `docs/posix/utilities/unalias.html`
  - `docs/posix/utilities/unset.html`
  - `docs/posix/utilities/wait.html`
- Shell runtime/system interface pages:
  - `docs/posix/functions/close.html`
  - `docs/posix/functions/dup2.html`
  - `docs/posix/functions/exec.html`
  - `docs/posix/functions/fork.html`
  - `docs/posix/functions/isatty.html`
  - `docs/posix/functions/kill.html`
  - `docs/posix/functions/open.html`
  - `docs/posix/functions/pipe.html`
  - `docs/posix/functions/setpgid.html`
  - `docs/posix/functions/sigaction.html`
  - `docs/posix/functions/tcgetpgrp.html`
  - `docs/posix/functions/tcsetpgrp.html`
  - `docs/posix/functions/waitpid.html`

## Parser And Grammar

- Token recognition, quoting preservation, operators: `src/syntax/mod.rs`
- Program/list/pipeline/simple command AST: `src/syntax/mod.rs`
- Current tests:
  - `cargo test` unit tests in `src/syntax/mod.rs`
- Gaps to close:
  - full reserved-word handling
  - alias substitution timing

## Expansion

- Parameter, command, arithmetic, quote removal, field splitting, pathname expansion: `src/expand.rs`
- Shell context bridge for expansions: `src/shell.rs`
- Current tests:
  - `cargo test` unit tests in `src/expand.rs`
  - integration tests in `tests/spec/basic.rs`
  - differential tests in `tests/differential/portable.rs`
- Gaps to close:
  - remaining `${...}` operators such as pattern trimming forms
  - finer POSIX field-splitting corner cases around mixed quoting and IFS interactions
  - issue-8 `$'...'` semantics

## Execution

- Program/list/pipeline execution: `src/exec/mod.rs`
- External process and environment handoff: `src/exec/mod.rs`, `src/shell.rs`
- Current tests:
  - integration tests in `tests/spec/` and `tests/differential/`
- Gaps to close:
  - tighter subshell fidelity and more shell-language edge cases

## Builtins

- Builtin dispatch and current implementations: `src/builtin/mod.rs`
- Required shell state mutations: `src/shell.rs`
- Gaps to close:
  - full POSIX special builtin semantics
  - trap persistence and signal integration

## Interactive Behavior

- Prompt loop, `ENV`, simple history: `src/interactive/mod.rs`
- Job table and background coordination: `src/shell.rs`
- Gaps to close:
  - line editing modes
  - POSIX history semantics
  - tty foreground process-group handoff

## System Interface Layer

- Manual Unix bindings and wait-status decoding: `src/sys/mod.rs`
- Gaps to close:
  - `waitpid` integration with the runtime
  - signal disposition wrappers
  - pgid and tty control in job-control paths

## Validation Lanes

- Unit tests: `cargo test`
- Spec-oriented shell tests: `tests/spec/`
- Differential behavior checks: `tests/differential/`
- Performance harnesses: `tests/perf/`
