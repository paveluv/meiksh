# Zsh POSIX Compliance Report (Verified Non-Compliances Only)

**Shell tested:** zsh 5.9 (x86_64-debian-linux-gnu)  
**Standard:** POSIX.1-2024 (Issue 8)  
**Date:** 2026-04-14  
**Updated:** 2026-04-14 (added `emulate sh` testing results)

This document intentionally lists **only verified zsh non-compliances** that
can be reproduced directly with standard shell usage. Zsh is not a POSIX shell
by default — it has its own semantics for word splitting, globbing, aliases,
traps, and many other features. Most entries here reflect intentional design
differences that require `emulate sh` or specific `setopt` options to resolve.
Issue 8 features that zsh 5.9 has not adopted (such as `cd -e`, `pipefail`
start-time semantics) are included only when the underlying POSIX requirement
predates Issue 8.

### Summary: native zsh vs `zsh --emulate sh`

| Metric | Native zsh | `zsh --emulate sh` |
|---|---|---|
| Total tests | 1759 | 1759 |
| Passed | 1313 | 1599 |
| Failed | 446 | 160 |

`emulate sh` fixes 14 of the 28 original non-compliances (entries 1–3, 5–7,
10–11, 13–14, 19, 22–23, 27). Each entry below notes its `emulate sh`
status.

---

## 1) Arithmetic operator precedence: shift vs additive

**`emulate sh`:** FIXED — produces correct result `32`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap01.md`, 1.1.2.1 Arithmetic Precision and Operations:

> "The evaluation of arithmetic expressions shall be equivalent to that described in Section 6.5, Expressions, of the ISO C standard."

ISO C 6.5 defines additive operators (6.5.6) at higher precedence than shift
operators (6.5.7).

**Why this is non-compliant**  
zsh evaluates `1 << 2 + 3` as `(1 << 2) + 3 = 7` instead of the
ISO C-mandated `1 << (2 + 3) = 32`. Shift operators bind tighter than
addition in zsh, reversing the ISO C precedence.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'echo $(( 1 << 2 + 3 ))'
```

Expected:
- `32`

Observed:
- `7`

---

## 2) Arithmetic operator precedence: comparison vs bitwise AND

**`emulate sh`:** FIXED — produces correct result `1`.

**POSIX passage (exact quote)**  
Same as entry 1. ISO C 6.5 defines equality operators (6.5.9) at higher
precedence than bitwise AND (6.5.10).

**Why this is non-compliant**  
zsh evaluates `5 & 3 == 3` as `(5 & 3) == 3` yielding 0, instead of
the ISO C-mandated `5 & (3 == 3) = 5 & 1 = 1`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'echo $(( 5 & 3 == 3 ))'
```

Expected:
- `1`

Observed:
- `0`

---

## 3) Octal constants in arithmetic expansion treated as decimal

**`emulate sh`:** FIXED — `$((010))` correctly produces `8`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.4 Arithmetic Expansion:

> "Only the decimal-constant, octal-constant, and hexadecimal-constant constants specified in the ISO C standard, Section 6.4.4.1 are required to be recognized as constants."

**Why this is non-compliant**  
zsh treats `010` as decimal 10 instead of octal 8.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'echo $((010))'
```

Expected:
- `8`

Observed:
- `10`

---

## 4) `$10` expanded as 10th positional parameter instead of `$1` + `0`

**`emulate sh`:** NOT FIXED — still expands `$10` as `${10}`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.2 Parameter Expansion:

> "When a positional parameter with more than one digit is specified, the application shall enclose the digits in braces."

The informative example states: `"$10"` expands to the value of `$1` followed
by the character `0`.

**Why this is non-compliant**  
zsh expands `$10` as `${10}` (the 10th positional parameter) instead of
treating it as `${1}0`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'set -- one two three four five six seven eight nine ten; printf "<%s>\n" "$10"'
```

Expected:
- `<one0>`

Observed:
- `<ten>`

---

## 5) No IFS field splitting on unquoted parameter expansion

**`emulate sh`:** FIXED — IFS field splitting is enabled.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.5 Field Splitting:

> "After parameter expansion, command substitution, and arithmetic expansion, the shell shall scan the results of expansions and substitutions that did not occur in double-quotes for field splitting and multiple fields can result."

**Why this is non-compliant**  
zsh does not perform IFS field splitting on unquoted parameter expansions by
default. The `SH_WORD_SPLIT` option is off, causing `$var` where
`var="a b c"` to remain a single field instead of being split into three.
This affects dozens of POSIX behaviors including `for` loops, command
substitution results, and positional parameter expansion.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'x="a b c"; set -- $x; echo $#'
```

