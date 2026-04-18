# ash POSIX Compliance Report (Verified Non-Compliances Only)

**Shell tested:** FreeBSD `/bin/sh` (ash-derived, FreeBSD 15.0)  
**Standard:** POSIX.1-2024 (Issue 8)  
**Date:** 2026-04-17

This document intentionally lists **only verified non-compliances** of the
FreeBSD base-system `/bin/sh` (a direct descendant of the original Almquist
shell, commonly referred to as ash). Findings are based on a matrix run of
the meiksh POSIX conformance suites using:

```sh
./tests/matrix/run.sh --shell /bin/sh
```

against FreeBSD 15.0 `/bin/sh`. Behaviors that are explicitly
implementation-defined, unspecified, or that concern optional extensions
(e.g. `function` as a reserved word, `$'...'` dollar-single-quotes, `;&`
case fallthrough, `read -d`) are omitted — those are missing features, not
standard violations relative to the edition ash targets.

---

## 1) `trap -p` not supported

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, trap:

> "-p: Write to standard output a list of commands associated with each *condition* operand. [...] The shell shall format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same trapping results for the specified set of conditions."

**Why this is non-compliant**  
FreeBSD `/bin/sh` rejects `-p` as an illegal option. This affects every use
of `trap -p`, including `trap -p <condition>` for querying individual traps
and the `trap -p` form used in the POSIX DESCRIPTION example
(`save_traps=$(trap -p); eval "$save_traps"`).

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'trap "echo hi" INT; trap -p INT'
```

Expected:
- `trap -- 'echo hi' INT`

Observed:
- `trap: Illegal option -p`

Note: dash exhibits the same gap; bash, ksh, and zsh all support `trap -p`.

---

## 2) `trap` (no args) in a subshell does not show parent traps before modification

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.13 Shell Execution Environment:

> "If the command `trap` is executed with no arguments in a subshell environment before any trap commands have been executed in the subshell environment, it shall write the list of commands associated with each condition that were in effect when the subshell environment was created."

**Why this is non-compliant**  
When a subshell executes `trap` with no arguments before setting any traps
of its own, FreeBSD `/bin/sh` produces no output instead of listing the
parent shell's traps.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'trap "echo exit_trap" EXIT; ( trap )'
```

Expected:
- output contains `trap -- 'echo exit_trap' EXIT`

Observed:
- (empty — no output from the subshell `trap`)

---

## 3) Command substitution `trap` (no args) does not show parent traps before modification

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.13 Shell Execution Environment:

> "If the command `trap` is executed with no arguments in a subshell environment before any trap commands have been executed in the subshell environment, it shall write the list of commands associated with each condition that were in effect when the subshell environment was created."

(Command substitution is a subshell environment per 2.13 and 2.6.3.)

