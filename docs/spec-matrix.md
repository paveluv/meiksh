# Meiksh POSIX Traceability Matrix

This document maps the POSIX shell requirements mirrored under `docs/posix/` to the current `meiksh` implementation. The POSIX pages in `docs/posix/` are the only requirements source of truth for this matrix.

## Standards Baseline

- Primary semantic target: POSIX Issue 8 / IEEE Std 1003.1-2024
- Compatibility watchlist: Issue 7 shell behavior that may still matter for older validation suites
- Utility contract baseline: `docs/posix/issue8/sh-utility.html`
- Shell language baseline: `docs/posix/issue8/shell-command-language.html`
- Explanatory reference: `docs/posix/issue8/shell-rationale.html`

## Local Normative Mirror

- Issue 8 shell language: `docs/posix/issue8/shell-command-language.html`
- Issue 8 `sh` utility: `docs/posix/issue8/sh-utility.html`
- Issue 8 shell rationale: `docs/posix/issue8/shell-rationale.html`
- Issue 7 shell language: `docs/posix/issue7/shell-command-language.html`
- Issue 7 `sh` utility: `docs/posix/issue7/sh-utility.html`
- Validation reference index: `docs/posix/validation/posix-test-suites.html`
- Builtin utility pages: `docs/posix/utilities/*.html`
- Shell runtime/system interface pages: `docs/posix/functions/*.html`

## Utility Entry And Startup

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/sh-utility.html#tag_20_110_03` | Utility description and command-processing model | `src/main.rs`, `src/shell.rs` | `tests/spec/basic.rs` | `meiksh` runs shell command strings and script sources through `Shell` state and parser/executor layers. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_04` | Options | `src/shell.rs` | `src/shell.rs` unit tests, `tests/spec/basic.rs` | `-c`, `-n`, `-f`, and `-C` handling exists. Broader `sh` option coverage is incomplete. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_05` | Operands | `src/shell.rs` | `src/shell.rs` unit tests | Command-string and script-path paths exist. Full operand edge coverage remains narrower than the utility page. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_08` | Environment variables including `ENV`, `PS1`, `HISTFILE` | `src/interactive.rs`, `src/shell.rs` | `src/interactive.rs`, `tests/spec/basic.rs` | `ENV` is sourced only when absolute and present; `PS1` and `HISTFILE` are honored in the current simplified interactive layer. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_13` | Extended interactive description | `src/interactive.rs` | `src/interactive.rs` | Prompt loop and plain history append exist; command history list semantics and command-line editing modes are not implemented. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_14` | Exit status | `src/main.rs`, `src/shell.rs` | `tests/spec/basic.rs` | Basic command and syntax-check exit paths exist. Full utility-page exit-status coverage is still incomplete. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_15` | Consequences of errors | `src/shell.rs`, `src/builtin.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Non-interactive shell errors currently fail explicitly; some POSIX consequence distinctions still need tightening. |

