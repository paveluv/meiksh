# ksh POSIX Compliance Report (Verified Non-Compliances Only)

**Shell tested:** ksh93u+m/1.0.10 2024-08-01 (AT&T Research, AJM)  
**Standard:** POSIX.1-2024 (Issue 8)  
**Date:** 2026-04-14

This document intentionally lists **only verified ksh non-compliances** that
can be reproduced directly with standard shell usage. Behaviors that are
explicitly implementation-defined or unspecified by POSIX are omitted.

---

## 1) Octal constants in arithmetic expansion treated as decimal

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.4 Arithmetic Expansion:

> "Only the decimal-constant, octal-constant, and hexadecimal-constant constants specified in the ISO C standard, Section 6.4.4.1 are required to be recognized as constants."

ISO C 6.4.4.1 defines an octal constant as a `0` prefix followed by octal
digits; `010` is the octal constant for decimal 8.

**Why this is non-compliant**  
ksh93u+m treats `010` as the decimal value 10, ignoring the leading-zero octal
prefix required by ISO C / POSIX.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'echo $((010))'
```

Expected:
- `8`

Observed:
- `10`

---

## 2) `trap -p CONDITION` omits the `trap --` format

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, trap:

> "-p: Write to standard output a list of commands associated with each *condition* operand."

The format requirement (same section):

> "trap -- %s %s ...\n", \<action\>, \<condition\> ...

And:

> "The shell shall format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same trapping results"

**Why this is non-compliant**  
When `trap -p` is invoked with a specific condition operand (e.g. `trap -p INT`),
ksh outputs only the action string (e.g. `echo hi`) instead of the full
`trap -- 'echo hi' INT` format. The output is not suitable for reinput to the
shell.

Note: `trap -p` with no operands and `trap` with no operands both produce the
correct format. Only `trap -p CONDITION` is broken.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'trap "echo hi" INT; trap -p INT'
```

Expected:
- `trap -- 'echo hi' INT`

Observed:
- `echo hi`

---

## 3) Child script does not show ignored traps via `trap -p`

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.13 Shell Execution Environment:

> "If the utility is a shell script, traps caught by the shell shall be set to the default values and traps ignored by the shell shall be set to be ignored by the utility."

And from 2.15, trap DESCRIPTION:

> "The trap command with no operands shall write to standard output a list of commands associated with each of a set of conditions"

**Why this is non-compliant**  
When a parent shell ignores a signal with `trap '' INT` and then invokes a
child ksh script, `trap -p` and `trap` (no args) in the child produce no output
for the inherited ignored trap. POSIX requires that ignored traps be inherited
and be reportable.

**Reproduction (portable shell commands)**

```sh
cat > /tmp/ksh_trap_test.sh << 'EOF'
trap -p
EOF
/usr/bin/ksh -c 'trap "" INT; /usr/bin/ksh /tmp/ksh_trap_test.sh'
```

Expected:
- `trap -- '' INT`

Observed:
- (no output)

---

## 4) `wait` interrupted by trapped signal returns 1 instead of >128

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.12 Signals and Error Handling:

> "When the shell is waiting, by means of the wait utility, for asynchronous commands to complete, the reception of a signal for which a trap has been set shall cause the wait utility to return immediately with an exit status >128, immediately after which the trap associated with that signal shall be taken."

**Why this is non-compliant**  
ksh returns exit status 1 from `wait` when interrupted by a trapped signal
instead of the required >128.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c '
  trap "echo GOT_USR1" USR1
  sleep 60 & p=$!
  (sleep 0.1; kill -USR1 $$) &
  wait $p
  echo "wait_rc=$?"