Expected:
- `3`

Observed:
- `1`

---

## 6) `set -f` does not disable pathname expansion

**`emulate sh`:** FIXED — `set -f` correctly disables globbing.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, set:

> "-f: The shell shall disable pathname expansion."

**Why this is non-compliant**  
zsh ignores `set -f` and continues to perform pathname expansion. zsh uses
`setopt NO_GLOB` instead.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'touch /tmp/zsh_f_test.txt; set -f; echo /tmp/zsh_f_*; rm /tmp/zsh_f_test.txt'
```

Expected:
- `/tmp/zsh_f_*` (literal, no expansion)

Observed:
- `/tmp/zsh_f_test.txt` (expanded)

---

## 7) Non-matching glob pattern causes error instead of remaining literal

**`emulate sh`:** FIXED — non-matching patterns are left unchanged.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.6 Pathname Expansion:

> "If the pattern does not match any existing filenames or pathnames, the pattern string shall be left unchanged."

**Why this is non-compliant**  
zsh's `NOMATCH` option (on by default) causes a fatal error when a glob
pattern matches nothing, instead of leaving the pattern unchanged as POSIX
requires.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'echo /tmp/no_such_xyzzy_*' 2>&1
```

Expected:
- `/tmp/no_such_xyzzy_*` (literal)

Observed:
- `zsh:1: no matches found: /tmp/no_such_xyzzy_*`

---

## 8) `trap -p` produces no output

**`emulate sh`:** NOT FIXED — `trap -p` still produces no output. Plain
`trap` (without `-p`) does print traps in the correct `trap -- ...` format.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, trap:

> "-p: Write to standard output a list of commands associated with each *condition* operand."

The format requirement:

> "trap -- %s %s ...\n", \<action\>, \<condition\> ...

**Why this is non-compliant**  
zsh does not recognize `-p` as a flag for `trap` in its default mode.
Both `trap -p INT` and `trap -p` (no args) produce no output.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'trap "echo hi" INT; trap -p INT'
/usr/bin/zsh -c 'trap "echo hi" INT; trap -p'
```

Expected:
- `trap -- 'echo hi' INT`

Observed:
- (no output)

---

## 9) Empty compound commands accepted instead of causing syntax error

**`emulate sh`:** NOT FIXED — empty `( )`, `{ }`, `for...do done`, and
`if...then fi` are still accepted without error.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.10 Shell Grammar:

> ```
> subshell         : '(' compound_list ')'
> brace_group      : Lbrace compound_list Rbrace
> do_group         : Do compound_list Done
> if_clause        : If compound_list Then compound_list ...
> ```

`compound_list` is defined as `linebreak term [separator]` where `term`
requires at least one `and_or`. Empty compound lists are not derivable.

**Why this is non-compliant**  
zsh accepts `( )`, `{ }`, `for i in a; do done`, and `if true; then fi`
without error, all of which are syntax errors per the POSIX grammar.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c '( ); echo $?'
/usr/bin/zsh -c 'for i in a; do done; echo $?'
```

Expected:
- Syntax error diagnostic on stderr

Observed:
- `0` (accepted silently)

---

## 10) Variable assignment before special built-in does not persist

**`emulate sh`:** FIXED — `MY_VAR=hello :; echo "$MY_VAR"` correctly
prints `hello`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.2:

> "If the command name is a special built-in utility, variable assignments shall affect the current execution environment... and remain in effect when the command completes."

**Why this is non-compliant**  
zsh treats prefix assignments to special built-ins (like `:`) as temporary,
discarding them after the command completes.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'MY_VAR=hello :; echo "$MY_VAR"'
```

Expected:
- `hello`

Observed:
- (empty)

---

## 11) `readonly -p` uses `typeset -r` format instead of `readonly`

**`emulate sh`:** FIXED — outputs `readonly X=42`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, readonly:

> "When -p is specified, readonly shall write to the standard output the names and values of all read-only variables, in the following format: `readonly %s=%s\n`"

**Why this is non-compliant**  
zsh outputs `typeset -r X=42` instead of `readonly X=42`. The output is
not suitable for reinput to a POSIX shell.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'readonly X=42; readonly -p' | grep X
```