## Tokenization, Grammar, And Parsing

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/shell-command-language.html#tag_19_02` | Quoting | `src/syntax.rs` | `src/syntax.rs`, `src/expand.rs` | Tokenization preserves raw quoting so later expansion stages can apply POSIX quote-sensitive behavior. |
| `docs/posix/issue8/shell-command-language.html#tag_19_03` | Token recognition | `src/syntax.rs` | `src/syntax.rs` | Operators, words, comments, separators, and here-doc token collection are implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_03_01` | Alias substitution | `src/syntax.rs`, `src/shell.rs`, `src/exec.rs` | `src/syntax.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Parser-time alias substitution exists, including blank-terminated alias chaining and same-source visibility across top-level and nested bodies. Alias recursion is bounded. Reserved-word interaction is still not exhaustively covered. |
| `docs/posix/issue8/shell-command-language.html#tag_19_04` | Reserved words | `src/syntax.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Core reserved words are recognized in many required positions. Exact reserved words are no longer accepted as function names, `for name` now accepts linebreaks before `in`, `do`/`done` are preserved as ordinary words inside `for ... in` wordlists, and `case WORD` now accepts the grammar-required linebreak before `in`. Some grammar positions still need more precise reserved-word recognition coverage. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_02` | Pipelines and leading `!` | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | `!` is parsed as pipeline negation only at pipeline start, and bare `!` in later command-start positions is rejected as syntax. Broader grammar-edge coverage is still incomplete. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_03` | Lists and AND-OR lists | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Sequential lists, `&&`, `||`, and asynchronous lists are implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_04` | Compound commands | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Groups, subshells, `for`, `case`, `if`, `while`, and `until` are parsed and executed. `case` now accepts the `Case WORD linebreak in` form and empty case lists with only linebreaks after `in`. Additional edge-case fidelity remains to be tightened. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_05` | Function definition command | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Shell functions are parsed and executed; function names now reject exact reserved words. |
| `docs/posix/issue8/shell-command-language.html#tag_19_10_01` | Grammar lexical conventions | `src/syntax.rs` | `src/syntax.rs` | Context-dependent treatment of `WORD`, `NAME`, reserved words, redirections, and function syntax is partly modeled, including the `for`-specific third-word distinction for `in`/`do` and the `case` third-word recognition of `in` after an optional linebreak. |
| `docs/posix/issue8/shell-command-language.html#tag_19_10_02` | Grammar rules | `src/syntax.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Main grammar productions are implemented. `for` clauses now respect the linebreak-before-`in` form and keep reserved-word-looking tokens in the wordlist as ordinary words, and `case` clauses now respect the `WORD linebreak in` form while rejecting non-grammar separators after `in`. The matrix still tracks remaining reserved-word-position work rather than claiming full grammar closure. |

## Expansion And Pattern Semantics

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/shell-command-language.html#tag_19_05` | Parameters and variables | `src/shell.rs`, `src/expand.rs` | `src/shell.rs`, `src/expand.rs`, `tests/spec/basic.rs` | Positional and special parameters are exposed through shell state and expansion hooks. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_01` | Tilde expansion | `src/expand.rs` | `src/expand.rs` | Implemented for leading `~` word forms currently supported by the expander. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_02` | Parameter expansion | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs` | Plain substitutions, length, default/assign/error/alternate operators, and multi-digit positionals are implemented. Pattern-trimming forms are still missing. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_03` | Command substitution | `src/expand.rs`, `src/shell.rs` | `src/expand.rs`, `tests/spec/basic.rs` | Implemented by recursively invoking the current `meiksh` binary with `-c`. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_04` | Arithmetic expansion | `src/expand.rs` | `src/expand.rs` | Integer arithmetic currently covers literals and `+`, `-`, `*`, `/`, and `%`. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_05` | Field splitting | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs` | IFS whitespace versus non-whitespace splitting is modeled. Further mixed-quoting and IFS corner cases remain open. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_06` | Pathname expansion | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | Implemented with dotfile suppression unless the pattern segment starts with `.`. `set -f` disables it. |
| `docs/posix/issue8/shell-command-language.html#tag_19_06_07` | Quote removal | `src/expand.rs` | `src/expand.rs` | Implemented as part of the word-expansion pipeline. |
| `docs/posix/issue8/shell-command-language.html#tag_19_02_04` | Dollar-single-quotes | `src/syntax.rs`, `src/expand.rs` | `src/syntax.rs`, `src/expand.rs` | Issue 8 `$'...'` semantics are still a tracked gap. |
| `docs/posix/issue8/shell-command-language.html#tag_19_14` | Pattern matching notation | `src/expand.rs`, `src/exec.rs` | `src/expand.rs`, `src/exec.rs` | Pattern matching is used for pathname expansion and `case` matching. Coverage exists for wildcard and bracket-class behavior already implemented. |

## Redirection

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/shell-command-language.html#tag_19_07` | General redirection model | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Redirections are parsed in the grammar and applied for simple, builtin, function, and compound-command execution. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_01` | Redirecting input | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Implemented, including numeric fd prefixes. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_02` | Redirecting output | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Implemented, including noclobber handling through `set -C`. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_03` | Appending output | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_04` | Here-document | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs` | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs`, `tests/spec/basic.rs` | `<<` and `<<-` are implemented. Parsing attaches bodies, tab stripping is honored for `<<-`, and expansion runs only for unquoted delimiters. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_05` | Duplicating input fd | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_06` | Duplicating output fd | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_07_07` | Read/write open fd | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |

