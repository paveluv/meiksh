# Meiksh POSIX Traceability Matrix

This document is the top-level conformance ledger for `meiksh`. The local pages under `docs/posix/` are the only requirements source of truth for entries in this matrix.

Supporting docs:
- `docs/requirements/conventions.md`
- `docs/requirements/standards-inventory.md`
- `docs/requirements/gap-register.md`
- `docs/implementation-policy.md`

## Standards Baseline

- Primary semantic target: POSIX Issue 8 / IEEE Std 1003.1-2024
- Compatibility watchlist: Issue 7 shell behavior that may still matter for older validation suites
- Mirror contract: `docs/posix-manifest.txt`
- Mirror fetch: `docs/fetch-posix-docs.sh`
- Mirror validation: `scripts/check-posix-docs.sh`

## Mirror And Governance

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation / evidence | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `REQ-DOCS-MIRROR-001` | `docs/posix-manifest.txt` | Required local shell-conformance mirror is defined by an explicit manifest. | implemented | covered | `docs/posix-manifest.txt`, `docs/README.md` | The manifest now defines the expected local standards set instead of relying on ad-hoc fetch commands. |
| `REQ-DOCS-MIRROR-002` | `docs/posix-manifest.txt` | Local mirror can be fetched from a single manifest-driven workflow. | implemented | covered | `docs/fetch-posix-docs.sh` | Fetch script now reads the manifest, and the current local mirror matches the manifest-defined set. |
| `REQ-DOCS-MIRROR-003` | `docs/posix-manifest.txt` | Local mirror completeness can be checked mechanically. | implemented | covered | `scripts/check-posix-docs.sh` | This is the repository guardrail for mirror completeness claims; `GAP-DOCS-001` is closed. |
| `REQ-DOCS-LEDGER-001` | `docs/posix/issue8/sh-utility.html`, `docs/posix/issue8/shell-command-language.html` | Conformance tracking uses stable REQ IDs and separate normative/test status. | implemented | covered | `docs/spec-matrix.md`, `docs/requirements/conventions.md` | Milestone 0 converts the matrix from a prose-heavy summary into a requirement ledger. |

## Utility Entry And Startup

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-SH-STARTUP-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_03` | Utility description and command-processing model | implemented | covered | `src/main.rs`, `src/shell.rs` | `tests/spec/basic.rs` | Command strings and script sources run through shared shell state and parser/executor layers. |
| `REQ-SH-OPTIONS-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_04` | Implemented startup options: `-a`, `-c`, `-n`, `-i`, `-f`, `-C`, `-s`, lone `-`, invalid-option rejection, and named `-o`/`+o` handling for the implemented subset | partial | covered | `src/shell.rs` | `src/shell.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | `meiksh` now supports startup `-a` and named `-o`/`+o` forms for `allexport`, `noclobber`, `noglob`, and `noexec`, but the broader Issue 8 option surface is not closed. See `GAP-SH-001`. |
| `REQ-SH-OPERANDS-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_05` | Command-string, `command_name`/`$0`, script-path, and `-s` stdin-with-operands paths | implemented | covered | `src/shell.rs` | `src/shell.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | `-c` assigns special parameter 0 from `command_name`, command-file execution assigns `$0` from the operand, and non-interactive stdin now executes incrementally without violating the utility-page no-read-ahead rule. |
| `REQ-SH-OPERANDS-002` | `docs/posix/issue8/sh-utility.html#tag_20_110_05` | Command-file search without a slash | implemented | covered | `src/shell.rs` | `tests/spec/basic.rs` | `meiksh` now attempts the current working directory first and then searches `PATH` for an executable file. |
| `REQ-SH-ENV-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_08` | Interactive `ENV` parameter expansion and identity guard | implemented | covered | `src/interactive.rs`, `src/expand.rs`, `src/sys.rs` | `src/interactive.rs`, `src/expand.rs`, `src/sys.rs`, `tests/spec/basic.rs` | `ENV` is parameter-expanded and skipped when real/effective ids differ. |
| `REQ-SH-ENV-002` | `docs/posix/issue8/sh-utility.html#tag_20_110_08` | Interactive prompt and history-path environment handling | partial | covered | `src/interactive.rs` | `src/interactive.rs`, `tests/spec/basic.rs` | `PS1` plus `HISTFILE`/`$HOME/.sh_history` are wired in a simplified interactive layer. |
| `REQ-SH-INTERACTIVE-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_13` | Interactive prompt loop continues after command errors with diagnostics | implemented | covered | `src/interactive.rs` | `src/interactive.rs`, `tests/spec/basic.rs` | Interactive error behavior was tightened in Milestone 6. |
| `REQ-SH-INTERACTIVE-002` | `docs/posix/issue8/sh-utility.html#tag_20_110_13_01` | Command history list semantics | partial | partial | `src/interactive.rs` | `src/interactive.rs` | Current behavior is plain append-only history. See `GAP-SH-003`. |
| `REQ-SH-INTERACTIVE-003` | `docs/posix/issue8/sh-utility.html#tag_20_110_13_02`, `docs/posix/issue8/sh-utility.html#tag_20_110_13_03` | Command-line and vi-mode editing | unimplemented | missing | none | none | Still unimplemented. See `GAP-SH-003`. |
| `REQ-SH-EXIT-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_14` | `sh` utility exit-status behavior | partial | covered | `src/main.rs`, `src/shell.rs` | `tests/spec/basic.rs` | Startup paths now distinguish not-found (`127`) and unrecoverable read-error (`128`) command-file failures, but broader utility-page exit-status and error-consequence coverage remains open. See `GAP-SH-002`. |
| `REQ-SH-ERRORS-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_15` | Consequences of errors for interactive and non-interactive shells | partial | partial | `src/shell.rs`, `src/builtin.rs`, `src/interactive.rs` | `src/builtin.rs`, `src/interactive.rs`, `tests/spec/basic.rs` | Some finer distinctions remain open. See `GAP-EXEC-001`. |