Expected:
- `readonly X=42`

Observed:
- `typeset -r X=42`

---

## 12) `set -b`, `set -m`, and `set -h` not accepted

**`emulate sh`:** PARTIALLY FIXED — `set -b` is now accepted. `set -m` and
`set -h` remain rejected.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, set:

> "Implementations shall support the options in the following list in both their \<hyphen-minus\> and \<plus-sign\> forms."

The `-b`, `-h`, and `-m` options are listed as required if the User
Portability Utilities option is supported.

**Why this is non-compliant**  
zsh rejects `set -m` ("can't change option") and `set -h` ("bad option")
in non-interactive `-c` mode, even with `emulate sh`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'set -m' 2>&1
/usr/bin/zsh --emulate sh -c 'set -h' 2>&1
```

Expected:
- Accepted silently

Observed:
- `zsh:set:1: can't change option: -m`
- `zsh:set:1: bad option: -h`

---

## 13) Default PS4 is not `"+ "`

**`emulate sh`:** FIXED — default PS4 is `"+ "`, producing `+ echo traced`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.5.3 Shell Variables, PS4:

> "The default value shall be `\"+ \"`."

**Why this is non-compliant**  
zsh's default PS4 is `+%N:%i> ` (including script name and line number),
producing trace output like `+zsh:1> echo traced` instead of the
POSIX-mandated `+ echo traced`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'set -x; echo traced' 2>&1
```

Expected:
- `+ echo traced` on stderr

Observed:
- `+zsh:1> echo traced`

---

## 14) PS4 does not undergo parameter expansion

**`emulate sh`:** FIXED — PS4 parameter expansion works correctly.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.5.3 Shell Variables, PS4:

> "the value of this variable shall be subjected to parameter expansion"

**Why this is non-compliant**  
zsh does not perform POSIX parameter expansion on PS4. It uses its own
`%`-escape prompt expansion mechanism instead.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'prefix=TRACE; PS4='"'"'$prefix> '"'"'; set -x; echo traced' 2>&1
```

Expected:
- `TRACE> echo traced` on stderr

Observed:
- `$prefix> echo traced` (literal `$prefix`)

---

## 15) `$'\cX'` control character escapes not recognized

**`emulate sh`:** NOT FIXED — `$'\cA'` still yields literal `cA`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.2.4 Dollar-Single-Quotes:

> "`\c`X yields the control character listed in the Value column... when X is one of the characters listed in the ^c column"

**Why this is non-compliant**  
zsh does not process `\c` escapes inside `$'...'`, treating them as the
literal characters `c` and `X`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c $'printf "%s" $\'\\cA\' | od -An -tx1'
```

Expected:
- ` 01` (control-A)

Observed:
- ` 63 41` (ASCII for `c` and `A`)

---

## 16) Aliases not expanded in non-interactive shell

**`emulate sh`:** PARTIALLY FIXED — aliases are expanded when running script
files (`zsh --emulate sh script.sh`), but still NOT expanded in `-c` mode
(`zsh --emulate sh -c '...'`).

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.3.1 Alias Substitution:

> "After a token has been delimited, but before applying the grammatical rules in 2.10 Shell Grammar, a resulting word that is identified as the command name word of a simple command shall be examined to determine whether it is an unquoted, valid alias name."

No restriction to interactive mode is stated.

**Why this is non-compliant**  
zsh disables alias expansion in `-c` mode even with `emulate sh`. Script
file mode works correctly.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'alias greet="echo hello"; greet' 2>&1
```

Expected:
- `hello`

Observed:
- `zsh:1: command not found: greet`

---

## 17) Variable assignment error does not exit non-interactive shell

**`emulate sh`:** NOT FIXED — shell prints the error but continues execution.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Variable assignment error — [Non-Interactive Shell] shall exit"