## Command Execution Model

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/shell-command-language.html#tag_19_08_01` | Consequences of shell errors | `src/shell.rs`, `src/builtin.rs`, `src/exec.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Shell errors currently favor explicit failure paths. Some POSIX consequence distinctions remain open. |
| `docs/posix/issue8/shell-command-language.html#tag_19_08_02` | Exit status for commands | `src/exec.rs`, `src/shell.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Basic exit-status propagation exists for simple commands, pipelines, lists, and compound commands. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_01` | Simple commands | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Assignment handling, command name resolution, and redirection sequencing are implemented for the current supported feature set. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_01_03` | Commands with no command name | `src/exec.rs`, `src/shell.rs` | `src/exec.rs` | Assignment-only commands execute in the current shell state. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_01_04` | Command search and execution | `src/exec.rs`, `src/builtin.rs`, `src/shell.rs` | `src/exec.rs`, `src/builtin.rs`, `tests/spec/basic.rs` | Builtins, shell functions, and external command lookup are implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_01_06` | Non-built-in utility execution | `src/exec.rs`, `src/sys.rs` | `src/exec.rs`, `tests/spec/basic.rs` | External execution exists, including `ENOEXEC` fallback via `sh` for text executables. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_02_01` | Pipeline exit status | `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Regular pipeline status and `!` negation paths are implemented. `pipefail` support is not implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_03_02` | Asynchronous AND-OR lists | `src/exec.rs`, `src/shell.rs` | `src/exec.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Background job launch and tracking exist. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_03_04` | Sequential lists | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_03_06` | AND lists | `src/syntax.rs`, `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_03_08` | OR lists | `src/syntax.rs`, `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_09_04_01` | Grouping commands | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Groups and subshells are implemented. Tighter subshell fidelity is still tracked as an open area. |
| `docs/posix/issue8/shell-command-language.html#tag_19_13` | Shell execution environment | `src/shell.rs`, `src/exec.rs`, `src/builtin.rs` | `src/shell.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Current-shell state mutation versus pipeline/background execution is explicitly modeled for builtins and control flow. |

## Special Builtins And Related Utilities