'
```

Expected:
- `GOT_USR1` followed by `wait_rc=` with a value >128

Observed:
- `GOT_USR1` followed by `wait_rc=1`

---

## 5) Asynchronous list job notification uses TAB instead of SPACE

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.3.1 Examples:

> "If the shell is interactive and the asynchronous AND-OR list became a background job, the job number and the process ID associated with the job shall be written to standard error using the format:"
>
> "[%d] %d\n", \<job-number\>, \<process-id\>

**Why this is non-compliant**  
The POSIX format string `"[%d] %d\n"` specifies a literal space between the
closing bracket and the PID. ksh outputs a TAB character instead.

**Reproduction (portable shell commands)**

```sh
echo 'set -m; sleep 0.01 &' | /usr/bin/ksh -i 2>&1 | cat -A | grep '^\[1\]'
```

Expected:
- `[1] <pid>` with a space after `]`

Observed:
- `[1]^I<pid>` (TAB character shown as `^I`)

---

## 6) PS1 prompt not written to stderr when nested under `ksh -c`

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.5.3 Shell Variables, PS1:

> "After expansion, the value shall be written to standard error."

**Why this is non-compliant**  
When an interactive ksh is invoked from within `ksh -c` (e.g. `ksh -c 'ksh -i
2>file ...'`), the nested interactive shell does not write PS1 to standard
error. When invoked directly or from bash, PS1 is correctly written to stderr.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c '
  /usr/bin/ksh -i > /dev/null 2>/tmp/ps1_test.txt <<EOF
PS1="MARKER> "
:
exit
EOF
  grep -q "MARKER> " /tmp/ps1_test.txt && echo stderr_ok || echo stderr_missing
  rm -f /tmp/ps1_test.txt
'
```

Expected:
- `stderr_ok`

Observed:
- `stderr_missing`

Cross-reference: `bash -c '/usr/bin/ksh -i ...'` with the same heredoc does write
PS1 to stderr correctly, confirming the bug is specific to the parent being ksh.

---

## 7) Here-document delimiter search does not apply backslash-newline continuation

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.7.4 Here-Document:

> "The removal of \<backslash\>\<newline\> for line continuation (see 2.2.1 Escape Character (Backslash)) shall be performed during the search for the trailing delimiter."

**Why this is non-compliant**  
ksh does not perform line continuation when searching for the here-document
trailing delimiter. A delimiter split across two lines with backslash-newline
is not recognized.

**Reproduction (portable shell commands)**

```sh
printf 'cat <<EOF\nbefore\nEO\\\nF\necho after\nEOF\n' | /usr/bin/ksh
```

Expected:
- `before` followed by `after` (the `EO\<newline>F` is recognized as `EOF`)

Observed:
- `before`, `EO\`, `F`, `echo after` (backslash-newline not processed, here-doc
  extends to the final `EOF`)

---

## 8) Tilde expansion with null HOME produces zero fields instead of one

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.1 Tilde Expansion:

> "If the word being expanded consists of only the \<tilde\> character and HOME is set to the null string, this produces an empty field (as opposed to zero fields) as the expanded word."

**Why this is non-compliant**  
ksh produces zero fields (`$#` = 0) instead of one empty field (`$#` = 1).

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'HOME=""; set -- ~; echo "$#"'
```

Expected:
- `1`

Observed:
- `0`

---

## 9) `command -v` for alias does not output a re-executable command line

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/command.md`:

> "An alias shall be written as a command line that represents its alias definition."

**Why this is non-compliant**  
ksh outputs only the alias expansion value (e.g. `'echo hi'`) rather than a
complete command line that would recreate the alias (e.g. `alias greet='echo hi'`).

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'alias greet="echo hi"; command -v greet'
```

Expected:
- `alias greet='echo hi'` (or equivalent re-executable form)

Observed:
- `'echo hi'`

---

## 10) Variable assignment error on readonly variable does not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.2:

> "If any of the variable assignments attempt to assign a value to a variable for which the readonly attribute is set in the current shell environment (regardless of whether the assignment is made in that environment), a variable assignment error shall occur."

And from 2.8.1 Consequences of Shell Errors:

> "Variable assignment error — [Non-Interactive Shell] shall exit"

**Why this is non-compliant**  
ksh reports the error but continues execution instead of exiting the
non-interactive shell.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'readonly FOO=1; FOO=2 env; echo survived'
```

Expected:
- Shell exits after the assignment error; `survived` is not printed

Observed:
- `survived` is printed (shell continues)

---

## 11) `unset` of readonly variable does not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities:

> "An error in a special built-in utility may cause a shell executing that utility to abort"

And from 2.8.1:

> "Special built-in utility error — [Non-Interactive Shell] shall exit"

