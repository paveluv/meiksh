# Docs

This directory contains project documentation and the local standards-material workflow used while implementing `meiksh`.

## POSIX Reference Material

The `docs/posix/` tree is intentionally not committed to the repository.

Reason:
- the source material is published by The Open Group
- `meiksh` uses those pages locally as the only standards source of truth for POSIX conformance work
- the HTML is kept out of git for copyright reasons

The path is ignored in `.gitignore`:

```text
docs/posix/
```

## Required Local Mirror

For routine shell work, `meiksh` relies on a local mirror that includes:
- Issue 8 shell language, `sh`, and shell rationale pages
- Issue 7 shell language and `sh` pages for compatibility review
- Issue 8 Base Definitions chapter pages that shell text frequently cross-references
- shell-related utility pages under `docs/posix/utilities/`
- shell-relevant system-interface pages under `docs/posix/functions/`
- a publication index under `docs/posix/validation/`

The authoritative expected file list lives in `docs/posix-manifest.txt`.

## Fetch The Mirror

Use the manifest-driven fetch script:

```sh
./docs/fetch-posix-docs.sh
```

That script reads `docs/posix-manifest.txt`, downloads the POSIX tarball, and unpacks it under `docs/posix/`.

## Policy

- `docs/implementation-policy.md`: implementation-defined and temporary project choices

## Source URLs

Primary publication root:
- <https://pubs.opengroup.org/onlinepubs/9799919799/>

Issue 8 Base Definitions:
- <https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/contents.html>

Issue 8 shell command language:
- <https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html>

Issue 8 `sh` utility:
- <https://pubs.opengroup.org/onlinepubs/9799919799/utilities/sh.html>

Issue 8 shell rationale:
- <https://pubs.opengroup.org/onlinepubs/9799919799/xrat/V4_xcu_chap01.html>

Issue 7 shell command language:
- <https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html>

Issue 7 `sh` utility:
- <https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sh.html>

## Notes

- `docs/posix-manifest.txt` is the mirror contract, and `docs/fetch-posix-docs.sh` is expected to stay in sync with it.
- Keep the downloaded material untracked.