## Tokenization, Grammar, And Parsing

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-PARSE-QUOTE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_02` | Raw quoting survives tokenization for later expansion stages | implemented | covered | `src/syntax.rs` | `src/syntax.rs`, `src/expand.rs` | Current parser preserves quote-sensitive information for expansion. |
| `REQ-PARSE-TOKENS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_03` | Operators, words, comments, separators, and here-doc token collection | implemented | covered | `src/syntax.rs` | `src/syntax.rs` | Core token recognition is in place. |
| `REQ-PARSE-ALIAS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_03_01` | Parser-time alias substitution and chained reparsing | implemented | covered | `src/syntax.rs`, `src/shell.rs`, `src/exec.rs` | `src/syntax.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Top-level and nested reparsing cases are covered. |
| `REQ-PARSE-RESERVED-001` | `docs/posix/issue8/shell-command-language.html#tag_19_04` | Reserved words in grammar-sensitive positions | implemented | covered | `src/syntax.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Includes `for`, `case`, and brace-group-sensitive parsing. |
| `REQ-PARSE-PIPE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_02` | Pipeline parsing and leading `!` behavior | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Grammar linebreak after `|` is now honored. |
| `REQ-PARSE-LISTS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_03` | Sequential, AND, OR, and async list parsing | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Grammar linebreak after `&&` and `||` is covered. |
| `REQ-PARSE-COMPOUND-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_04` | Compound commands: groups, subshells, `for`, `case`, `if`, `while`, `until` | partial | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Syntax is largely covered; execution-fidelity gaps remain for subshell-related behavior. |
| `REQ-PARSE-FUNCTION-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_05` | Function definition command | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Function names now reject exact reserved words. |
| `REQ-PARSE-GRAMMAR-001` | `docs/posix/issue8/shell-command-language.html#tag_19_10_01`, `docs/posix/issue8/shell-command-language.html#tag_19_10_02` | Grammar lexical conventions and main productions | implemented | covered | `src/syntax.rs` | `src/syntax.rs`, `tests/spec/basic.rs` | Current grammar handling is strong for the implemented subset. |