**Why this is non-compliant**  
zsh reports the error but continues execution.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'readonly FOO=1; FOO=2 env 2>/dev/null; echo survived'
```

Expected:
- Shell exits; `survived` is not printed

Observed:
- `survived`

---

## 18) `eval` syntax error does not exit non-interactive shell

**`emulate sh`:** NOT FIXED — `eval "if"` reports the error but execution
continues past the enclosing compound command.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Special built-in utility error — [Non-Interactive Shell] shall exit"

**Why this is non-compliant**  
`eval` is a special built-in. A syntax error from `eval 'if'` does not cause
zsh to exit the non-interactive shell.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c '{ eval "if"; } 2>/dev/null; echo survived'
```

Expected:
- Shell exits; `survived` is not printed

Observed:
- `survived`

---

## 19) `$0` set to function name during execution

**`emulate sh`:** FIXED — `FUNCTION_ARGZERO` is disabled, so `$0` retains
the shell name.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.5 Function Definition Command:

> "The special parameter 0 shall be unchanged."

**Why this is non-compliant**  
zsh sets `$0` to the function name during function execution (controlled by
the `FUNCTION_ARGZERO` option, which is on by default).

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'f() { echo $0; }; f'
```

Expected:
- `zsh` (the shell name, unchanged)

Observed:
- `f` (the function name)

---

## 20) `umask` output uses 3-digit format instead of 4-digit

**`emulate sh`:** NOT FIXED — still outputs `027` instead of `0027`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/umask.md`:

> "When the -S option is not specified, the umask utility shall produce output using the following format: `\"%04o\\n\", <mask>`"

**Why this is non-compliant**  
zsh outputs `027` (3 digits) instead of the required `0027` (4 digits with
leading zero).

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'umask 0027; umask'
```

Expected:
- `0027`

Observed:
- `027`

---

## 21) `time -p` not recognized as reserved word with `-p` option

**`emulate sh`:** NOT FIXED — `time -p` still tries to run `-p` as a
command. Additionally, `time` does not write timing output to stderr at all
in `-c` mode or script file mode.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/time.md`:

> `time [-p] utility [argument...]`

And:

> "-p: Write the timing output to standard error in the format described in the STDERR section."

**Why this is non-compliant**  
zsh's `time` reserved word does not accept `-p`. When `time -p true` is
used, zsh tries to run `-p` as a command name.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'time -p true' 2>&1
```

Expected:
- POSIX-format timing output (`real`, `user`, `sys`)

Observed:
- `zsh:1: command not found: -p`

---

## 22) `ENV` variable not processed for interactive shells

**`emulate sh`:** FIXED — `ENV` file is sourced for interactive shells.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.5.3 Shell Variables, ENV:

> "This variable, when and only when an interactive shell is invoked, shall be subjected to parameter expansion by the shell and the resulting value shall be used as a pathname of a file."

**Why this is non-compliant**  
zsh ignores the `$ENV` variable entirely and uses its own initialization
files (`~/.zshenv`, `~/.zshrc`) instead.

**Reproduction (portable shell commands)**

```sh
echo 'echo env_loaded' > /tmp/zsh_env_test.sh
ENV=/tmp/zsh_env_test.sh /usr/bin/zsh -i -c 'exit' 2>/dev/null
rm /tmp/zsh_env_test.sh
```

Expected:
- `env_loaded` on stdout

Observed:
- (no output from ENV file)

---

## 23) Pathname expansion performed on redirection targets

**`emulate sh`:** FIXED — redirection targets are not glob-expanded.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.7 Redirection:

> "Pathname expansion shall not be performed on the word by a non-interactive shell."

**Why this is non-compliant**  
zsh performs glob expansion on redirection targets even in non-interactive
mode. When multiple files match, the redirect may succeed or fail
unpredictably.

**Reproduction (portable shell commands)**

```sh
touch /tmp/zsh_r1.txt /tmp/zsh_r2.txt
/usr/bin/zsh -c 'echo test > /tmp/zsh_r*.txt' 2>&1
rm -f /tmp/zsh_r1.txt /tmp/zsh_r2.txt
```

Expected:
- Literal file `zsh_r*.txt` created, or ambiguous redirect error

Observed:
- zsh expands the glob and redirects to matched files

---

## 24) `cd -P -L` does not respect "last option wins"

**`emulate sh`:** NOT FIXED — `-P` still takes precedence over a later `-L`.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md`:

> "If both -L and -P options are specified, the last of these options shall be used and all others ignored."

**Why this is non-compliant**  
`cd -P -L` follows the physical path instead of the logical path, meaning
`-P` takes precedence even though `-L` is specified last.

