# Bash POSIX Compliance Report

**Shell tested:** GNU bash 3.2.57(1)-release (arm64-apple-darwin24) — macOS `/bin/sh`  
**Also tested:** GNU bash 5.3.9(1)-release — `/opt/homebrew/bin/bash`  
**Standard:** POSIX.1-2024 (Issue 8)  
**Test suite:** `tests/matrix` (64 test files)  
**Date:** 2026-03-24

| Shell | Passed | Failed |
|-------|--------|--------|
| bash 3.2.57 (`/bin/sh`) | 58 | 6 |
| bash 5.3.9 | 46 | 18 |

Bash 5.3 fixes all four non-vi items listed below (1–4) but introduces new
failures in other areas (alias expansion in non-interactive mode, `export -p`
/ `readonly -p` output format, `echo` escape sequences, and others). Those
additional bash 5.3 regressions are not covered in this report.

This report documents every case where bash (as shipped with macOS) fails to
conform to the POSIX.1-2024 Shell & Utilities specification. Each item
includes the normative text from the standard, a standalone reproduction
command, and the expected vs actual behavior.

---

## 1. `cd ""` does not produce an error

**Severity:** Low  
**POSIX reference:** `cd` utility, OPERANDS section

### Standard says

> If *directory* is an empty string, `cd` **shall** write a diagnostic
> message to standard error and exit with non-zero status.

This requirement was added by Austin Group Defect 1047.

### Reproduction

```sh
/bin/sh -c 'cd ""; echo "exit=$?"'
```

**Expected:** diagnostic on stderr, non-zero exit  
**Actual:** `exit=0` (silently succeeds, cd to `$HOME` or no-op depending on context)

**Bash 5.3:** Fixed. Returns `exit=1` with diagnostic `/opt/homebrew/bin/bash: line 1: cd: null directory`.

---

## 2. `command` not treated as declaration utility for tilde expansion

**Severity:** Medium  
**POSIX reference:** `command` utility, DESCRIPTION section (Issue 8)

### Standard says

> The `command` utility **shall** be treated as a declaration utility if the
> first argument passed to the utility is recognized as a declaration
> utility. In this case, subsequent words of the form *name*=*word* **shall**
> be expanded in an assignment context.

This requirement was added by Austin Group Defects 351 and 1393. It means
that `command export VAR=~` must perform tilde expansion just like bare
`export VAR=~` does.

Note: bare `export HOMEDIR=~` works correctly in bash 3.2 — the tilde is
expanded. The failure is specifically when `command` wraps `export`.

### Reproduction

```sh
/bin/sh -c 'command export HOMEDIR=~; echo "$HOMEDIR"'
```

**Expected:** `/Users/<username>` (the value of `$HOME`)  
**Actual:** `~` (literal tilde, unexpanded)

Both dash and ksh handle this correctly. Bash 3.2 does not recognize
`command export` as a declaration context.

**Bash 5.3:** Fixed. `command export HOMEDIR=~` correctly expands tilde.

---

## 3. Case statement `;&` fallthrough not supported

**Severity:** Medium  
**POSIX reference:** Shell Command Language §2.9.4.3 *Case Conditional Construct*

### Standard says

> Each case statement clause, with the possible exception of the last,
> **shall** be terminated with either `";;"` or `";&"`.
>
> If the case statement clause is terminated by `";&"`, then the
> compound-list (if any) of each subsequent clause **shall** be executed,
> in order, until either a clause terminated by `";;"` is reached and its
> compound-list (if any) executed or there are no further clauses in the
> case statement.

The `;&` terminator was added in POSIX Issue 8 (2024).

### Reproduction

```sh
/bin/sh -c '
x=a
case "$x" in
    a) echo one ;&
    b) echo two ;&
    c) echo three ;;
esac
'
```

**Expected output:**
```
one
two
three
```

**Actual:** Syntax error — bash 3.2 does not recognize `;&` as a valid case
terminator.

**Bash 5.3:** Fixed. `;&` fallthrough works correctly (supported since bash 4.0).

---

## 4. `read` into a readonly variable returns exit status 0

**Severity:** Medium  
**POSIX reference:** `read` utility, DESCRIPTION section

### Standard says

> An error in setting any variable (such as if a *var* has previously been
> marked *readonly*) **shall** be considered an error of `read` processing,
> and **shall** result in a return value greater than one. Variables named
> before the one generating the error shall be set as described above; it is
> unspecified whether variables named later shall be set as above, or `read`
> simply ceases processing when the error occurs, leaving later named
> variables unaltered.

### Reproduction