| Utility page | Implementation | Status | Notes |
| --- | --- | --- | --- |
| `docs/posix/utilities/alias.html` | `src/builtin.rs`, `src/syntax.rs`, `src/shell.rs`, `src/exec.rs` | partial | Builtin exists and alias substitution is integrated into parsing. Remaining work is in the last reserved-word interaction gaps beyond the now-covered `for` and `case` linebreak-sensitive cases. |
| `docs/posix/utilities/bg.html` | `src/builtin.rs`, `src/shell.rs` | partial | Basic resume-by-job-id path exists. tty/job-control fidelity is incomplete. |
| `docs/posix/utilities/break.html` | `src/builtin.rs`, `src/exec.rs`, `src/shell.rs` | implemented with remaining edge review | Loop-depth validation and propagation exist. |
| `docs/posix/utilities/cd.html` | `src/builtin.rs` | partial | Core directory-change behavior exists. Broader utility-page option and environment semantics still need review. |
| `docs/posix/utilities/command.html` | `src/builtin.rs` | partial | Builtin now executes utilities while bypassing shell functions, supports `-p`, `-v`, and `-V`, and reports aliases/reserved words/builtins/path lookups. Detailed special-builtin parity and some execution edge cases still need review. |
| `docs/posix/utilities/continue.html` | `src/builtin.rs`, `src/exec.rs`, `src/shell.rs` | implemented with remaining edge review | Loop-depth validation and propagation exist. |
| `docs/posix/utilities/dot.html` | `src/builtin.rs`, `src/shell.rs` | partial | Sourcing by pathname exists. Search semantics and related edge cases remain to be reviewed. |
| `docs/posix/utilities/eval.html` | `src/builtin.rs`, `src/shell.rs` | partial | Re-executes joined arguments through the parser/executor. |
| `docs/posix/utilities/exec.html` | `src/builtin.rs`, `src/sys.rs` | partial | No-argument no-op and replacement execution exist. |
| `docs/posix/utilities/exit.html` | `src/builtin.rs`, `src/shell.rs` | implemented with remaining edge review | Basic status parsing and shell termination exist. |
| `docs/posix/utilities/export.html` | `src/builtin.rs`, `src/shell.rs` | partial | Variable export exists and `-p` now emits shell-reinput-safe quoting for exported names, including unset exported names. Remaining review is around unspecified no-operand behavior and finer special-builtin diagnostics. |
| `docs/posix/utilities/fg.html` | `src/builtin.rs`, `src/shell.rs` | partial | Basic foreground wait path exists. tty foreground handoff and output details remain open. |
| `docs/posix/utilities/jobs.html` | `src/builtin.rs`, `src/shell.rs` | partial | Job table printing exists, but POSIX output detail and state fidelity remain incomplete. |
| `docs/posix/utilities/pwd.html` | `src/builtin.rs` | partial | Builtin now parses `-L`/`-P` and prefers a valid logical `PWD` for the default logical mode. `cd` still does not preserve symlink-logical `PWD` state with full POSIX fidelity. |
| `docs/posix/utilities/read.html` | `src/builtin.rs`, `src/shell.rs` | partial | Intrinsic builtin now reads from standard input into current-shell variables, supports `-r` and `-d`, distinguishes EOF from error, and applies `IFS`-driven assignment splitting. Multi-byte and some interactive prompt edge semantics still need tightening against the utility page. |
| `docs/posix/utilities/readonly.html` | `src/builtin.rs`, `src/shell.rs` | partial | Marking variables readonly exists and `-p` now emits shell-reinput-safe quoting for readonly names, including unset readonly names. Remaining work is finer special-builtin error handling review. |
| `docs/posix/utilities/return.html` | `src/builtin.rs`, `src/exec.rs`, `src/shell.rs` | implemented with remaining edge review | Current-shell function return semantics exist. |
| `docs/posix/utilities/set.html` | `src/builtin.rs`, `src/shell.rs`, `src/expand.rs`, `src/exec.rs` | partial | Positional-parameter handling and `-C`/`+C`, `-f`/`+f` exist. Most option surface is still missing. |
| `docs/posix/utilities/shift.html` | `src/builtin.rs`, `src/shell.rs` | implemented with remaining edge review | Implemented. |
| `docs/posix/utilities/times.html` | `src/builtin.rs`, `src/sys.rs` | partial | Builtin now reports shell and child process times via handwritten `times()` and `sysconf(_SC_CLK_TCK)` bindings. Formatting and basic error paths are covered; broader locale/detail review is still open. |
| `docs/posix/utilities/trap.html` | `src/builtin.rs` | placeholder | Stub only; no real trap registration, output formatting, or eval semantics yet. |
| `docs/posix/utilities/umask.html` | `src/builtin.rs`, `src/sys.rs` | partial | Builtin now reads and updates the current shell umask, supports octal masks, `-S` symbolic output, and a useful subset of symbolic mask operands. Full chmod-style symbolic surface still needs review. |
| `docs/posix/utilities/unalias.html` | `src/builtin.rs` | partial | Builtin exists; option surface is not implemented. |
| `docs/posix/utilities/unset.html` | `src/builtin.rs`, `src/shell.rs` | partial | Variable unsetting now supports `-v`, function removal supports `-f`, and readonly-variable failures return a non-zero status without treating missing names as errors. Special-parameter handling and unspecified no-option function interactions still need review. |
| `docs/posix/utilities/wait.html` | `src/builtin.rs`, `src/shell.rs`, `src/sys.rs` | partial | Waiting by internal job id and wait-all exist. Full POSIX `wait` semantics still need deeper `waitpid` integration. |

## Interactive Behavior, Job Control, And Signals

| POSIX reference | Requirement area | Implementation | Validation | Current status |
| --- | --- | --- | --- | --- |
| `docs/posix/issue8/sh-utility.html#tag_20_110_13_01` | Command history list | `src/interactive.rs` | `src/interactive.rs` | Only plain line append to `HISTFILE` or default history path exists. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_13_02` | Command line editing | none | none | Not implemented. |
| `docs/posix/issue8/sh-utility.html#tag_20_110_13_03` | vi-mode editing | none | none | Not implemented. |
| `docs/posix/issue8/shell-command-language.html#tag_19_11` | Job control | `src/shell.rs`, `src/builtin.rs`, `src/sys.rs` | `src/shell.rs`, `src/builtin.rs`, `tests/spec/basic.rs` | Background jobs, `jobs`, `fg`, `bg`, and `wait` exist in a partial form. Full process-group and tty handoff semantics are still open. |
| `docs/posix/issue8/shell-command-language.html#tag_19_12` | Signals and error handling | `src/sys.rs`, `src/builtin.rs`, `src/shell.rs` | `src/sys.rs` | Signal-related low-level bindings exist, but real trap handling and full signal policy are not implemented. |

