# Requirements Docs

This directory contains the shell-conformance audit structure that sits below `docs/spec-matrix.md`.

## Purpose

The goal is to keep POSIX conformance tracking auditable and stable as implementation work continues. These files are documentation scaffolding for the standards-first workflow; they do not replace the local `docs/posix/` mirror as the only requirements source of truth.

## Files

- `conventions.md`: REQ-ID format, status vocabulary, and evidence rules
- `standards-inventory.md`: inventory of required local standards pages and mirror categories
- `gap-register.md`: current conformance backlog items tied to requirement areas

## Update Rules

- Add or update REQ IDs when a POSIX requirement is split into a smaller tracked item.
- Cite exact `docs/posix/...` paths and anchors whenever practical.
- Treat `docs/spec-matrix.md` as the main ledger and these files as supporting structure.
- Update the gap register when a previously broad partial area is broken into specific tasks or when a gap is fully closed.
- Do not use host-shell behavior as a requirements source; only use it in differential tests where the project policy allows it.