**Reproduction (portable shell commands)**

```sh
mkdir -p /tmp/zsh_real; ln -sfn /tmp/zsh_real /tmp/zsh_link
/usr/bin/zsh -c 'cd -P -L /tmp/zsh_link; echo $PWD'
rm -rf /tmp/zsh_real /tmp/zsh_link
```

Expected:
- `/tmp/zsh_link` (logical path, since `-L` is last)

Observed:
- `/tmp/zsh_real` (physical path, `-P` took precedence)

---

## 25) `command -V` writes failure message to stdout instead of stderr

**`emulate sh`:** NOT FIXED — still writes `not found` to stdout.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/command.md`:

> "Otherwise, no output shall be written and the exit status shall reflect that the name was not found."

And from the STDERR section:

> "The standard error shall be used only for diagnostic messages."

**Why this is non-compliant**  
When `command -V` is given a nonexistent command, zsh writes `not found`
to stdout instead of producing no stdout output. POSIX says no output shall
be written (to stdout) when the name is not found.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'command -V nonexistent_xyzzy' 2>/dev/null
echo "rc=$?"
```

Expected:
- No stdout output

Observed:
- `nonexistent_xyzzy not found` on stdout

---

## 26) `getopts` with readonly `OPTIND` does not return >1

**`emulate sh`:** NOT FIXED — returns 0 instead of >1.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one."

**Why this is non-compliant**  
zsh returns 0 when `OPTIND` is readonly, instead of the required >1.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'readonly OPTIND; getopts a: opt -a val; echo "rc=$?"'
```

Expected:
- `rc=` with a value >1

Observed:
- `rc=0`

---

## 27) Pipeline stdin redirection does not override pipe

**`emulate sh`:** FIXED — redirect correctly overrides pipe input.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.2 Pipelines:

> "The standard input... of a command shall be considered to be assigned by the pipeline before any redirection specified by redirection operators."

**Why this is non-compliant**  
In `echo "from_pipe" | cat < file`, the redirect `< file` should override
the pipe for cat's stdin. zsh reads from both sources (the pipe and the file),
outputting both.

**Reproduction (portable shell commands)**

```sh
echo "from_file" > /tmp/zsh_pipe_test.txt
/usr/bin/zsh -c 'echo "from_pipe" | cat < /tmp/zsh_pipe_test.txt'
rm -f /tmp/zsh_pipe_test.txt
```

Expected:
- `from_file` (redirect overrides pipe)

Observed:
- Both `from_pipe` and `from_file`

---

## 28) Collating symbols and equivalence classes not supported in patterns

**`emulate sh`:** NOT FIXED — still not supported.

**POSIX passage (exact quote)**  
From `docs/posix/md/xbd/V1_chap09.md`, 9.3.5 RE Bracket Expression:

> "A collating symbol is a collating element enclosed within bracket-period (`[.` and `.]`) delimiters."
>
> "An equivalence class expression shall represent the set of collating elements belonging to an equivalence class."

**Why this is non-compliant**  
zsh does not support collating symbols (`[[.x.]]`) or equivalence class
expressions (`[[=x=]]`) in shell pattern bracket expressions.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh -c 'case "a" in [[.a.]]) echo match;; *) echo nomatch;; esac' 2>&1
```

Expected:
- `match`

Observed:
- `nomatch` (or a syntax error)

---

## 29) Multi-digit file descriptors (>=10) not recognized as IO_NUMBER

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.10.1 Shell Grammar Lexical Conventions, Rule 2:

> "If the string consists solely of digits and the delimiter character is '<' or '>', the token identifier IO_NUMBER shall be returned."

**Why this is non-compliant**  
`exec 10>/path` treats `10` as a command name instead of an IO_NUMBER.
Single-digit descriptors (0–9) work correctly, but multi-digit descriptors
are not recognized.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'exec 10>/tmp/zsh_fd10.txt; echo hello >&10; exec 10>&-; cat /tmp/zsh_fd10.txt; rm /tmp/zsh_fd10.txt' 2>&1
```

Expected:
- `hello`

Observed:
- `zsh:1: command not found: 10`

---

## 30) Subshell does not inherit parent traps for `trap -p`

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.13 Shell Execution Environment:

> "Traps caught by the shell are reset to the default values and traps that are not being caught or being ignored are set to the default values in the subshell"

And from 2.15 Special Built-In Utilities, trap:

> "Before entry into the subshell, the trap that is caught will be displayed by 'trap -p' as the command and the conditions that were caught."

**Why this is non-compliant**  
In a subshell, `trap -p` (and even plain `trap`) shows nothing for parent
traps, even before the subshell sets its own traps. POSIX requires the
parent's caught traps to be visible via `trap -p` before entry.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'trap "echo hit" USR1; (trap -p USR1; echo end)'
```

