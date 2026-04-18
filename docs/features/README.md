# Non-POSIX Features

This directory holds normative specifications for shell features that meiksh implements even though POSIX does not describe them.

## Why These Specs Exist

POSIX.1-2024 (Issue 8) is the authoritative source for the core shell language, utilities, and the `vi` line-editing mode. Meiksh conforms to that text strictly, backed by the mirror under [../posix/](../posix/) and the test matrix under [../../tests/matrix/](../../tests/matrix/).

POSIX is deliberately silent on a number of features that users nonetheless expect from a modern Unix shell. The GNU Bash manual, the GNU Readline manual, and the ksh93 and zsh reference pages are product documentation for their respective implementations; none of them is a standard. When a feature has been universally adopted across bash, ksh, and zsh, users treat it as part of "a shell" rather than part of "bash". Meiksh therefore provides such features, but writes its own spec for them so the behavior is not defined by accident.

## Authority

Every file in this directory is the authoritative definition of its feature for meiksh. The specs use RFC 2119 "shall / shall not / should / may" language with the same normative weight as the POSIX text we mirror. Conformance is enforced by the test matrix in [../../tests/matrix/](../../tests/matrix/) exactly like POSIX conformance is.

## What Triggers a New Spec Here

A feature is considered for inclusion in this directory when it meets all of:

- **POSIX is silent**: the feature is not mandated by POSIX.1-2024 `sh` or the Shell Command Language chapter.
- **De-facto adoption**: the feature is implemented, with substantially the same user-visible behavior, by at least two of bash, ksh, zsh.
- **User expectation**: users configuring or scripting for meiksh would reasonably expect the feature to be present (e.g., because their `~/.inputrc`, muscle memory from daily `C-r`, or tooling such as `fzf` depends on it).

A feature that only one shell implements is not a candidate. A feature that one shell abandoned (for example, 8-bit meta input) is not a candidate. Meiksh is not in the business of tracking every readline option or every zsh widget.

## Non-Goals Are Explicit

Each spec lists, in a normative non-goals section, every related feature that exists in the reference shells but is intentionally absent from meiksh. If bash has a command that this directory does not cover, the absence is a documented choice, not an oversight. Where practical, the spec also sketches what it would take to add the cut feature later, so the subset is recoverable without guesswork.

## Spec-First Workflow

New non-POSIX features start life as a markdown file in this directory, reviewed and merged before any implementation lands. The sequence is:

1. Propose a spec (draft `.md` file here).
2. Review and merge the spec.
3. Implement against the merged spec.
4. Add matrix tests under `tests/matrix/` that reference the spec.

This mirrors how POSIX work flows from [../posix/](../posix/) (read-only reference material) into code, just with this directory taking the role of "the standard" for features POSIX does not cover.

## Current Specs

Each spec declares its own implementation status in a `Status` section at the top of the document. Possible states are:

- **Not implemented** - the specification exists; no supporting code is present. The behavior described is what meiksh intends to provide once implementation lands.
- **Partially implemented** - some of the spec is honored by the shell; the `Status` section identifies which sections are live and which are not.
- **Implemented** - the shell implements all normative requirements of the spec; conformance tests under `tests/matrix/` reference the spec.

| Spec | Status |
|---|---|
| [emacs-editing-mode.md](emacs-editing-mode.md) - Emacs-style interactive line-editing mode enabled by `set -o emacs`, including the `bind` builtin. A pragmatic subset of GNU Readline's emacs mode. | Not implemented |
| [inputrc.md](inputrc.md) - The `inputrc` configuration file format used by the `bind -f` builtin and read at startup from `$INPUTRC`, `$HOME/.inputrc`, or `/etc/inputrc`. | Not implemented |