**Why this is non-compliant**  
`unset` is a special built-in. When it fails on a readonly variable, ksh prints
a warning but does not exit the non-interactive shell.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'readonly X=1; unset X; echo survived'
```

Expected:
- Shell exits; `survived` is not printed

Observed:
- `survived` is printed

---

## 12) `eval` syntax error does not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Special built-in utility error — [Non-Interactive Shell] shall exit"

`eval` is a special built-in. A syntax error from `eval 'if'` is a special
built-in error.

**Why this is non-compliant**  
ksh reports the syntax error but continues execution.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c '{ eval "if"; } 2>/dev/null; echo survived'
```

Expected:
- Shell exits; `survived` is not printed

Observed:
- `survived` is printed

---

## 13) Expansion error (`set -u`) does not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Expansion error — [Non-Interactive Shell] shall exit"

**Why this is non-compliant**  
With `set -u` in effect, expanding an unset variable is an expansion error.
ksh reports the error but continues execution.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'set -u; f() { echo $UNDEFINED_VAR; }; f 2>/dev/null; echo survived'
```

Expected:
- Shell exits; `survived` is not printed

Observed:
- `survived` is printed

---

## 14) `getopts` with readonly `OPTIND` does not return >1

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one."

**Why this is non-compliant**  
When `OPTIND` is readonly, `getopts` cannot update it. POSIX requires exit
status >1, but ksh returns 0.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'readonly OPTIND; getopts a: opt -a val; echo "rc=$?"'
```

Expected:
- `rc=` with a value >1

Observed:
- `rc=0`

---

## 15) `getopts` with readonly `OPTARG` does not return >1

**POSIX passage (exact quote)**  
Same as entry 14:

> "An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one."

**Why this is non-compliant**  
Same root cause as entry 14, but for `OPTARG` instead of `OPTIND`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'readonly OPTARG; getopts a: opt -a val; echo "rc=$?"'
```

Expected:
- `rc=` with a value >1

Observed:
- `rc=0`

---

## 16) `printf` with non-numeric argument exits zero and produces no diagnostic

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/printf.md`:

> "If an *argument* operand cannot be completely converted into an internal value appropriate to the corresponding conversion specification, a diagnostic message shall be written to standard error and the utility shall not exit with a zero exit status, but shall continue processing any remaining operands and shall write the value accumulated at the time the error was detected to standard output."

**Why this is non-compliant**  
ksh's `printf '%d' abc` exits with status 0 and writes no diagnostic to stderr.
Both the exit status and the missing diagnostic violate the specification.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'printf "%d" abc 2>/tmp/printf_err.txt; echo "rc=$?"; cat /tmp/printf_err.txt; rm -f /tmp/printf_err.txt'
```

Expected:
- Non-zero `rc=` value and diagnostic text in stderr

Observed:
- `rc=0` and empty stderr

---

## 17) Multiple bangs in pipeline accepted instead of causing syntax error

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.10 Shell Grammar:

> ```
> pipeline : pipe_sequence
>          | Bang pipe_sequence
>          ;
> ```

**Why this is non-compliant**  
The POSIX grammar allows exactly one `!` (Bang) before a pipe_sequence.
`! ! true` is not a valid production since after `Bang`, the parser expects
a `pipe_sequence`, and `!` is not a valid start of a command. ksh accepts
this as a double-negation extension.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c '! ! true; echo $?' 2>&1
```

Expected:
- Syntax error diagnostic on stderr

Observed:
- `0` (accepted and executed as double-negation)

Cross-reference: bash also accepts `! !` as a non-conforming extension. dash
correctly rejects it.

---

## 18) `time -p` not recognized as reserved word with `-p` option

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/time.md`:

> `time [-p] utility [argument...]`

And:

> "-p: Write the timing output to standard error in the format described in the STDERR section."

**Why this is non-compliant**  
ksh has `time` as a reserved word and it works without flags (e.g. `time sleep 1`).
However, `time -p` causes ksh to search for an external `time` utility instead
of recognizing `-p` as an option to the reserved word. The `-p` option is
required by POSIX.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'time -p true' 2>&1
```