## Expansion And Pattern Semantics

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-EXPAND-PARAMS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_05` | Shell parameters and variables are exposed to expansion | implemented | covered | `src/shell.rs`, `src/expand.rs` | `src/shell.rs`, `src/expand.rs`, `tests/spec/basic.rs` | Positional and special parameters exist. |
| `REQ-EXPAND-TILDE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_01` | Tilde expansion for supported forms | partial | covered | `src/expand.rs` | `src/expand.rs` | Supported leading forms exist, but full Issue 8 alignment remains open. See `GAP-EXPAND-002`. |
| `REQ-EXPAND-PARAMS-002` | `docs/posix/issue8/shell-command-language.html#tag_19_06_02` | Parameter expansion operators, length, and pattern removal | implemented | covered | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs` | Includes `%`, `%%`, `#`, `##`, default/assign/error/alternate operators, and length. |
| `REQ-EXPAND-CMDSUB-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_03` | Command substitution | partial | covered | `src/expand.rs`, `src/shell.rs` | `src/expand.rs`, `tests/spec/basic.rs` | Current recursive `meiksh -c` shortcut is a fidelity risk. See `GAP-EXEC-002`. |
| `REQ-EXPAND-ARITH-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_04` | Arithmetic expansion | partial | covered | `src/expand.rs` | `src/expand.rs` | Limited to integer literals and `+`, `-`, `*`, `/`, `%`. See `GAP-EXPAND-002`. |
| `REQ-EXPAND-FIELDS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_05` | IFS whitespace and non-whitespace field splitting core | partial | covered | `src/expand.rs`, `src/shell.rs` | `src/expand.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | Core behavior exists, and `$*` now joins using the first character of `IFS`; mixed-quoting and `"$@"`-adjacent corners remain open. See `GAP-EXPAND-001`. |
| `REQ-EXPAND-GLOB-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_06` | Pathname expansion and `set -f` interaction | implemented | covered | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | Dotfile suppression and `set -f` are covered. |
| `REQ-EXPAND-QUOTE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_06_07` | Quote removal | implemented | covered | `src/expand.rs` | `src/expand.rs` | Implemented as part of the word-expansion pipeline. |
| `REQ-EXPAND-DSQ-001` | `docs/posix/issue8/shell-command-language.html#tag_19_02_04` | Issue 8 dollar-single-quotes | implemented | covered | `src/expand.rs` | `src/expand.rs`, `tests/spec/basic.rs` | `$'...'` escape processing is implemented in unquoted words and remains literal inside double quotes. |
| `REQ-EXPAND-PATTERN-001` | `docs/posix/issue8/shell-command-language.html#tag_19_14` | Pattern matching notation for globbing and `case` | partial | covered | `src/expand.rs`, `src/exec.rs` | `src/expand.rs`, `src/exec.rs` | Wildcard and bracket-class behavior are covered for implemented paths. |

## Redirection

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-REDIR-GENERAL-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07` | General redirection model for simple, builtin, function, and compound execution | implemented | covered | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Redirections are parsed and applied across the main execution contexts. |
| `REQ-REDIR-IN-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_01` | Redirecting input | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Includes numeric fd prefixes. |
| `REQ-REDIR-OUT-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_02` | Redirecting output and noclobber interaction | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | `set -C` is respected for plain `>`. |
| `REQ-REDIR-APPEND-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_03` | Appending output | implemented | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Implemented. |
| `REQ-REDIR-HEREDOC-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_04` | Here-doc parsing, tab stripping, and delimiter-sensitive expansion | implemented | covered | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs` | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs`, `tests/spec/basic.rs` | `<<` and `<<-` are covered. |
| `REQ-REDIR-DUPIN-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_05` | Duplicating input fd | implemented | covered | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |
| `REQ-REDIR-DUPOUT-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_06` | Duplicating output fd | implemented | covered | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |
| `REQ-REDIR-RW-001` | `docs/posix/issue8/shell-command-language.html#tag_19_07_07` | Read/write open fd | implemented | covered | `src/syntax.rs`, `src/exec.rs`, `src/sys.rs` | `src/syntax.rs`, `src/exec.rs` | Implemented. |

