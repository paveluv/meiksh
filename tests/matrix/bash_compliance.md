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

## 8) `! !` multiple pipeline negations accepted

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`:

> "```
> pipeline         :      pipe_sequence
>                  | Bang pipe_sequence
>                  ;
> ```"

**Why this is non-compliant**  
The POSIX grammar allows exactly one `Bang` (`!`) per pipeline. Bash in `--posix` mode accepts multiple `!` tokens (e.g., `! ! true`) and applies double negation, extending the grammar.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c '! ! true' 2>/dev/null
printf "exit=%s\n" "$?"
```

Expected:
- syntax error and non-zero exit status

Observed:
- `exit=0`
- no error

---

## 9) Variable assignment error before regular command does not exit

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> | **Error** | **Non-Interactive Shell** | **Interactive Shell** | **Shell Diagnostic Message Required** |
> | --- | --- | --- | --- |
> | Variable assignment error | shall exit | shall not exit | yes |

**Why this is non-compliant**  
The 2.8.1 table requires a non-interactive shell to exit on any variable assignment error, with no exception based on the type of command that follows. Bash in `--posix` mode only exits when the assignment precedes a special built-in utility; when it precedes a regular command (e.g., `env`), bash writes a diagnostic but continues execution.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c '
  readonly X=1
  X=2 env >/dev/null
  echo "survived rc=$?"
'
```

Expected:
- shell exits before `echo survived`
- non-zero exit status

Observed:
- `survived rc=1`
- shell continues execution

---

## 10) `time -p` not recognized in POSIX mode

**POSIX passage (exact quotes)**  
From `docs/posix/md/utilities/time.md`:

SYNOPSIS:

> "`time [-p] utility [argument...]`"

OPTIONS:

> "The following option shall be supported:
>
> **-p**: Write the timing output to standard error in the format shown in the STDERR section."

STDERR:

> "If **-p** is specified, the following format shall be used for the timing statistics in the POSIX locale:
>
> `"real %f\nuser %f\nsys %f\n", <real seconds>, <user seconds>, <system seconds>`"

**Why this is non-compliant**  
Bash implements `time` as a shell reserved word. In `--posix` mode, the keyword does not recognize `-p` as an option; instead it treats `-p` as the utility operand to be timed, resulting in a command-not-found error. Outside POSIX mode, `time -p` works correctly. The POSIX standard requires `-p` to be supported regardless of whether `time` is implemented as a keyword or external utility.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'time -p true' 2>&1
```

Expected:
- timing output on stderr in POSIX format (`real`, `user`, `sys` lines)
- exit status `0`

Observed:
- `/usr/bin/bash: line 1: time: command not found`
- exit status `127`

Contrast with non-POSIX mode (works correctly):

```sh
/usr/bin/bash -c 'time -p true' 2>&1
```

Observed:
- `real 0.00` / `user 0.00` / `sys 0.00`
- exit status `0`

---

## 11) `unset` of readonly variable does not exit non-interactive shell

**POSIX passage (exact quotes)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> | **Error** | **Non-Interactive Shell** | **Interactive Shell** | **Shell Diagnostic Message Required** |
> | --- | --- | --- | --- |
> | Special built-in utility error | shall exit | shall not exit | no |

> "The shell shall exit only if the special built-in utility is executed directly. If it is executed via the *command* utility, the shell shall not exit."

From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities (which lists `unset` as a special built-in).

From the `unset` specification:

> "Read-only variables cannot be unset."

> "EXIT STATUS: 0 — All *name* operands were successfully unset. >0 — At least one *name* could not be unset."

