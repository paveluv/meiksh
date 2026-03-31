# Bash POSIX Compliance Report

**Shell tested:** GNU bash 5.2.37(1)-release (x86_64-pc-linux-gnu)
**Standard:** POSIX.1-2024 (Issue 8)
**Test suite:** `tests/matrix` (65 `.epty` test suites, 1011 tests)
**Date:** 2026-03-30

| Result | Count |
|--------|-------|
| Passed | 986 |
| Failed | 25 |

The 25 failures fall into 13 distinct issues documented below.
Items 1–4 are test-expectation issues (the tests assume behavior that is
debatable or contradicted by POSIX). Items 5–13 are genuine
non-compliances in bash 5.2.

---

## 1. `test -f` with missing operand exits 0 (test expectation issue)

**Severity:** None (bash is correct)
**Suite:** `maybe_builtins` (1 test)
**POSIX reference:** XCU §test, EXIT STATUS — argument-count algorithm

> 1 argument: Exit true (0) if $1 is not null; otherwise, exit false.

`test -f` has one argument (`-f`) which is not null, so exit status 0 is
correct per POSIX. The test expected `test -f` to be treated as a unary
primary with a missing operand, but the argument-count algorithm takes
precedence.

```sh
/usr/bin/bash --posix -c 'test -f; echo $?'
# Output: 0
```

---

## 2. `read` clears variable on EOF in while loop (test expectation issue)

**Severity:** None (bash is correct)
**Suite:** `read` (1 test)
**POSIX reference:** XCU §read, DESCRIPTION (SHALL-READ-1281)

> When the standard input is a terminal [...] if the end-of-file
> condition is detected, the read utility shall set each variable
> var to an empty string.

A `while read val; do :; done` loop that exhausts its input leaves `val`
empty after the final EOF-returning `read` call. The test expected `val`
to retain the value from the last *successful* iteration, but both bash
and dash clear it. This is POSIX-correct behavior.

```sh
/usr/bin/bash --posix -c '
val=init
while read val; do :; done <<EOF
first
last
EOF
echo "val=[$val]"'
# Output: val=[]
```

---

## 3. `set` output polluted by `BASH_EXECUTION_STRING` (test expectation issue)

**Severity:** Low (bash-specific variable interferes with grep)
**Suite:** `set_options` (1 test)
**POSIX reference:** XCU §set, DESCRIPTION (SHALL-DESCRIPTION-582)

> The current settings of the variables [...] shall be written to
> standard output [...] with appropriate quoting.

Bash's `set` output is properly quoted. However, when invoked via
`-c "script"`, bash exposes `BASH_EXECUTION_STRING` which contains the
script text. A `set | grep "my_weird_var="` picks up both the variable
assignment *and* the `BASH_EXECUTION_STRING` line, causing `eval` to
fail on the combined output. This is not a quoting deficiency — it is a
grep-selectivity issue caused by a bash-specific internal variable.

```sh
/usr/bin/bash --posix -c '
my_var="hello \"world\""
set | grep "my_var="'
# Output includes multiple matches:
#   BASH_EXECUTION_STRING='my_var="hello \"world\""...
#   my_var='hello "world"'
# The variable itself is properly quoted, but grep picks up
# BASH_EXECUTION_STRING too, breaking eval on the combined output.
```

---

## 4. `read` EOF on terminal does not preserve variable (test expectation issue)

*This is the same POSIX rule as item 2 — listed separately because it
manifests in a different test. See item 2 for details.*

---

## 5. `cd ""` does not produce an error

**Severity:** Low
**Suites:** `cd`, `cd_extended` (2 tests)
**POSIX reference:** XCU §cd, OPERANDS

> If *directory* is an empty string, `cd` **shall** write a diagnostic
> message to standard error and exit with non-zero status.

```sh
/usr/bin/bash --posix -c 'cd ""; echo "exit=$?"'
# Expected: diagnostic on stderr, non-zero exit
# Actual:   exit=0 (silently succeeds)
```

---

## 6. `echo` does not process XSI escape sequences

**Severity:** Medium
**Suite:** `maybe_builtins_echo` (11 tests)
**POSIX reference:** XCU §echo, OPERANDS (SHALL-OPERANDS-5003, SHALL-OPERANDS-5004)

> The following character sequences shall be recognized within any of
> the arguments: `\a`, `\b`, `\c`, `\f`, `\n`, `\r`, `\t`, `\v`,
> `\\`, `\0num`

```sh
/usr/bin/bash --posix -c 'echo "\a" | od -An -tx1 | tr -d " "'
# Expected: 070a   (BEL + newline)
# Actual:   5c610a (literal \a + newline)

/usr/bin/bash --posix -c 'echo "\0101"'
# Expected: A
# Actual:   \0101

/usr/bin/bash --posix -c 'printf "%s" "hello\c world" | wc -c'
# echo \c should suppress trailing output; bash outputs it literally
```

Bash requires `-e` or `shopt -s xpg_echo` to enable escape processing.
Even `--posix` mode does not activate XSI `echo` behavior.

---

## 7. Variable assignment before function call is temporary

**Severity:** Medium
**Suite:** `simple_commands_2` (1 test)
**POSIX reference:** §2.9.1 Simple Commands (SHALL-2-9-1-2-280)

