# Requirements Conventions

## Requirement IDs

`meiksh` uses stable requirement IDs in the form:

```text
REQ-<domain>-<topic>-<nnn>
```

Examples:
- `REQ-SH-STARTUP-001`
- `REQ-EXPAND-FIELDS-003`
- `REQ-BUILTIN-TRAP-007`
- `REQ-JOBS-CONTROL-004`

## Domains

- `SH`: shell utility entry, startup, environment, and interactive contract
- `PARSE`: tokenization, grammar, and parser-sensitive rules
- `EXPAND`: word-expansion and pattern semantics
- `REDIR`: redirection semantics
- `EXEC`: execution model and shell environment
- `BUILTIN`: builtin-specific requirements
- `JOBS`: job control, signals, and interactive process behavior
- `SYS`: low-level POSIX interface coverage
- `DOCS`: standards-mirror and validation-process requirements

## Status Vocabulary

Normative status values:
- `implemented`
- `partial`
- `unimplemented`
- `implementation-defined`

Test status values:
- `covered`
- `partial`
- `missing`

## Evidence Rules

- Every tracked item must cite at least one local POSIX page path.
- Use code paths under `src/` for implementation evidence.
- Use unit tests, spec tests, differential tests, or scripts as validation evidence.
- Coverage numbers are quality evidence only; they are not conformance evidence by themselves.

## Granularity Rules

- Split rows when a single status cell hides independent behaviors, options, or error modes.
- Prefer one row per observable requirement cluster rather than one row per source file or whole utility page.
- When POSIX leaves behavior implementation-defined or unspecified, record that in `docs/implementation-policy.md` and link the affected REQ IDs from the notes column.
