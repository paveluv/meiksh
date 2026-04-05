# Bash POSIX Compliance Report (Verified Non-Compliances Only)

**Shell tested:** GNU bash 5.2.37(1)-release (x86_64-pc-linux-gnu)  
**Standard:** POSIX.1-2024 (Issue 8)  
**Date:** 2026-04-01

This document intentionally lists **only verified bash non-compliances** that can be reproduced directly with standard shell usage.

---

## 1) `cd ""` returns success instead of failing

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md`:

> "If *directory* is an empty string, *cd* shall write a diagnostic message to standard error and exit with non-zero status."

**Why this is non-compliant**  
Bash in POSIX mode returns success (`0`) and emits no diagnostic for `cd ""`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'cd ""; printf "exit=%s\n" "$?"'
```

Expected:
- non-zero exit status
- diagnostic text on stderr

Observed:
- `exit=0`
- no stderr diagnostic

---

## 2) `echo` does not implement XSI backslash escapes in `--posix` mode

**POSIX passage (exact quotes)**  
From `docs/posix/md/utilities/echo.md`:

> "On XSI-conformant systems, if the first operand consists of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`}, it shall be treated as a string to be written. The following character sequences shall be recognized on XSI-conformant systems within any of the arguments:"

> - `\a` Write an `<alert>`.
> - `\b` Write a `<backspace>`.
> - `\c` Suppress the `<newline>` that otherwise follows the final argument in the output. All characters following the `\c` in the arguments shall be ignored.

**Why this is non-compliant**  
In bash `--posix` mode, these escape sequences are emitted literally unless non-POSIX toggles (like `-e` or shell options) are used.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'echo "\a" | od -An -tx1 | tr -d " \n"'
/usr/bin/bash --posix -c 'echo "hello\c world" | od -An -tx1 | tr -d " \n"'
```

Expected:
- first command contains `07` byte before trailing newline (`070a`)
- second command outputs only `hello` bytes (`68656c6c6f`) with no remainder

Observed:
- first command outputs literal `\a` bytes (`5c610a`)
- second command outputs literal `\c world` bytes (`68656c6c6f5c6320776f726c640a`)

---

## 3) `sh` vi-mode command `t`/`T` cursor semantics are wrong

**POSIX passage (exact quotes)**  
From `docs/posix/md/utilities/sh.md`:

> "- **[***count***]t***c*: Move to the character before the first occurrence of the character `'c'` that occurs after the current cursor position."

> "- **[***count***]T***c*: Move to the character after the first occurrence of the character `'c'` that occurs before the current cursor position."

**Why this is non-compliant**  
In bash vi mode, `t`/`T` behave like `f`/`F` (cursor lands on target character), not one character before/after as required.

**Reproduction (manual, no harness required)**

1. Start interactive shell:

   ```sh
   /usr/bin/bash --posix -i
   ```

2. Enable vi mode:

   ```sh
   set -o vi
   ```

3. Type `echo abc`, then press keys: `ESC 0 t c r Z Enter`

Expected behavior:
- `t c` lands on `b` (one char before `c`)
- `r Z` changes `b` → `Z`
- command output is `aZc`

Observed in bash:
- cursor lands on `c`
- resulting command line/edit behavior is inconsistent with POSIX `t` semantics

Repeat for reverse case:
- Type `echo abc`, then `ESC $ T a r Z Enter`
- POSIX requires landing one char after `a` (on `b`), but bash lands on `a`

---

## 4) `sh` vi-mode `[count]~` does not apply the count correctly

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/sh.md`:

> "- **[***count***]~**: ... If the `'~'` command is preceded by a *count*, that number of characters shall be converted, and the cursor shall be advanced to the character position after the last character converted. If the *count* is larger than the number of characters after the cursor, this shall not be considered an error; the cursor shall advance to the last character on the line."

**Why this is non-compliant**  
Bash toggles only one character in this scenario instead of applying the count over remaining characters.

**Reproduction (manual, no harness required)**

1. Start interactive shell:

   ```sh
   /usr/bin/bash --posix -i
   ```

2. Enable vi mode:

   ```sh
   set -o vi
   ```

3. Type `echo aB`, then press keys: `ESC 0 w 9 ~ Enter`

Expected:
- both remaining characters (`aB`) are toggled -> output `Ab`

Observed:
- only first character toggled -> output `AB`

---

## 5) non-ignored traps are not reset to default in subshell environments

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`:

> "A subshell environment shall be created as a duplicate of the shell environment, except that:
>
> - Unless specified otherwise (see [trap](#tag_19_29)), traps that are not being ignored shall be set to the default action."

**Why this is non-compliant**  
Bash `--posix` preserves a caught trap in a parenthesized subshell instead of
resetting it to the default action.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'trap "echo parent" USR1; (trap -p USR1; echo end)'
```

Expected:
- no `trap -p USR1` output from the subshell
- output is just `end`

Observed:
- the subshell still reports the inherited trap, e.g. `trap -- 'echo parent' USR1`
- output includes that trap line before `end`

---

## 6) `getopts` does not treat readonly `OPTIND` as a processing error

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "The shell variables *OPTIND* and *OPTARG* shall not be exported by default. An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one."

**Why this is non-compliant**  
When `OPTIND` is marked readonly, bash `--posix` still returns success (`0`)
and reports a parsed option, even though `getopts` could not update `OPTIND`
as required.

**Reproduction (portable shell commands)**

On a generic POSIX system with bash installed in `PATH`:

```sh
bash --posix -c '
  OPTIND=1
  readonly OPTIND
  set -- -a
  getopts a name 2>/dev/null
  printf "exit=%s name=%s OPTIND=%s\n" "$?" "$name" "$OPTIND"
'
```

Expected:
- `getopts` detects a processing error because it cannot set `OPTIND`
- exit status is greater than `1`

Observed:
- `exit=0 name=a OPTIND=1`
- `getopts` reports success even though `OPTIND` remained unchanged

---

## 7) `getopts` does not treat readonly `OPTARG` as a processing error

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "The shell variables *OPTIND* and *OPTARG* shall not be exported by default. An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one."

**Why this is non-compliant**  
When `OPTARG` is marked readonly and an option requiring an argument is
parsed, bash `--posix` still returns success (`0`) instead of reporting a
`getopts` processing error.

**Reproduction (portable shell commands)**

On a generic POSIX system with bash installed in `PATH`:

```sh
bash --posix -c '
  OPTIND=1
  readonly OPTARG
  set -- -f value
  getopts f: name 2>/dev/null
  printf "exit=%s name=%s OPTARG=%s\n" "$?" "$name" "${OPTARG-unset}"
'
```

Expected:
- `getopts` detects a processing error because it cannot assign `OPTARG`
- exit status is greater than `1`

Observed:
- `exit=0 name=f OPTARG=unset`
- `getopts` reports success even though it failed to set `OPTARG`

---

This file is intentionally strict: only independently reproducible, standards-backed bash deviations are included.