## Command Execution Model

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-EXEC-ERRORS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_08_01` | Consequences of shell errors | partial | partial | `src/shell.rs`, `src/builtin.rs`, `src/exec.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Explicit failure paths dominate today. See `GAP-EXEC-001`. |
| `REQ-EXEC-STATUS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_08_02` | Exit status propagation for commands | implemented | covered | `src/exec.rs`, `src/shell.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Simple commands, pipelines, lists, and compound commands propagate status. |
| `REQ-EXEC-SIMPLE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_01` | Simple command assignment handling, command name resolution, redirection ordering | partial | covered | `src/syntax.rs`, `src/expand.rs`, `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Broadly implemented for the current feature set, but command-search and failure-path fidelity are not fully closed. See `GAP-EXEC-003`. |
| `REQ-EXEC-SIMPLE-002` | `docs/posix/issue8/shell-command-language.html#tag_19_09_01_03` | Commands with no command name | implemented | covered | `src/exec.rs`, `src/shell.rs` | `src/exec.rs` | Assignment-only commands mutate current shell state. |
| `REQ-EXEC-SEARCH-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_01_04` | Command search and execution | partial | covered | `src/exec.rs`, `src/builtin.rs`, `src/shell.rs` | `src/exec.rs`, `src/builtin.rs`, `tests/spec/basic.rs` | Builtins, shell functions, and external lookup exist, but stricter executable-file semantics and broader failure parity remain open. See `GAP-EXEC-003`. |
| `REQ-EXEC-UTILITY-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_01_06` | External utility execution and `ENOEXEC` fallback | implemented | covered | `src/exec.rs`, `src/sys.rs` | `src/exec.rs`, `tests/spec/basic.rs` | Text executables fall back through `sh`. |
| `REQ-EXEC-PIPE-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_02_01` | Pipeline exit status and `!` negation | implemented | covered | `src/exec.rs` | `src/exec.rs`, `tests/spec/basic.rs` | `pipefail` is intentionally not tracked as a POSIX requirement. |
| `REQ-EXEC-ASYNC-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_03_02` | Asynchronous AND-OR lists and retained background status | partial | covered | `src/exec.rs`, `src/shell.rs` | `src/exec.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Job-control-disabled and signal-inheritance corners remain open. |
| `REQ-EXEC-GROUP-001` | `docs/posix/issue8/shell-command-language.html#tag_19_09_04_01` | Grouping commands and subshell execution | partial | covered | `src/syntax.rs`, `src/exec.rs` | `src/syntax.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Syntax works; subshell fidelity remains incomplete. |
| `REQ-EXEC-ENV-001` | `docs/posix/issue8/shell-command-language.html#tag_19_13` | Shell execution environment | partial | covered | `src/shell.rs`, `src/exec.rs`, `src/builtin.rs` | `src/shell.rs`, `src/exec.rs`, `tests/spec/basic.rs` | Current-shell state mutation is modeled; subshell trap reset remains only partially aligned. |