**Why this is non-compliant**  
The same gap as #2 applies when `trap` with no arguments runs inside a
command substitution — no trap list is produced.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'trap "echo parent_trap" USR1; out=$(trap); printf "%s\nend\n" "$out"'
```

Expected:
- `trap -- 'echo parent_trap' USR1\nend`

Observed:
- `\nend` (empty `out` followed by the `end` marker)

---

## 4) Interactive job-control background job does not print `[job] pid` on start

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.3.1 Asynchronous AND-OR Lists:

> "If the shell is interactive and the asynchronous AND-OR list became a background job, the job number and the process ID associated with the job shall be written to standard error using the format:
>
> `"[%d] %d\n", <job-number>, <process-id>`"

**Why this is non-compliant**  
When an interactive FreeBSD `/bin/sh` starts a background job (with job
control enabled via `set -m`), it does **not** emit the mandatory
`[job] pid` line at creation time. It only prints notifications for
subsequent state changes (e.g. `[1] + Done`, `[1] + Suspended`). POSIX
requires the `[N] pid` line on job creation.

This single deviation accounts for the majority of the job-control test
failures against FreeBSD `/bin/sh`.

**Reproduction (portable shell commands)**

Run interactively (e.g. from a PTY):

```sh
/bin/sh -im
sleep 30 &
```

Expected (on stderr):
- `[1] 12345`

Observed:
- (no `[N] pid` line; the job is created silently and the next prompt is written)

Cross-reference: bash, dash, ksh, and zsh all emit `[1] <pid>` on job
creation.

---

## 5) `set -v` / `set -o verbose` do not write input to standard error

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, set:

> "-v: The shell shall write its input to standard error as it is read."

> "-o *option*: [...] *verbose*: Equivalent to -v."

**Why this is non-compliant**  
With `-v` (or `-o verbose`) active, FreeBSD `/bin/sh` does not echo the
shell input to standard error when reading commands from a `-c` argument
or from a pipe. POSIX requires the shell to write every line of input to
stderr as it is consumed.

**Reproduction (portable shell commands)**

```sh
/bin/sh -v -c 'printf hello\n' 2>stderr_capture.txt
cat stderr_capture.txt
```

Expected:
- stderr contains the command text (e.g. `printf hello\n`)

Observed:
- stderr is empty

---

## 6) Redirection error on a function call exits non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Redirection error with function execution | shall not exit | shall not exit | yes"

(The table lists *Non-Interactive Shell*, *Interactive Shell*, and
*Shell Diagnostic Message Required* columns.)

**Why this is non-compliant**  
When a redirection applied to a function call fails, POSIX requires the
non-interactive shell to continue (with a diagnostic). FreeBSD `/bin/sh`
instead exits the shell, so subsequent commands never execute.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'f() { :; }; f </no/such/file; echo survived'
```

Expected:
- diagnostic on stderr, then `survived` on stdout

Observed:
- diagnostic only; `survived` is not printed

---

## 7) `readonly VAR=value` prefix on a simple command does not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1 Simple Commands:

> "If any of the variable assignments attempt to assign a value to a variable for which the *readonly* attribute is set in the current shell environment (regardless of whether the assignment is made in that environment), a variable assignment error shall occur."

From 2.8.1 Consequences of Shell Errors:

> "Variable assignment error | shall exit | shall not exit | yes"

**Why this is non-compliant**  
When a prefix assignment targets a read-only variable in front of a
regular command, FreeBSD `/bin/sh` does not treat the failure as a
variable assignment error that exits a non-interactive shell. Subsequent
commands continue to run.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'readonly R=1; R=2 true; echo survived'
```

Expected:
- diagnostic on stderr, shell exits without printing `survived`

Observed:
- `survived`

---

## 8) `unset` of a readonly variable produces no diagnostic and no non-zero exit from the shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, unset:

> "If the variable or function to unset has the *readonly* attribute, an error shall result."

And per 2.8.1, for special built-in utility errors the *Shell Diagnostic
Message Required* column is "yes" (note 2 allows the diagnostic to come
from the utility itself) and the non-interactive shell "shall exit".

**Why this is non-compliant**  
FreeBSD `/bin/sh` reports a non-zero exit status from `unset` but writes
nothing to stderr and does not terminate a non-interactive shell.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'readonly R=1; unset R 2>err.txt; rc=$?; echo rc=$rc; cat err.txt; rm -f err.txt; echo survived'
```

Expected:
- stderr contains a diagnostic, shell exits, `survived` is not printed

Observed:
- `rc=2`, empty stderr, `survived` printed

---

## 9) `shift` with invalid operand writes no diagnostic

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, shift:

> "If the *n* operand is invalid or is greater than `"$#"`, this may be treated as an error and a non-interactive shell may exit; if the shell does not exit in this case, a non-zero exit status shall be returned and a warning message shall be written to standard error."

**Why this is non-compliant**  
When `shift` is called with no positional parameters, with `n` greater
than `$#`, or with a non-numeric operand, FreeBSD `/bin/sh` returns a
non-zero status but does not write a warning message to stderr.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'shift' 2>err.txt; echo "rc=$?"; cat err.txt; rm -f err.txt
/bin/sh -c 'set -- a b; shift 5' 2>err.txt; echo "rc=$?"; cat err.txt; rm -f err.txt
```

Expected:
- non-zero `rc`, and a diagnostic (e.g. `shift: can't shift that many`) on stderr

