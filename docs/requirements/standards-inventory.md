# Standards Inventory

This inventory describes the shell-conformance standards mirror expected under `docs/posix/`. The concrete file list lives in `docs/posix-manifest.txt`.

## Mirror Categories

| Category | Local paths | Why it is mirrored |
| --- | --- | --- |
| Base definitions | `docs/posix/basedefs/*.html` | Shell text cross-references definitions, environment rules, pathnames, locales, regular expressions, and terminal concepts from XBD. |
| Issue 8 shell baseline | `docs/posix/issue8/*.html` | Primary normative baseline for shell language and the `sh` utility. |
| Issue 7 shell watchlist | `docs/posix/issue7/*.html` | Compatibility review for older validation suites and historical behavior checks. |
| Utility pages | `docs/posix/utilities/*.html` | Special builtins, shell builtins, and utility pages the shell contract depends on. |
| System interfaces | `docs/posix/functions/*.html` | Runtime interfaces that materially affect redirection, execution, traps, job control, tty handling, and path processing. |
| Validation/publication index | `docs/posix/validation/*.html` | Local pointer to the publication tree used while auditing standards coverage. |

## Shell-Focused Utility Coverage

The manifest includes the shell language chapter, `sh`, and the major shell-related utility pages needed for conformance work, including:
- special builtins such as `:`, `.`, `break`, `continue`, `eval`, `exec`, `exit`, `export`, `readonly`, `return`, `set`, `shift`, `times`, `trap`, and `unset`
- common shell builtins and related utilities such as `alias`, `bg`, `cd`, `command`, `fc`, `fg`, `getopts`, `hash`, `jobs`, `pwd`, `read`, `ulimit`, `umask`, `unalias`, and `wait`
- utility pages often referenced while validating shell behavior such as `[`, `test`, `echo`, `printf`, `true`, and `false`

## System Interface Coverage

The manifest includes the currently used low-level interfaces plus additional shell-relevant pages that are likely to matter as conformance work closes remaining gaps, including:
- process creation and replacement
- descriptor and file-opening primitives
- signal disposition and delivery
- process groups and terminal foreground ownership
- timing, limits, pathname, and shell-environment helpers
- pathname and pattern matching helpers

## Audit Expectations

- `docs/fetch-posix-docs.sh` should be able to populate every manifest entry.
- `scripts/check-posix-docs.sh` should succeed before a standards-completeness claim is made.
- `docs/spec-matrix.md` should reference exact local mirror paths from this inventory rather than ad-hoc external URLs.