## Builtins And Related Utilities

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-BUILTIN-ALIAS-001` | `docs/posix/utilities/alias.html` | Alias builtin and parser integration | implemented | covered | `src/builtin.rs`, `src/syntax.rs`, `src/shell.rs`, `src/exec.rs` | `src/builtin.rs`, `src/syntax.rs`, `tests/spec/basic.rs` | Builtin plus parser-time substitution are in place. |
| `REQ-BUILTIN-CD-001` | `docs/posix/utilities/cd.html` | `cd` operand handling, `PWD`/`OLDPWD`, `cd -`, and `CDPATH` search | partial | covered | `src/builtin.rs` | `src/builtin.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | `CDPATH` lookup and required path reporting for non-empty entries now exist. `-L`/`-P`/`-e` and full logical-path fidelity remain open. See `GAP-BUILTIN-001`. |
| `REQ-BUILTIN-COMMAND-001` | `docs/posix/utilities/command.html` | `command` utility lookup and execution modes | partial | covered | `src/builtin.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | `-p`, `-v`, and `-V` exist; finer parity remains open, and broader utility-surface closure is tracked in `GAP-BUILTIN-006`. |
| `REQ-BUILTIN-DOT-001` | `docs/posix/utilities/dot.html` | Sourcing by pathname and slashless `PATH` search | partial | covered | `src/builtin.rs`, `src/shell.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Some operand and error-consequence details still need review. |
| `REQ-BUILTIN-EXPORT-001` | `docs/posix/utilities/export.html` | Export assignment and `-p` reporting | partial | covered | `src/builtin.rs`, `src/shell.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | No-operand and finer diagnostics remain open. |
| `REQ-BUILTIN-JOBS-001` | `docs/posix/utilities/bg.html`, `docs/posix/utilities/fg.html`, `docs/posix/utilities/jobs.html`, `docs/posix/utilities/wait.html` | Job-related builtin selection and status reporting | partial | covered | `src/builtin.rs`, `src/shell.rs`, `src/sys.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Full job-id grammar, suspended-state fidelity, and retention-policy details remain open. See `GAP-JOBS-001`. |
| `REQ-BUILTIN-PWD-001` | `docs/posix/utilities/pwd.html` | Logical and physical `pwd` behavior | partial | covered | `src/builtin.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Depends on fuller `cd` logical-path fidelity. |
| `REQ-BUILTIN-READ-001` | `docs/posix/utilities/read.html` | Current-shell input assignment, `IFS`, `-r`, `-d` | partial | covered | `src/builtin.rs`, `src/shell.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Multibyte and prompt semantics remain open. See `GAP-BUILTIN-003`. |
| `REQ-BUILTIN-READONLY-001` | `docs/posix/utilities/readonly.html` | Readonly marking and `-p` reporting | partial | covered | `src/builtin.rs`, `src/shell.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Finer special-builtin error handling remains open. |
| `REQ-BUILTIN-SET-001` | `docs/posix/utilities/set.html` | Positional parameters and implemented option subset | partial | covered | `src/builtin.rs`, `src/shell.rs`, `src/expand.rs`, `src/exec.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | `set` now covers `-a`, `-C`, `-f`, `-n`, named `-o`/`+o` forms for the implemented subset, and `set -o` / `set +o` reporting, but most of the Issue 8 option surface still remains. See `GAP-BUILTIN-002`. |
| `REQ-BUILTIN-TRAP-001` | `docs/posix/utilities/trap.html` | Trap listing, default/ignore/command forms, EXIT and selected signals | partial | covered | `src/builtin.rs`, `src/shell.rs`, `src/sys.rs` | `src/builtin.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Ignored-on-entry, broader signal coverage, and subshell exceptions remain open. See `GAP-BUILTIN-004`. |
| `REQ-BUILTIN-TIMES-001` | `docs/posix/utilities/times.html` | Shell and child CPU accounting | partial | covered | `src/builtin.rs`, `src/sys.rs` | `src/builtin.rs` | Locale/detail review remains open. |
| `REQ-BUILTIN-UMASK-001` | `docs/posix/utilities/umask.html` | Octal and partial symbolic `umask` behavior | partial | covered | `src/builtin.rs`, `src/sys.rs` | `src/builtin.rs` | Full chmod-style symbolic surface remains open. See `GAP-BUILTIN-005`. |
| `REQ-BUILTIN-UNALIAS-001` | `docs/posix/utilities/unalias.html` | `unalias -a` and missing-name status | partial | covered | `src/builtin.rs` | `src/builtin.rs`, `tests/spec/basic.rs`, `tests/differential/portable.rs` | Broader diagnostics still need review. |
| `REQ-BUILTIN-UNSET-001` | `docs/posix/utilities/unset.html` | Variable and function removal paths | partial | covered | `src/builtin.rs`, `src/shell.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Special-parameter handling and some unspecified interactions remain open. |
| `REQ-BUILTIN-CONTROL-001` | `docs/posix/utilities/break.html`, `docs/posix/utilities/continue.html`, `docs/posix/utilities/return.html`, `docs/posix/utilities/exit.html`, `docs/posix/utilities/eval.html`, `docs/posix/utilities/exec.html`, `docs/posix/utilities/shift.html` | Control-flow builtins and shell-special execution paths | partial | covered | `src/builtin.rs`, `src/exec.rs`, `src/shell.rs`, `src/sys.rs` | `src/builtin.rs`, `tests/spec/basic.rs` | Core behaviors exist; remaining edges are narrower than the larger open gaps above, while still-unimplemented mirrored utilities are tracked in `GAP-BUILTIN-006`. |

## Interactive Behavior, Job Control, And Signals

| REQ ID | POSIX reference | Requirement cluster | Normative status | Test status | Implementation | Validation | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `REQ-JOBS-HISTORY-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_13_01` | Command history list | partial | partial | `src/interactive.rs` | `src/interactive.rs` | Current implementation only appends plain input lines. |
| `REQ-JOBS-EDITING-001` | `docs/posix/issue8/sh-utility.html#tag_20_110_13_02`, `docs/posix/issue8/sh-utility.html#tag_20_110_13_03` | Command-line editing and vi-mode editing | unimplemented | missing | none | none | Still unimplemented. See `GAP-SH-003`. |
| `REQ-JOBS-CONTROL-001` | `docs/posix/issue8/shell-command-language.html#tag_19_11` | Process groups, `fg`/`bg`, and tty foreground handoff | partial | covered | `src/shell.rs`, `src/builtin.rs`, `src/sys.rs`, `src/exec.rs` | `src/shell.rs`, `src/builtin.rs`, `src/exec.rs`, `tests/spec/basic.rs` | `set -m`, stopped-job accounting, full job-id semantics, and tty mode restore remain open. See `GAP-JOBS-001` and `GAP-JOBS-003`. |
| `REQ-JOBS-SIGNALS-001` | `docs/posix/issue8/shell-command-language.html#tag_19_12` | Signal disposition, pending trap delivery, and interrupt status behavior | partial | covered | `src/sys.rs`, `src/builtin.rs`, `src/shell.rs` | `src/sys.rs`, `src/builtin.rs`, `src/shell.rs`, `tests/spec/basic.rs` | Ignored-on-entry rules and broader async inheritance behavior remain open. See `GAP-JOBS-002`. |

