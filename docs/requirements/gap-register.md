# Gap Register

This register turns broad partial-conformance areas into named backlog items that can be moved milestone by milestone.

## High-Priority Open Gaps

| Gap ID | Linked requirement area | Current gap |
| --- | --- | --- |
| `GAP-SH-001` | `REQ-SH-OPTIONS-*` | `sh` and `set` still implement only part of the full Issue 8 option surface. |
| `GAP-SH-002` | `REQ-SH-OPERANDS-*` | Remaining `sh` operand work is now around broader utility-page polish after command-file `$0` and slashless lookup were tightened. |
| `GAP-SH-003` | `REQ-SH-INTERACTIVE-*` | Command history list semantics and command-line editing modes are unimplemented. |
| `GAP-EXPAND-001` | `REQ-EXPAND-FIELDS-*` | Field splitting still needs exact mixed-quoting, `"$@"`, and remaining IFS edge coverage. |
| `GAP-EXEC-001` | `REQ-EXEC-ERRORS-*` | Some POSIX shell-error consequence distinctions remain open. |
| `GAP-EXEC-002` | `REQ-EXEC-SUBSHELL-*` | Subshell and command-substitution fidelity are still affected by the current recursive `meiksh -c` shortcut. |
| `GAP-BUILTIN-001` | `REQ-BUILTIN-CD-*` | `cd` still lacks `-L`/`-P`/`-e` and full logical-path fidelity after the new `CDPATH` coverage. |
| `GAP-BUILTIN-002` | `REQ-BUILTIN-SET-*` | Most `set` builtin options and reporting semantics are still missing. |
| `GAP-BUILTIN-003` | `REQ-BUILTIN-READ-*` | `read` still needs tighter multibyte, prompt, and corner-case conformance. |
| `GAP-BUILTIN-004` | `REQ-BUILTIN-TRAP-*` | Trap coverage still lacks ignored-on-entry semantics, broader signal names, and subshell/command-substitution exceptions. |
| `GAP-BUILTIN-005` | `REQ-BUILTIN-UMASK-*` | Full chmod-style symbolic `umask` operands remain incomplete. |
| `GAP-JOBS-001` | `REQ-JOBS-CONTROL-*` | Job control still lacks `set -m`, stopped-job tracking, tty mode restore, and complete job-id grammar. |
| `GAP-JOBS-002` | `REQ-JOBS-SIGNALS-*` | Async-list signal inheritance and some signal-interruption rules remain open. |
| `GAP-DOCS-001` | `REQ-DOCS-MIRROR-*` | The standards mirror contract now exists, but the local mirror must still be populated to match it. |

## Milestone Mapping

- Milestone 0: `GAP-DOCS-001`
- Milestone 1: `GAP-SH-001`, `GAP-SH-002`, `GAP-SH-003`
- Milestone 2: `GAP-EXPAND-001`
- Milestone 3: `GAP-BUILTIN-001`, `GAP-BUILTIN-002`, `GAP-BUILTIN-003`, `GAP-BUILTIN-004`, `GAP-BUILTIN-005`
- Milestone 4: `GAP-EXEC-001`, `GAP-EXEC-002`
- Milestone 5: `GAP-JOBS-001`, `GAP-JOBS-002`