Observed:
- non-zero `rc`, stderr is empty

---

## 10) Filename pathname expansion matches leading `.` after `/`

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.14.3 Patterns Used for Filename Expansion:

> "If a filename begins with a `<period>` (`'.'`), the `<period>` shall be explicitly matched by using a `<period>` as the first character of the pattern or immediately following a `<slash>` character. The leading `<period>` shall not be matched by: [...]"

**Why this is non-compliant**  
For a pattern of the form `dir/*`, the component after `/` must not match
filenames that begin with `.`. FreeBSD `/bin/sh` nevertheless expands
`dir/*` to include `.`, `..`, and `.hidden`.

**Reproduction (portable shell commands)**

```sh
mkdir -p tmp_pat/sub
: > tmp_pat/sub/visible
: > tmp_pat/sub/.hidden
/bin/sh -c 'set -- tmp_pat/sub/*; printf "%s\n" "$@"'
```

Expected:
- `tmp_pat/sub/visible`

Observed:
- `tmp_pat/sub/visible`
- `tmp_pat/sub/.`
- `tmp_pat/sub/..`
- `tmp_pat/sub/.hidden`

---

## 11) Collating symbols `[[.x.]]` not supported in pattern matching

**POSIX passage (exact quote)**  
From `docs/posix/md/basedefs/V1_chap09.md`, 9.3.5 RE Bracket Expression:

> "A collating symbol is a collating element enclosed within bracket-period (`[.` and `.]`) delimiters."

From `docs/posix/md/utilities/V3_chap02.md`, 2.14 Pattern Matching Notation:

> "The pattern bracket expression `[...]` shall match [...] as described in XBD RE Bracket Expression."