## Low-Level System Interface Coverage

| REQ ID | POSIX function page | Requirement cluster | Normative status | Test status | Implementation | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `REQ-SYS-FDS-001` | `docs/posix/functions/close.html`, `docs/posix/functions/dup.html`, `docs/posix/functions/dup2.html`, `docs/posix/functions/open.html` | Descriptor and redirection primitives | implemented | covered | `src/sys.rs`, `src/exec.rs` | Used for descriptor cleanup and redirection setup. |
| `REQ-SYS-EXEC-001` | `docs/posix/functions/exec.html`, `docs/posix/functions/execl.html`, `docs/posix/functions/execve.html` | Process replacement and external execution references | partial | covered | `src/sys.rs`, `src/builtin.rs`, `src/exec.rs` | Current implementation uses the subset required by the present execution path. |
| `REQ-SYS-PROCESS-001` | `docs/posix/functions/fork.html`, `docs/posix/functions/wait.html`, `docs/posix/functions/waitid.html`, `docs/posix/functions/waitpid.html` | Process creation and waiting references | partial | covered | `src/sys.rs`, `src/shell.rs`, `src/builtin.rs`, `src/exec.rs` | Waiting behavior is more advanced than direct process creation today. |
| `REQ-SYS-SIGNALS-001` | `docs/posix/functions/signal.html`, `docs/posix/functions/sigaction.html`, `docs/posix/functions/kill.html` | Signal disposition and delivery references | partial | covered | `src/sys.rs`, `src/shell.rs`, `src/builtin.rs` | Broader signal coverage remains a conformance gap. |
| `REQ-SYS-JOBCONTROL-001` | `docs/posix/functions/isatty.html`, `docs/posix/functions/setpgid.html`, `docs/posix/functions/tcgetpgrp.html`, `docs/posix/functions/tcsetpgrp.html`, `docs/posix/functions/tcgetattr.html`, `docs/posix/functions/tcsetattr.html` | Job-control and terminal-control references | partial | partial | `src/sys.rs`, `src/exec.rs`, `src/shell.rs` | Tty foreground handoff exists; tty mode save/restore is still open. See `GAP-JOBS-003`. |
| `REQ-SYS-PATHS-001` | `docs/posix/functions/getcwd.html`, `docs/posix/functions/getpwnam.html`, `docs/posix/functions/pathconf.html`, `docs/posix/functions/stat.html`, `docs/posix/functions/lstat.html`, `docs/posix/functions/fstat.html`, `docs/posix/functions/unlink.html` | Pathname, home lookup, and file-state references | partial | covered | `src/sys.rs`, `src/builtin.rs`, `src/expand.rs` | Current implementation uses a subset of the mirrored path-related interfaces. |
| `REQ-SYS-MATCH-001` | `docs/posix/functions/fnmatch.html`, `docs/posix/functions/glob.html`, `docs/posix/functions/wordexp.html` | Pattern and word-expansion reference set | implementation-defined | missing | `src/expand.rs` | `meiksh` currently implements expansion itself instead of calling these interfaces directly. |
| `REQ-SYS-ACCOUNTING-001` | `docs/posix/functions/times.html`, `docs/posix/functions/umask.html`, `docs/posix/functions/sysconf.html`, `docs/posix/functions/getrlimit.html`, `docs/posix/functions/setrlimit.html` | Accounting, limits, and shell-environment support | partial | covered | `src/sys.rs`, `src/builtin.rs` | `times` and `umask` are wired; `ulimit`/limit handling remains open. |

## Validation Lanes

- Unit tests in `src/syntax.rs`, `src/expand.rs`, `src/exec.rs`, `src/builtin.rs`, `src/interactive.rs`, `src/shell.rs`, and `src/sys.rs`
- Spec-oriented integration tests in `tests/spec/basic.rs`
- Differential compatibility checks in `tests/differential/portable.rs`
- Production-only line coverage in `scripts/coverage.sh`
- Local standards mirror validation in `scripts/check-posix-docs.sh`

## Gap Register

The milestone-oriented open backlog now lives in `docs/requirements/gap-register.md`.