```sh
echo 'val' | /bin/sh -c 'readonly x=locked; read x; echo "exit=$?"'
```

**Expected:** diagnostic on stderr, `exit=2` (or any value > 1)  
**Actual:** `/bin/sh: x: readonly variable` on stderr, but `exit=0`

Bash writes the diagnostic but returns 0 instead of > 1.

A second form with partial assignment:

```sh
echo 'a b' | /bin/sh -c 'readonly second=locked; read first second; echo "exit=$?"'
```

**Expected:** non-zero exit  
**Actual:** `exit=0`

**Bash 5.3:** Fixed. Both forms return `exit=1`.

---

## 5. Vi editing mode: `t`/`T` (find character) broken

**Severity:** Low — affects interactive editing only  
**POSIX reference:** `sh` utility, Vi Line Editing section

### Standard says

> `[count]tc`
>
> Move to the character before the first occurrence of the character 'c'
> that occurs after the current cursor position.
>
> `[count]Tc`
>
> Move to the character after the first occurrence of the character 'c'
> that occurs before the current cursor position.

### Reproduction

For `tc`:

```sh
/bin/sh -i
# Type: set -o vi<Enter>
# Type: echo abc
# Press: Escape
# Type: 0 (go to start of line)
# Type: tc (move to character before 'c')
# Type: rZ (replace with Z)
# Press: Enter
```

**Expected output:** `echo aZc` (Z replaces 'b', the character before 'c')  
**Actual:** `Zcho abc` — the cursor lands at position 0 instead of before
'c', so the replacement hits the wrong character.

For `Ta`:

```sh
/bin/sh -i
# Type: set -o vi<Enter>
# Type: echo abc
# Press: Escape (cursor on 'c')
# Type: Ta (move to character after 'a')
# Type: rZ (replace with Z)
# Press: Enter
```

**Expected output:** `echo aZc` (Z replaces 'b', the character after 'a')  
**Actual:** `echo abcZc` — bash appends rather than replacing in place,
indicating `T` does not position the cursor correctly.

**Bash 5.3:** Still broken. Same failures for both `t` and `T`.

---

## 6. Vi editing mode: `[count]~` (tilde case toggle with count) broken

**Severity:** Low — affects interactive editing only  
**POSIX reference:** `sh` utility, Vi Line Editing section

### Standard says

> `[count]~`
>
> Convert, if the current character is a lowercase letter, to the equivalent
> uppercase letter and vice versa [...] The current cursor position then
> **shall** be advanced by one character.
>
> If the count is larger than the number of characters after the cursor,
> this **shall not** be considered an error; the cursor shall advance to the
> last character on the line.

Note: the basic `~` command (without a count prefix) works correctly.
The failure is specifically when a numeric count is provided.

### Reproduction

```sh
/bin/sh -i
# Type: set -o vi<Enter>
# Type: echo aB
# Press: Escape
# Type: 0w (move to start of 'aB')
# Type: 9~ (toggle case of up to 9 characters — only 2 remain)
# Press: Enter
```

**Expected output:** `Ab` (both characters toggled: `a`→`A`, `B`→`b`)  
**Actual:** `AB` — only the first character (`a`→`A`) is toggled. The
count is not applied; `~` processes a single character and stops. The
final character `B` remains uppercase instead of being toggled to `b`.

**Bash 5.3:** Still broken. Same behavior — `9~` toggles only one character.

---

## Summary

| # | Area | POSIX Section | Impact | Bash 3.2 | Bash 5.3 |
|---|------|--------------|--------|----------|----------|
| 1 | `cd ""` error handling | `cd` OPERANDS | Low | Broken | Fixed |
| 2 | `command` declaration utility | `command` DESCRIPTION | Medium | Broken | Fixed |
| 3 | `;&` case fallthrough | §2.9.4.3 | Medium | Broken | Fixed (since 4.0) |
| 4 | `read` readonly exit status | `read` DESCRIPTION | Medium | Broken | Fixed |
| 5 | Vi editing: `t`/`T` | `sh` Vi Editing | Low | Broken | Broken |
| 6 | Vi editing: `[count]~` | `sh` Vi Editing | Low | Broken | Broken |

Items 1–4 are semantic non-compliances in the shell language or built-in
utilities. All four are fixed in bash 5.3. Items 5–6 are vi line editing
deficiencies that persist through bash 5.3 and do not affect script
execution.

Item 3 (`;&`) was added in POSIX Issue 8 (2024) and was not part of earlier
POSIX versions. Bash 4.0+ supports `;&` (added in 2009). All other items
represent deviations from requirements that existed in earlier POSIX
versions as well.
