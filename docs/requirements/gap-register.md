# Gap Register

This register turns broad partial-conformance areas into named backlog items that can be moved milestone by milestone.

## Closed Structural Gaps

| Gap ID | Linked requirement area | Resolution |
| --- | --- | --- |
| `GAP-DOCS-001` | `REQ-DOCS-MIRROR-*` | Closed. The local standards mirror is now complete relative to `docs/posix-manifest.txt`, and `scripts/check-posix-docs.sh` validates it mechanically. |

## High-Priority Open Gaps

| Gap ID | Linked requirement area | Current gap |
| --- | --- | --- |
| `GAP-SH-001` | `REQ-SH-OPTIONS-*` | `sh` and `set` now cover `-a`, `-C`, `-f`, `-n`, `-u`, `-v`, and named `-o`/`+o` forms for the implemented subset, but the rest of the full Issue 8 option surface still remains open. |
| `GAP-SH-002` | `REQ-SH-STARTUP-*`, `REQ-SH-OPERANDS-*`, `REQ-SH-EXIT-*` | The non-interactive stdin no-read-ahead rule, blocking-stdin correction for inherited non-blocking FIFO/terminal descriptors, and the direct-ordinary-builtin versus direct-special-builtin error split are now implemented; the remaining startup gap is narrower utility-page polish around the rest of the option surface and top-level exit-status/error classification. |
| `GAP-SH-003` | `REQ-SH-INTERACTIVE-*` | Command history list semantics are still simplified, and command-line / vi-mode editing remain unimplemented. |
| `GAP-EXPAND-001` | `REQ-EXPAND-FIELDS-*` | Field splitting still needs exact mixed-quoting, `"$@"`, and remaining IFS edge coverage. |
| `GAP-EXPAND-002` | `REQ-EXPAND-TILDE-*`, `REQ-EXPAND-QUOTE-*`, `REQ-EXPAND-ARITH-*` | Tilde expansion, double-quote backslash rules, and arithmetic expansion are still only partially aligned with the mirrored Issue 8 text. |
| `GAP-EXEC-001` | `REQ-EXEC-ERRORS-*` | Some POSIX shell-error consequence distinctions remain open, especially syntax / expansion / assignment / function-execution cases beyond the now-fixed ordinary-builtin versus special-builtin split. |
| `GAP-EXEC-002` | `REQ-EXEC-ENV-*`, `REQ-EXPAND-CMDSUB-*`, `REQ-EXEC-GROUP-*` | Subshell and command substitution now use fork with inherited shell state. Remaining gaps are subshell trap reset and fine-grained environment isolation per POSIX shell execution environment rules. |
| `GAP-EXEC-003` | `REQ-EXEC-SEARCH-*` | Command search still needs stricter executable-file semantics and broader utility-page parity for external lookup and failure cases. |
| `GAP-BUILTIN-001` | `REQ-BUILTIN-CD-*` | `cd` still lacks `-L`/`-P`/`-e` and full logical-path fidelity after the new `CDPATH` coverage. |
| `GAP-BUILTIN-002` | `REQ-BUILTIN-SET-*` | `set` now has a stronger core subset, including `-a`, `-u`, `-v`, named `-o`/`+o`, option-state reporting, and plain `nounset` expansion failures, but most remaining Issue 8 options and their exact semantics are still missing. |
| `GAP-BUILTIN-003` | `REQ-BUILTIN-READ-*` | `read` still needs tighter multibyte, prompt, and corner-case conformance. |
| `GAP-BUILTIN-004` | `REQ-BUILTIN-TRAP-*` | Trap coverage still lacks ignored-on-entry semantics, broader signal names, and subshell/command-substitution exceptions. |
| `GAP-BUILTIN-005` | `REQ-BUILTIN-UMASK-*` | Full chmod-style symbolic `umask` operands remain incomplete. |
| `GAP-BUILTIN-006` | `REQ-BUILTIN-*`, `REQ-SYS-ACCOUNTING-*` | Mirrored utility pages such as `hash`, `getopts`, `ulimit`, and `fc` still need either implementation or an explicit conformance decision before a full POSIX claim is credible. |
| `GAP-JOBS-001` | `REQ-JOBS-CONTROL-*` | Job control still lacks `set -m`, stopped-job tracking, tty mode restore, and complete job-id grammar. |
| `GAP-JOBS-002` | `REQ-JOBS-SIGNALS-*` | Async-list signal inheritance and some signal-interruption rules remain open. |
| `GAP-JOBS-003` | `REQ-SYS-JOBCONTROL-*`, `REQ-JOBS-CONTROL-*` | Terminal mode save/restore through the mirrored termios interfaces is still missing, so foreground job handoff is only partially compliant. |

## Milestone Mapping

- Milestone 0: structural mirror and matrix work is complete; `GAP-DOCS-001` is closed.
- Milestone 1: `GAP-SH-001`, `GAP-SH-002`, `GAP-SH-003`
- Milestone 2: `GAP-EXPAND-001`, `GAP-EXPAND-002`
- Milestone 3: `GAP-BUILTIN-001`, `GAP-BUILTIN-002`, `GAP-BUILTIN-003`, `GAP-BUILTIN-004`, `GAP-BUILTIN-005`, `GAP-BUILTIN-006`
- Milestone 4: `GAP-EXEC-001`, `GAP-EXEC-002`, `GAP-EXEC-003`
- Milestone 5: `GAP-JOBS-001`, `GAP-JOBS-002`, `GAP-JOBS-003`