## Low-Level System Interface Coverage

| POSIX function page | Implementation | Current status |
| --- | --- | --- |
| `docs/posix/functions/close.html` | `src/sys.rs`, `src/exec.rs` | Used for descriptor cleanup and shell redirection guards. |
| `docs/posix/functions/dup.html` and `docs/posix/functions/dup2.html` | `src/sys.rs`, `src/exec.rs` | Used for current-shell and child-process redirection setup. |
| `docs/posix/functions/exec.html` | `src/sys.rs`, `src/builtin.rs`, `src/exec.rs` | Used for process replacement and external command execution paths. |
| `docs/posix/functions/fork.html` | indirect via process spawning model and job handling | Rust process creation is used today; deeper direct process-group control is still pending. |
| `docs/posix/functions/isatty.html` | `src/sys.rs` | Binding exists. |
| `docs/posix/functions/kill.html` | `src/sys.rs`, `src/shell.rs` | Binding exists and is used for job continuation paths. |
| `docs/posix/functions/open.html` | `src/sys.rs`, `src/exec.rs` | Used for redirection targets. |
| `docs/posix/functions/pipe.html` | `src/sys.rs`, `src/exec.rs` | Used for pipelines and here-doc transport. |
| `docs/posix/functions/setpgid.html` | `src/sys.rs` | Binding exists; broader job-control integration is still pending. |
| `docs/posix/functions/sigaction.html` | `src/sys.rs` | Binding presence is tracked, but real trap/signal disposition management is still pending. |
| `docs/posix/functions/tcgetpgrp.html` and `docs/posix/functions/tcsetpgrp.html` | `src/sys.rs` | Bindings exist; tty foreground handoff is not yet wired through shell job control. |
| `docs/posix/functions/times.html` | `src/sys.rs`, `src/builtin.rs` | Handwritten bindings now expose process and child CPU accounting for the `times` builtin. |
| `docs/posix/functions/umask.html` | `src/sys.rs`, `src/builtin.rs` | Handwritten binding now exposes current-shell umask reads and updates for the `umask` builtin. |
| `docs/posix/functions/waitpid.html` | `src/sys.rs`, `src/shell.rs`, `src/builtin.rs` | Used for job reaping and waiting; full POSIX `wait` semantics remain open. |

## Validation Lanes

- Unit tests in `src/syntax.rs`, `src/expand.rs`, `src/exec.rs`, `src/builtin.rs`, `src/interactive.rs`, `src/shell.rs`, and `src/sys.rs`
- Spec-oriented integration tests in `tests/spec/basic.rs`
- Differential compatibility checks in `tests/differential/portable.rs`
- Production-only line coverage in `scripts/coverage.sh`

## Highest-Priority Remaining Gaps

- Reserved-word recognition is still not exhaustively covered across all grammar positions in `2.4`, `2.9`, and `2.10`, but the `for` and `case` linebreak-sensitive grammar cases are now covered directly from the Issue 8 grammar.
- Parameter expansion still lacks the pattern-trimming operators from `2.6.2`.
- Field splitting still needs more exact coverage for mixed quoting and IFS edge cases in `2.6.5`.
- `trap` remains a placeholder relative to `docs/posix/utilities/trap.html` and `2.12`.
- `set` only implements a small subset of the `sh` and `set` option surface.
- `read` still needs tighter multi-byte, continuation-prompt, and corner-case review relative to `docs/posix/utilities/read.html`.
- `umask` still lacks the full chmod-style symbolic operand surface from `docs/posix/utilities/umask.html`.
- Job control remains partial relative to `2.11`, `fg`, `bg`, `jobs`, `wait`, `setpgid()`, `tcgetpgrp()`, and `tcsetpgrp()`.