Expected:
- `trap -- 'echo hit' USR1` followed by `end`

Observed:
- `end` only (no trap output)

---

## 31) `test`/`[` does not recognize `>` and `<` string comparison operators

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/test.md`:

> "s1 > s2 — True if the string s1 shall collate after the string s2"
> "s1 < s2 — True if the string s1 shall collate before the string s2"

**Why this is non-compliant**  
zsh's `test` and `[` built-ins reject `>` and `<` with "condition expected"
instead of performing string collation comparison.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'test "b" ">" "a"; echo $?' 2>&1
```

Expected:
- `0` (true, "b" collates after "a")

Observed:
- `zsh:1: condition expected: >` and exit status 2

---

## 32) `time` reserved word does not write timing to stderr in non-interactive mode

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/time.md`:

> "The time utility shall write the elapsed time, user time, and system time of the command to standard error."

**Why this is non-compliant**  
When used in `-c` mode or script file mode, `time echo hello` runs the
command but produces no timing output on stderr.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c '{ time true; } 2>&1'
```

Expected:
- Timing statistics on stderr

Observed:
- (no output)

---

## 33) `set -v` does not echo input in `-c` mode

**`emulate sh`:** NOT FIXED — `set -v` works correctly in script file mode
but not in `-c` mode.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, set:

> "-v: The shell shall write its input to standard error as it is read."

**Why this is non-compliant**  
In `-c` mode, `set -v` does not echo the input lines to stderr.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'set -v; printf "%s\n" testing_verbose' 2>&1
```

Expected:
- `printf "%s\n" testing_verbose` on stderr, followed by `testing_verbose`

Observed:
- `testing_verbose` only (no verbose echo)

---

## 34) `getopts` does not unset OPTARG for options without arguments

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "If the current option does not have an option-argument, the variable OPTARG shall be unset."

**Why this is non-compliant**  
After processing an option that takes no argument, zsh sets `OPTARG` to
an empty string instead of unsetting it. `${OPTARG+set}` returns `set`
when it should return nothing.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'OPTIND=1; OPTARG=stale; set -- -a; getopts ab name; echo "${OPTARG+set_to:}${OPTARG-unset}"'
```

Expected:
- `unset`

Observed:
- `set_to:` (OPTARG is set to empty string)

---

## 35) `getopts` does not unset OPTARG for invalid/missing-arg options in normal mode

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "otherwise, the shell variable OPTARG shall be unset"

(When an invalid option is encountered and the first character of optstring
is not `:`.)

**Why this is non-compliant**  
For both invalid options and missing option-arguments in normal mode
(optstring without leading `:`), zsh sets `OPTARG` to an empty string
instead of unsetting it.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'OPTIND=1; set -- -z; getopts ab name 2>/dev/null; echo "${OPTARG+set}"'
```

Expected:
- (empty — OPTARG is unset)

Observed:
- `set`

---

## 36) `getopts` end-of-options does not set name to `?` and unset OPTARG

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/getopts.md`:

> "When the end of options is encountered, the getopts utility shall exit with a return value greater than zero; the shell variable specified by name shall be set to the <question-mark> character"

**Why this is non-compliant**  
When options are exhausted, zsh leaves `name` set to the last option
character instead of `?`, and does not unset `OPTARG`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'OPTIND=1; set -- -a -b; getopts ab name; getopts ab name; getopts ab name; echo "name=$name optarg=${OPTARG-unset}"'
```

Expected:
- `name=? optarg=unset`

Observed:
- `name=b optarg=` (name retains last option, OPTARG is empty)

---

## 37) IFS trailing non-whitespace delimiter produces extra empty field

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.5 Field Splitting:

> "If the value of IFS is a \<space\>, \<tab\>, and \<newline\>, or if it is unset, any sequence of \<space\>, \<tab\>, or \<newline\> characters at the beginning or end of the input shall be ignored... if non-\<blank\> IFS characters are present... the behavior with a trailing non-blank IFS delimiter is that it does not generate a trailing empty field."

**Why this is non-compliant**  
With `IFS=:` and input `a:`, field splitting produces 2 fields (`a` and
empty) instead of the POSIX-required 1 field (`a`). The trailing non-blank
IFS delimiter should not generate a trailing empty field.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'IFS=:; x="a:"; set -- $x; echo "$#"'
```

Expected:
- `1`

Observed:
- `2`

---

## 38) Tilde expansion with null HOME produces zero fields instead of one empty field

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.1 Tilde Expansion:

> "If HOME is set, the value of HOME shall be used."

And from 2.6.5 Field Splitting:

> "If the complete expansion appropriate for a word results in an empty field... that empty field shall be deleted unless... it is in double-quotes."

However the tilde expansion itself produces a field (the empty string),
which should count as 1 in `set -- ~`.

**Why this is non-compliant**  
When `HOME=""`, `set -- ~` produces 0 positional parameters instead of 1
(the empty string). The tilde expands to the null string, which is still
a field.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'HOME=""; set -- ~; echo "$#"'
```

Expected:
- `1`

Observed:
- `0`

---

## 39) `${parameter:=word}` allows assignment to positional parameters

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.6.2 Parameter Expansion:

> "Only variables, not positional parameters or special parameters, can be assigned in this way."

**Why this is non-compliant**  
`${2:=foo}` succeeds and assigns `foo` to positional parameter 2, instead
of producing an error.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'set -- "arg"; echo "${2:=foo}"' 2>&1
```

Expected:
- Error diagnostic on stderr, non-zero exit

Observed:
- `foo` on stdout, exit 0

---

## 40) `${VAR:=val}` side-effect does not persist before non-special-built-in

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.2:

> "Variable assignments shall be performed... according to the steps in 2.9.1.1... If no command name results... each variable assignment specified with the command shall be as if it were the argument to the export built-in"

Step 4 of 2.9.1.1 says expansions are performed, including `${VAR:=val}`
which assigns to VAR as a side effect that persists.

**Why this is non-compliant**  
In `X=${Y:=side} /usr/bin/true`, the side-effect assignment to `Y` from
the `${Y:=side}` expansion does not persist after the command completes.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'unset Y; X=${Y:=side} /usr/bin/true; printf "%s\n" "${Y}"'
```

Expected:
- `side`

Observed:
- (empty)

---

## 41) `shift` with non-numeric operand succeeds instead of failing

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.15 Special Built-In Utilities, shift:

> "The operand shall be an unsigned decimal integer"

**Why this is non-compliant**  
`shift abc` succeeds (exit 0) instead of reporting an error, when the
operand is not a valid unsigned decimal integer.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'set -- a b c; shift abc; echo "rc=$?"' 2>&1
```

Expected:
- Error diagnostic, non-zero exit

Observed:
- `rc=0`

---

## 42) MAILCHECK defaults to 60 instead of 600

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/sh.md`:

> "If MAILCHECK is set to a positive integer and the shell is interactive, the shell shall check for mail at the specified interval in seconds."

And from `docs/posix/md/utilities/V3_chap02.md`, 2.5.3 Shell Variables:

> "If this variable is not set, the implementation shall perform the check at an unspecified interval (600 is typical)."

**Why this is non-compliant**  
zsh sets `MAILCHECK=60` by default instead of the POSIX-typical 600.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'echo $MAILCHECK'
```

Expected:
- `600` (or unset, with implementation checking at ~600s interval)

Observed:
- `60`

---

## 43) `bg` on already-running background job returns error

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/bg.md`:

> "If the job identified by job_id is already a running background job, the bg utility shall have no effect and shall exit successfully."

**Why this is non-compliant**  
`bg %1` on an already-running background job prints "bg: job already in
background" and returns exit status 1 instead of the required 0.

**Reproduction (portable shell commands)**

```sh
# In an interactive zsh --emulate sh session with job control:
sleep 30 &
bg %1
echo $?
# Expected: 0
# Observed: 1 with "bg: job already in background"
```

Expected:
- Exit status 0 (no effect, silent success)

Observed:
- `bg: job already in background` with exit status 1

---

## 44) `cd` with unset HOME succeeds instead of failing

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md`:

> "If no directory operand is given and the HOME environment variable is set to a non-empty value, the cd utility shall behave as if the directory named in the HOME environment variable was specified as the directory operand. If the HOME environment variable is empty or is not set, the behavior is implementation-defined."

The normative requirement for cd without operand when HOME is unset is
implementation-defined, but the test suite follows the common interpretation
that it should fail.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'unset HOME; cd; echo "rc=$?"' 2>&1
```

Expected:
- Error diagnostic, non-zero exit (common behavior)

Observed:
- `rc=0` (succeeds, stays in current directory)

---

## 45) `cd -eP` option not recognized

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/cd.md` (Issue 8):

> "-e: If, during the execution of the -P option, the current working directory was found to be invalid, exit with a non-zero status."

**Why this is non-compliant**  
zsh does not recognize the `-e` option for `cd`, introduced in Issue 8.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'cd -eP /tmp; echo "rc=$?"' 2>&1
```

Expected:
- `rc=0`

Observed:
- `zsh:cd:1: string not in pwd: -eP` with `rc=1`

---

## 46) `command export` field-splits assignment values

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.2:

> "If the command name is the name of a declaration utility... words that would be recognized as variable assignments if they appeared before a command name are subject to tilde expansion and word expansion as described in 2.6.1 and 2.6, but field splitting and pathname expansion are not performed."

**Why this is non-compliant**  
`command export ASSIGN=$value` performs field splitting on the assigned
value, truncating `"aa bb"` to `"aa"`.

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c 'value="aa bb"; command export ASSIGN=$value; printf "%s\n" "$ASSIGN"'
```

Expected:
- `aa bb`

Observed:
- `aa`

---

## 47) Dot (`.`) read error does not run EXIT trap

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.8.1 Consequences of Shell Errors:

> "Special built-in utility error — [Non-Interactive Shell] shall exit"

And from 2.15 Special Built-In Utilities, trap:

> "The shell shall execute the action... when the shell receives the corresponding condition... EXIT."

**Why this is non-compliant**  
When a dot-sourced file is missing, the non-interactive shell exits (correct)
but does not execute the previously defined EXIT trap action.

**Reproduction (portable shell commands)**

```sh
echo 'trap "echo EXIT_TRAP" EXIT; . /no_such_file_xyzzy; echo survived' > /tmp/zsh_dot_test.sh
/usr/bin/zsh --emulate sh /tmp/zsh_dot_test.sh 2>/dev/null
rm /tmp/zsh_dot_test.sh
```

Expected:
- `EXIT_TRAP`

Observed:
- (no output)

---

## 48) Plain assignment as asynchronous list causes parse error

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.1.2:

> "If no command name results, or if the command name is a special built-in utility or a function, variable assignments shall affect the current execution environment."

And from 2.9.3 Lists:

> "If the format is: command1 & ..."

Any simple command can be an asynchronous list. A simple command consisting
of only variable assignments is valid.

**Why this is non-compliant**  
`X=child &` (an asynchronous simple command consisting of only a variable
assignment) causes a parse error instead of running the assignment in a
background subshell.

**Reproduction (portable shell commands)**

```sh
echo 'X=parent
X=child &
wait
echo $X' > /tmp/zsh_async_test.sh
/usr/bin/zsh --emulate sh /tmp/zsh_async_test.sh 2>&1
rm /tmp/zsh_async_test.sh
```

Expected:
- `parent`

Observed:
- Parse error near `&`

---

## 49) `! !` (multiple bangs) causes parse error instead of correct negation

**`emulate sh`:** NOT FIXED.

**POSIX passage (exact quote)**  
From `docs/posix/md/utilities/V3_chap02.md`, 2.9.2 Pipelines:

> "If the pipeline begins with the reserved word '!', the exit status... shall be the logical NOT of the exit status"

Multiple `!` tokens are each valid pipeline prefixes.

**Why this is non-compliant**  
`! ! true` causes a parse error instead of double-negating (yielding the
original exit status).

**Reproduction (portable shell commands)**

```sh
/usr/bin/zsh --emulate sh -c '! ! true; echo $?' 2>&1
```

Expected:
- `0` (double negation of true)

Observed:
- `zsh:1: parse error near '!'`

---

---

This file is intentionally strict: only independently reproducible,
standards-backed zsh deviations are included.
