# Bash POSIX Compliance Report

**Shell tested:** GNU bash 5.2.37(1)-release (x86_64-pc-linux-gnu)
**Standard:** POSIX.1-2024 (Issue 8)
**Test suite:** `tests/matrix` (65 `.epty` test suites, 1011 tests)
**Date:** 2026-03-30

| Result | Count |
|--------|-------|
| Passed | 992 |
| Failed | 19 |

The 19 failures fall into 6 distinct issues documented below.
All are genuine non-compliances in bash 5.2.

---

## 1. `cd ""` does not produce an error

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

## 2. `echo` does not process XSI escape sequences

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

## 3. Variable assignment before function call is temporary

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

## 4. Vi editing mode: `t`/`T` (find character) broken

**Severity:** Low
**Suite:** `vi_editing` (2 tests)
**POSIX reference:** XCU §sh, Vi Line Editing (Command Mode)

> `[count]tc` — Move the cursor to the character **before** the first
> occurrence of `c` that has not already been found [...] after the
> cursor.
>
> `[count]Tc` — Move the cursor to the character **after** the first
> occurrence of `c` that has not already been found [...] before the
> cursor.

Both `t` and `T` position the cursor on the target character itself
(like `f`/`F`) rather than one character before/after it.

**Reproduction** (interactive — run `/usr/bin/bash --posix -i`):

```
$ set -o vi
$ echo abc        # type "echo abc", then:
                   # ESC  0  tc  rZ  Enter
```

Step by step: type `echo abc`, press **Escape**, press **0** (go to
`e`), press **t** then **c** (should move cursor to `b`, one *before*
`c`), press **r** then **Z** (replace character under cursor with `Z`).

Expected output: `aZc` (`b` was replaced).
Actual output: bash runs `Zcho abc` — cursor landed on `c` (the target
itself, not one before it), so `rZ` replaced `c` and the line became
`echo abZ`, then the `0` repositioned and the replacement hit the wrong
spot.

The same issue affects `T`: with cursor at end of `echo abc`, pressing
**T** then **a** should move to `b` (one *after* `a`), but bash moves
to `a` itself.

---

## 5. Vi editing mode: `[count]~` (tilde case toggle) ignores count

**Severity:** Low
**Suite:** `vi_editing` (1 test)
**POSIX reference:** XCU §sh, Vi Line Editing (Command Mode)

> `[count]~` — If the count is larger than the number of characters
> after the cursor, this shall not be considered an error; the cursor
> shall advance to the last character on the line.

**Reproduction** (interactive — run `/usr/bin/bash --posix -i`):

```
$ set -o vi
$ echo aB          # type "echo aB", then:
                    # ESC  0w  9~  Enter
```

Step by step: type `echo aB`, press **Escape**, press **0** then **w**
(cursor on `a`), press **9** then **~** (toggle case of up to 9
characters — only 2 remain: `aB`).

Expected output: `Ab` (both characters toggled).
Actual output: `AB` — bash ignores the count and toggles only the
character under the cursor (`a` → `A`), advancing by one. The `B`
is untouched.

---

## 6. `wait -l` not implemented

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
| 1 | `cd ""` error handling | XCU §cd OPERANDS | Low | 2 | Broken |
| 2 | `echo` XSI escapes | XCU §echo SHALL-OPERANDS-5003 | Medium | 11 | Broken |
| 3 | Function prefix assignment | §2.9.1 SHALL-2-9-1-2-280 | Medium | 1 | Broken |
| 4 | Vi editing: `t`/`T` | XCU §sh Vi Editing | Low | 2 | Broken |
| 5 | Vi editing: `[count]~` | XCU §sh Vi Editing | Low | 1 | Broken |
| 6 | `wait -l` | XCU §wait SHALL-WAIT-1353 | Low | 2 | Broken (Issue 8 feature) |
|   | **Total** | | | **19** | |

The `echo` issue (item 2, 11 tests) accounts for over half of all
failures. Bash requires `-e` or `shopt -s xpg_echo` to enable POSIX
`echo` escape processing, even in `--posix` mode.

Item 6 (`wait -l`) is a POSIX.1-2024 (Issue 8) addition that bash 5.2
has not yet implemented. All other items represent deviations from
requirements that existed in earlier POSIX versions.