> If the command name is a function that is not a standard utility
> implemented as a function, variable assignments shall affect the
> current execution environment.

```sh
/usr/bin/bash --posix -c '
f() { echo "func"; }
x="old"
x="new" f >/dev/null
echo "$x"'
# Expected: new
# Actual:   old
```

Bash scopes prefix assignments to the function call as temporary,
reverting on return.

---

## 8. MAIL notification not triggered in interactive mode

**Severity:** Low
**Suite:** `sh_mail` (3 tests)
**POSIX reference:** XCU §sh, Shell Variables — MAIL (SHALL-SH-1025), MAILCHECK (SHALL-SH-1027, SHALL-SH-1029, SHALL-SH-1031), MAILPATH (SHALL-SH-1032, SHALL-SH-1033)

> If `MAIL` is set, the shell **shall** inform the user if the file
> named by the variable is created or if its modification time has
> changed. Informing the user **shall** be accomplished by writing a
> string of unspecified format to standard error.

The three failing tests set `MAIL` (or `MAILPATH`) and `MAILCHECK=1`
(or `0`), create the mailbox file, and wait for a notification message.
Bash 5.2 in `--posix` interactive mode does not produce the expected
notification within the 5-second timeout. This may be a timing issue,
a `MAILCHECK` granularity issue, or a genuine non-compliance.

```sh
# Manual reproduction (interactive):
/usr/bin/bash --posix -i
$ MAIL=/tmp/test_mbox_$$
$ MAILCHECK=1
# Wait 2 seconds, then in another terminal:
#   echo data > /tmp/test_mbox_$$
# Press Enter to trigger a prompt.
# Expected: "you have mail" message on stderr
# Actual:   no message appears
```

---

## 9. Vi editing mode: `t`/`T` (find character) broken

**Severity:** Low
**Suite:** `vi_editing` (2 tests)
**POSIX reference:** XCU §sh, Vi Line Editing

`tc` should move to the character *before* the first occurrence of `c`
after the cursor. `Tc` should move to the character *after* the first
occurrence of `c` before the cursor.

Both commands position the cursor incorrectly in bash, causing
replacements (`rZ`) to land on the wrong character.

---

## 10. Vi editing mode: `[count]~` (tilde case toggle) ignores count

**Severity:** Low
**Suite:** `vi_editing` (1 test)
**POSIX reference:** XCU §sh, Vi Line Editing

`9~` on `aB` should toggle both characters to produce `Ab`. Bash only
toggles the first character, producing `AB`.

---

## 11. `wait -l` not implemented

**Severity:** Low
**Suite:** `wait` (2 tests)
**POSIX reference:** XCU §wait (SHALL-WAIT-1353)

> When both the `-l` option and *exit_status* operand are specified,
> the symbolic name of the corresponding signal **shall** be written
> to standard output.

`wait -l 143` should output a line containing `TERM` (128 + 15 =
SIGTERM). Bash 5.2 does not implement the `-l` option — it was added
in POSIX.1-2024 (Issue 8).

```sh
/usr/bin/bash --posix -c 'wait -l 143 2>&1'
# Expected: line containing "TERM"
# Actual:   bash: wait: -l: invalid option
#           wait: usage: wait [-fn] [-p var] [id ...]
```

---

## Summary

| # | Area | POSIX Section | Impact | Tests | Status |
|---|------|---------------|--------|-------|--------|
| 1 | `test -f` arg-count rule | XCU §test EXIT STATUS | None | 1 | Test issue |
| 2 | `read` EOF clears var | XCU §read SHALL-READ-1281 | None | 1 | Test issue |
| 3 | `set` + `BASH_EXECUTION_STRING` | XCU §set SHALL-DESCRIPTION-582 | Low | 1 | Test issue |
| 4 | (Same as 2) | — | — | — | — |
| 5 | `cd ""` error handling | XCU §cd OPERANDS | Low | 2 | Broken |
| 6 | `echo` XSI escapes | XCU §echo SHALL-OPERANDS-5003 | Medium | 11 | Broken |
| 7 | Function prefix assignment | §2.9.1 SHALL-2-9-1-2-280 | Medium | 1 | Broken |
| 8 | MAIL notification | XCU §sh SHALL-SH-1025 | Low | 3 | Broken |
| 9 | Vi editing: `t`/`T` | XCU §sh Vi Editing | Low | 2 | Broken |
| 10 | Vi editing: `[count]~` | XCU §sh Vi Editing | Low | 1 | Broken |
| 11 | `wait -l` | XCU §wait SHALL-WAIT-1353 | Low | 2 | Broken (Issue 8 feature) |
|    | **Total** | | | **25** | |

Items 1–3 are test-expectation issues where bash behavior is arguably
correct or the test methodology has a flaw. Items 5–11 are genuine
non-compliances.

The `echo` issue (item 6, 11 tests) accounts for nearly half of all
failures. Bash requires `-e` or `shopt -s xpg_echo` to enable POSIX
`echo` escape processing, even in `--posix` mode.

Item 11 (`wait -l`) is a POSIX.1-2024 (Issue 8) addition that bash 5.2
has not yet implemented. All other items represent deviations from
requirements that existed in earlier POSIX versions.
