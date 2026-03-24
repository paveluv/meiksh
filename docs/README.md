# Docs

This directory contains project documentation and pointers to external standards material used while implementing `meiksh`.

## POSIX Reference Material

The `docs/posix/` tree is intentionally not committed to the repository.

Reason:
- the source material is published by The Open Group
- we use those pages locally as implementation references
- we do not vendor the HTML into git for copyright reasons

The path is ignored in `.gitignore`:

```text
docs/posix/
```

## How To Populate `docs/posix`

Create the directory structure:

```sh
mkdir -p docs/posix/{issue7,issue8,utilities,functions,validation}
```

Fetch the main shell documents:

```sh
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html" -o docs/posix/issue8/shell-command-language.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/sh.html" -o docs/posix/issue8/sh-utility.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/xrat/V4_xcu_chap01.html" -o docs/posix/issue8/shell-rationale.html

curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html" -o docs/posix/issue7/shell-command-language.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/sh.html" -o docs/posix/issue7/sh-utility.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/contents.html" -o docs/posix/issue8/contents.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9699919799/utilities/contents.html" -o docs/posix/issue7/contents.html
```

Fetch shell-related utility pages:

```sh
for spec in alias bg break cd command continue dot eval exec exit export fg jobs pwd read readonly return set shift times trap umask unalias unset wait; do
  curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/${spec}.html" -o "docs/posix/utilities/${spec}.html"
done
```

Fetch shell-related function pages:

```sh
for func in close dup dup2 exec fork isatty kill open pipe setpgid sigaction tcgetpgrp tcsetpgrp wait waitid waitpid wordexp; do
  curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/${func}.html" -o "docs/posix/functions/${func}.html"
done
```

Fetch shell chapter/index pages used by the main references:

```sh
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap01.html" -o docs/posix/utilities/V3_chap01.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html" -o docs/posix/utilities/V3_chap02.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap03.html" -o docs/posix/utilities/V3_chap03.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/contents.html" -o docs/posix/utilities/contents.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/V2_chap02.html" -o docs/posix/functions/V2_chap02.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/utilities/wait.html" -o docs/posix/utilities/wait.html
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/functions/waitpid.html" -o docs/posix/functions/waitpid.html
```

Fetch the validation reference:

```sh
curl -LfsS "https://pubs.opengroup.org/onlinepubs/9799919799/" -o docs/posix/validation/posix-test-suites.html
```

## Source URLs

Primary source:
- <https://pubs.opengroup.org/onlinepubs/9799919799/>

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

- `docs/spec-matrix.md` references the expected local `docs/posix/` layout.
- If you want a broader local mirror, fetch any additional linked `utilities/*.html` and `functions/*.html` pages referenced from the main shell documents.
- Keep the downloaded material untracked.