**Why this is non-compliant**  
FreeBSD `/bin/sh` does not recognize `[[.x.]]` syntax in `case` patterns.
Cross-reference: dash exhibits the same gap (dash compliance #16).

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'case "a" in [[.a.]]) echo match;; *) echo nomatch;; esac'
```

Expected:
- `match`

Observed:
- `nomatch`

---

## 12) Equivalence classes `[[=x=]]` not supported in pattern matching

**POSIX passage (exact quote)**  
From `docs/posix/md/basedefs/V1_chap09.md`, 9.3.5 RE Bracket Expression:

> "An equivalence class expression shall represent the set of collating elements belonging to an equivalence class."

**Why this is non-compliant**  
FreeBSD `/bin/sh` does not recognize `[[=x=]]` syntax in `case` patterns.
Cross-reference: dash exhibits the same gap (dash compliance #17).

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'case "a" in [[=a=]]) echo match;; *) echo nomatch;; esac'
```

Expected:
- `match`

Observed:
- `nomatch`

---

## 13) Unrecoverable read error: non-interactive shell does not exit and writes no diagnostic

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Unrecoverable read error when reading commands | shall exit | shall exit | yes"

And note 4:

> "If an unrecoverable read error occurs when reading commands, other than from the *file* operand of the [*dot*](#dot) special built-in, the shell shall execute no further commands [...]"

**Why this is non-compliant**  
When the shell is reading commands from a file descriptor that becomes
unrecoverably unreadable (e.g. closed underneath it), FreeBSD `/bin/sh`
continues executing with a zero exit status and emits no diagnostic,
rather than exiting with a non-zero status and a diagnostic.

**Reproduction (portable shell commands)**

```sh
# Feed the shell a command stream whose FD is torn down mid-read.
# On a conforming shell this yields rc != 0 and a diagnostic on stderr.
printf 'echo first\nexec 0>&-\necho after_close\n' | /bin/sh 2>err.txt
echo rc=$?
cat err.txt
```

Expected:
- non-zero `rc`, and a diagnostic on stderr

Observed:
- `rc=0`, empty stderr (`first` is printed and the shell exits cleanly)

---

## 14) `break 0` and `continue 0` do not exit non-interactive shell

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, break:

> "The application shall ensure that the value of *n* is a positive decimal integer."

And under EXIT STATUS:

> ">0: The *n* value was not an unsigned decimal integer greater than or equal to 1."

Continue has identical wording. Per 2.8.1, a "Special built-in utility
error" causes the non-interactive shell to exit.

**Why this is non-compliant**  
`break 0` and `continue 0` are invalid invocations of special built-ins;
POSIX requires a non-interactive shell to exit. FreeBSD `/bin/sh` simply
returns a non-zero status and allows the script to continue.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'for i in 1; do break 0; done; echo survived'
/bin/sh -c 'for i in 1; do continue 0; done; echo survived'
```

Expected:
- shell exits before printing `survived`

Observed:
- `survived`

---

## 15) `exec` does not recognize multi-digit IO_NUMBER

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.7 Redirection:

> "If one or more digits are written before a redirection operator, the number formed from these digits shall be used for the file descriptor."

From 2.10.2 Shell Grammar Rules, rule 2:

> "If the string consists solely of digits and the delimiter character is '<' or '>', the token identifier IO_NUMBER shall be returned."

**Why this is non-compliant**  
FreeBSD `/bin/sh` correctly parses multi-digit fds in most command
contexts but misinterprets the leading digits of `exec 10>file` as a
command argument rather than an IO_NUMBER token. Cross-reference: dash
exhibits the same gap (dash compliance #14).

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'exec 10>/tmp/_ash_fd10 2>&1'; echo "rc=$?"
```

Expected:
- silent success, fd 10 opened, `rc=0`

Observed:
- `exec: 10: not found`, `rc=127`

---

## 16) CDPATH match does not write new directory to standard output

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md`:

> "If a non-empty directory name from *CDPATH* is used, or if the operand `'-'` is used, and the absolute pathname of the new working directory can be determined, that pathname shall be written to the standard output [...]"

**Why this is non-compliant**  
After a successful `cd` that resolves through a non-empty `CDPATH` entry,
FreeBSD `/bin/sh` does not print the new working directory.

**Reproduction (portable shell commands)**

```sh
base=$(mktemp -d)
mkdir -p "$base/searchroot/target"
CDPATH=$base/searchroot /bin/sh -c 'out=$(cd target); printf "[%s]\n" "$out"'
```

Expected:
- output is the absolute pathname of `$base/searchroot/target`

Observed:
- `[]` (empty)

---

## 17) `cd -` does not write the previous directory to standard output

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md`:

> "If *directory* consists of a single `'-'` (`<hyphen-minus>`) character, the *cd* utility shall behave as if *directory* contained the value of the *OLDPWD* environment variable, except that after it sets the value of *PWD* it shall write the new value to standard output."

**Why this is non-compliant**  
When invoked inside a command substitution (or any non-terminal stdout),
FreeBSD `/bin/sh` does not reliably write the new `PWD` to stdout after
`cd -`. In the matrix harness, the captured `$(cd -)` is empty even when
`OLDPWD` is set to a valid directory.

**Reproduction (portable shell commands)**

```sh
a=$(mktemp -d); b=$(mktemp -d)
/bin/sh -c 'cd "$1"; cd "$2"; out=$(cd -); printf "[%s]\n" "$out"' _ "$a" "$b"
```

Expected:
- output is the absolute pathname of `$a`

Observed:
- `[]` (empty)

---

## 18) Tilde expansion with null `HOME` produces zero fields instead of one empty field

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.1 Tilde Expansion:

> "If the login name is the empty string, the tilde-prefix shall be replaced by the value of the variable HOME."

When `HOME` is the null string, the tilde expands to the null string — a
single (empty) field.

**Why this is non-compliant**  
FreeBSD `/bin/sh` leaves `~` unexpanded when `HOME` is empty, producing
the literal one-field result `~` instead of a single empty field.
Cross-reference: dash silently drops the field to zero; bash/ksh/zsh
produce a single empty field.

**Reproduction (portable shell commands)**

```sh
/bin/sh -c 'HOME=""; set -- ~; printf "count=%s\n" "$#"; printf "[%s]\n" "$@"'
```

Expected:
- `count=1`
- `[]`

Observed:
- `count=1`
- `[~]` (the tilde is not expanded at all)

---

## 19) Multi-byte IFS character is truncated when joining `$*`

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.5.2 Special Parameters:

> "Expands to the positional parameters, starting from one, initially producing one field for each positional parameter that is set. [...] When the expansion occurs in a context where field splitting will not be performed, the initial fields shall be joined to form a single field with the value of each parameter separated by the first character of the *IFS* variable if *IFS* contains at least one character, or separated by a `<space>` if *IFS* is unset, or with no separator if *IFS* is set to a null string."

"First character of the *IFS* variable" means the first (possibly
multi-byte) character, not the first byte.

**Why this is non-compliant**  
When IFS begins with a multi-byte character (e.g. U+00E9 `é` in UTF-8,
encoded as `c3 a9`), FreeBSD `/bin/sh` uses only the first **byte**
(`0xc3`) as the separator in the quoted `$*` expansion, producing
malformed UTF-8. POSIX requires the full multi-byte character.

**Reproduction (portable shell commands)**

```sh
LC_ALL=en_US.UTF-8 /bin/sh -c '
IFS="$(printf "\xc3\xa9")X"
set -- a b
printf "%s" "$*" | od -An -tx1 | tr -d " "
'
```

Expected:
- `61c3a962` (`a`, U+00E9, `b`)

Observed:
- `61c362` (`a`, 0xc3, `b` — UTF-8 lead byte only)

---

## 20) `read` does not treat a multi-byte IFS character as a single delimiter

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/read.md`:

> "The characters in the value of the *IFS* variable shall be used to split the line into fields [...]"

In a multi-byte locale, the "characters" of IFS are characters, not
bytes, so a single multi-byte delimiter should delimit one field.

**Why this is non-compliant**  
FreeBSD `/bin/sh` treats each byte of a multi-byte IFS character as an
independent delimiter, producing spurious empty fields and leaving the
trailing bytes of the character in the next field.

**Reproduction (portable shell commands)**

```sh
LC_ALL=en_US.UTF-8 /bin/sh -c '
IFS="$(printf "\xc3\xa9")"
printf "a%sb\n" "$IFS" | { read x y; printf "[%s]|[%s]\n" "$x" "$y"; }
'
```

Expected:
- `[a]|[b]`

Observed:
- `[a]|[` followed by the trailing UTF-8 continuation byte and then `b`

---

## 21) `time` operand with a `/` does not bypass PATH search

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/time.md`:

> "The time utility shall invoke the utility named by the *utility* operand with arguments supplied as the *argument* operands and write a message to standard error that lists timing statistics for the utility."

And from `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.4 Command Search
and Execution:

> "If the command name contains at least one `<slash>`, the shell shall execute the utility in a separate utility environment with actions equivalent to calling the *execve*() function [...] with the *pathname* of the utility as the file argument."

**Why this is non-compliant**  
When `time` is given a pathname containing `/`, the shell / `time`
combination in FreeBSD `/bin/sh` still performs PATH lookup and finds a
different executable, instead of honoring the slash-qualified pathname
directly.

**Reproduction (portable shell commands)**

```sh
base=$(mktemp -d)
mkdir -p "$base/bin1" "$base/bin2"
cat > "$base/bin1/hello" <<'EOF'
#!/bin/sh
echo from-path
EOF
cat > "$base/bin2/hello" <<'EOF'
#!/bin/sh
echo from-direct
EOF
chmod +x "$base/bin1/hello" "$base/bin2/hello"
PATH=$base/bin1:$PATH /bin/sh -c 'time "$0/bin2/hello"' "$base" 2>/dev/null
```

Expected:
- `from-direct`

Observed:
- `from-path`

---

This file is intentionally strict: only independently reproducible,
standards-backed FreeBSD `/bin/sh` deviations are included.