**Why this is non-compliant**  
When `unset` is invoked directly and fails because the operand names a readonly variable, it returns >0. Since `unset` is a special built-in, this is a special built-in utility error, and the non-interactive shell shall exit. Bash in `--posix` mode writes a diagnostic but continues execution. Other special built-in errors (e.g., `export -Z`, `set -Z`, `readonly -Z`) correctly cause bash to exit.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'readonly X=1; unset X; echo "survived"' 2>&1
```

Expected:
- shell exits after `unset X` fails
- no `survived` output
- non-zero exit status

Observed:
- `bash: line 1: unset: X: cannot unset: readonly variable`
- `survived`
- exit status `0` (shell continued)

Cross-reference with a compliant shell:

```sh
dash -c 'readonly X=1; unset X; echo "survived"' 2>&1
```

Observed (dash):
- `dash: 1: unset: X: is read only`
- no `survived` output
- exit status `2`

---

## 12) `printf` conversion error exit status masked by `%b` `\c`

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/printf.md`:

> "If an *argument* operand cannot be completely converted into an internal value appropriate to the corresponding conversion specification, a diagnostic message shall be written to standard error and the utility shall not exit with a zero exit status, but shall continue processing any remaining operands and shall write the value accumulated at the time the error was detected to standard output."

**Why this is non-compliant**  
When a numeric conversion error occurs (e.g., `abc` for `%d`) and a subsequent `%b` argument contains `\c` (which causes `printf` to stop processing), bash returns exit status 0 instead of non-zero. The `\c` escape triggers an immediate return in the `printf` builtin that bypasses the check of the `conversion_error` flag. Without `\c`, the same conversion error correctly returns exit status 1.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'printf "%d%b" abc "\c" 2>/dev/null; echo "exit=$?"'
```

Expected:
- exit status non-zero (conversion error on `abc`)

Observed:
- `0exit=0`
- exit status `0` (conversion error masked)

Contrast without `\c`:

```sh
/usr/bin/bash --posix -c 'printf "%d" abc 2>/dev/null; echo "exit=$?"'
```

Observed:
- `0exit=1`
- exit status `1` (conversion error correctly reported)

Cross-reference with a compliant shell:

```sh
dash -c 'printf "%d%b" abc "\c" 2>/dev/null; echo "exit=$?"'
```

Observed (dash):
- `0exit=1`
- exit status `1` (correct)

---

## 13) `break 0` and `continue 0` do not exit non-interactive shell

**POSIX passage (exact quotes)**  
From `docs/posix/md/utilities/V3_chap02.md`, break EXIT STATUS:

> "- 0: Successful completion.
> - \>0: The *n* value was not an unsigned decimal integer greater than or equal to 1."

From `docs/posix/md/utilities/V3_chap02.md`, continue EXIT STATUS:

> "- 0: Successful completion.
> - \>0: The *n* value was not an unsigned decimal integer greater than or equal to 1."

From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> | **Error** | **Non-Interactive Shell** |
> | --- | --- |
> | Special built-in utility error | shall exit |

From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities (which lists both `break` and `continue` as special built-ins).

**Why this is non-compliant**  
`break` and `continue` require the operand *n* to be an unsigned decimal integer greater than or equal to 1. When *n* is 0 (or negative), both utilities write a diagnostic and return a non-zero exit status, which is a special built-in utility error. Per 2.8.1, a non-interactive shell shall exit when a special built-in encounters an error. Bash in `--posix` mode writes the diagnostic but continues execution. Notably, bash correctly exits for non-numeric operands (e.g., `break abc`) but not for the numeric-but-invalid case of zero.

**Reproduction (portable shell commands)**

```sh
/usr/bin/bash --posix -c 'for i in 1; do break 0; done; echo "survived"' 2>&1
```

Expected:
- shell exits after `break 0` error
- no `survived` output
- non-zero exit status

Observed:
- `bash: line 1: break: 0: loop count out of range`
- `survived`
- exit status `0` (shell continued)

Same for `continue`:

```sh
/usr/bin/bash --posix -c 'for i in 1; do continue 0; done; echo "survived"' 2>&1
```

Expected:
- shell exits after `continue 0` error
- no `survived` output
- non-zero exit status

Observed:
- `bash: line 1: continue: 0: loop count out of range`
- `survived`
- exit status `0` (shell continued)

Cross-reference with a compliant shell:

```sh
dash -c 'for i in 1; do break 0; done; echo "survived"' 2>&1
```

Observed (dash):
- `dash: 1: break: Illegal number: 0`
- no `survived` output
- exit status `2`

---