Expected:
- POSIX-format timing output on stderr:
  `real 0.00`, `user 0.00`, `sys 0.00`

Observed:
- `ksh: time: not found`

---

## 19) `]` not recognized as literal at start of bracket expression in case patterns

**POSIX passage (exact quote)**  
From `docs/posix/md/xbd/V1_chap09.md`, 9.3.5 RE Bracket Expression:

> "The \<right-square-bracket\> (']') shall lose its special meaning and represent itself in a bracket expression if it occurs first in the list (after an initial \<circumflex\> ('^'), if any)."

**Why this is non-compliant**  
In a `case` pattern like `[]a-]`, the `]` immediately after `[` should be
treated as a literal character in the bracket expression. ksh fails to match
`]` against this pattern.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c 'case "]" in []a-]) echo match;; *) echo nomatch;; esac'
```

Expected:
- `match`

Observed:
- `nomatch`

Cross-reference: bash and dash both correctly output `match`.

---

## 20) Child script does not inherit exec-opened file descriptors

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.13 Shell Execution Environment:

> "Open files inherited on invocation of the shell, open files controlled by the exec special built-in plus any modifications, and additions specified by any redirections to the utility"

This establishes that file descriptors opened with `exec` are part of the
execution environment and shall be inherited by child utilities.

**Why this is non-compliant**  
When a file descriptor is opened via `exec 3<file` and a child shell script
is invoked, the child cannot read from fd 3 — it receives a "Bad file descriptor"
error. ksh appears to set close-on-exec on descriptors opened by `exec`.

**Reproduction (portable shell commands)**

```sh
echo "hello" > /tmp/ksh_fd_test.txt
cat > /tmp/ksh_fd_child.sh << 'EOF'
read line <&3
echo "$line"
EOF
/usr/bin/ksh -c 'exec 3</tmp/ksh_fd_test.txt; /usr/bin/ksh /tmp/ksh_fd_child.sh'
```

Expected:
- `hello`

Observed:
- `ksh: 3: cannot open [Bad file descriptor]`

---

## 21) `fc` fails with "invalid range" in interactive sessions started from `-c` mode

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/fc.md`:

> "The fc utility shall list, or shall edit and re-execute, commands previously entered to an interactive sh."

And the HISTFILE variable (from `docs/posix/md/utilities/V3_chap02.md`, 2.5.3):

> "When an interactive shell is entered, the shell may read commands from this file"

**Why this is non-compliant**  
When ksh is invoked as `ksh -c 'ksh -i ...'`, the inner interactive shell's
`fc` command fails with "invalid range" errors for all operations, including
simple `fc -l`. The history mechanism is not properly initialized. This
prevents any use of `fc` in such environments.

**Reproduction (portable shell commands)**

```sh
echo 'echo test_cmd; fc -l' | /usr/bin/ksh -i 2>&1
```

Expected:
- `fc -l` lists recently entered commands

Observed:
- `ksh: fc: 1-0: invalid range`

Note: this affects all `fc` operations (`-l`, `-s`, editing, etc.) when the
history is not properly initialized.

---

## 22) Multiple pending trapped signals may be lost

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.12 Signals and Error Handling:

> "If multiple signals are pending for the shell for which there are associated trap actions, the order of execution of trap actions is unspecified."

The "order is unspecified" language implies all pending signals shall be
delivered; only the order is left to the implementation.

**Why this is non-compliant**  
When two signals (USR1, USR2) are sent in rapid succession while the shell is
blocked, ksh may deliver only one of them and silently drop the other.

**Reproduction (portable shell commands)**

```sh
/usr/bin/ksh -c '
  trap "echo GOT_USR1" USR1
  trap "echo GOT_USR2" USR2
  (sleep 0.1; kill -USR1 $$; kill -USR2 $$) &
  sleep 0.3
  echo DONE
'
```

Expected:
- Both `GOT_USR1` and `GOT_USR2` (in either order), followed by `DONE`

Observed:
- Only one of the two signals fires; the other is lost (typically only
  `GOT_USR1` appears, `GOT_USR2` is dropped, or vice versa)

---

---

This file is intentionally strict: only independently reproducible,
standards-backed ksh deviations are included.
