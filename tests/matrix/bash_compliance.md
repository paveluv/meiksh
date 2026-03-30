# Bash POSIX Compliance Report

**Shell tested:** GNU bash 3.2.57(1)-release (arm64-apple-darwin24) — macOS `/bin/sh`  
**Also tested:** GNU bash 5.3.9(1)-release — `/opt/homebrew/bin/bash`  
**Standard:** POSIX.1-2024 (Issue 8)  
**Test suite:** `tests/matrix` (65 test files)  
**Date:** 2026-03-24

| Shell | Passed | Failed |
|-------|--------|--------|
| bash 3.2.57 (`/bin/sh`) | 59 | 6 |
| bash 5.3.9 | 57 | 8 |

Bash 5.3 fixes items 1–4 below but introduces new failures (items 7–12).
Items 5–6 (vi editing) persist in both versions.

---

## 1. `cd ""` does not produce an error

**Severity:** Low  
**POSIX reference:** `cd` utility, OPERANDS section  
**Bash 3.2:** Broken — **Bash 5.3:** Fixed

> If *directory* is an empty string, `cd` **shall** write a diagnostic
> message to standard error and exit with non-zero status.

```sh
/bin/sh -c 'cd ""; echo "exit=$?"'
```

**Expected:** diagnostic on stderr, non-zero exit  
**Actual (3.2):** `exit=0` (silently succeeds)

---

## 2. `command` not treated as declaration utility for tilde expansion

**Severity:** Medium  
**POSIX reference:** `command` utility, DESCRIPTION section (Issue 8)  
**Bash 3.2:** Broken — **Bash 5.3:** Fixed

> The `command` utility **shall** be treated as a declaration utility if the
> first argument passed to the utility is recognized as a declaration utility.

```sh
/bin/sh -c 'command export HOMEDIR=~; echo "$HOMEDIR"'
```

**Expected:** `/Users/<username>` (HOME expanded)  
**Actual (3.2):** `~` (literal tilde)

---

## 3. Case statement `;&` fallthrough not supported

**Severity:** Medium  
**POSIX reference:** Shell Command Language §2.9.4.3  
**Bash 3.2:** Broken — **Bash 5.3:** Fixed (since 4.0)

> If the case statement clause is terminated by `";&"`, then the
> compound-list of each subsequent clause **shall** be executed.

```sh
/bin/sh -c 'x=a; case "$x" in a) echo one ;& b) echo two ;; esac'
```

**Expected:** `one` then `two`  
**Actual (3.2):** Syntax error

---

## 4. `read` into a readonly variable returns exit status 0

**Severity:** Medium  
**POSIX reference:** `read` utility, DESCRIPTION section  
**Bash 3.2:** Broken — **Bash 5.3:** Fixed

> An error in setting any variable (such as if a *var* has previously been
> marked *readonly*) **shall** result in a return value greater than one.

```sh
echo 'val' | /bin/sh -c 'readonly x=locked; read x; echo "exit=$?"'
```

**Expected:** non-zero exit  
**Actual (3.2):** `exit=0`

---

## 5. Vi editing mode: `t`/`T` (find character) broken

**Severity:** Low  
**POSIX reference:** `sh` utility, Vi Line Editing section  
**Bash 3.2:** Broken — **Bash 5.3:** Broken

`tc` should move to the character *before* the first occurrence of `c`
after the cursor. `Tc` should move to the character *after* the first
occurrence of `c` before the cursor.

Both commands position the cursor incorrectly in bash, causing
replacements (`rZ`) to land on the wrong character.

---

## 6. Vi editing mode: `[count]~` (tilde case toggle) ignores count

**Severity:** Low  
**POSIX reference:** `sh` utility, Vi Line Editing section  
**Bash 3.2:** Broken — **Bash 5.3:** Broken

`9~` on "aB" should toggle both characters to produce "Ab". Bash only
toggles the first character, producing "AB".

---

## 7. `trap` with unsigned decimal integer does not reset to default

**Severity:** Medium  
**POSIX reference:** `trap` utility, DESCRIPTION (SHALL-DESCRIPTION-629)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

> If the `-p` option is not specified and the first operand is an unsigned
> decimal integer, the shell shall treat all operands as conditions and
> reset each condition to the default value.

```sh
bash --posix -c 'trap "echo trapped" INT; trap 2; trap -p INT'
```

**Expected:** empty output (`INT` was reset to default)  
**Actual (5.3):** `trap -- - INT` — bash treats `2` as an action string
rather than triggering the numeric-reset code path.

---

## 8. Subshell traps not reset to default

**Severity:** Medium  
**POSIX reference:** `trap` DESCRIPTION (SHALL-DESCRIPTION-640), §2.13 (SHALL-2-13-471)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

> When a subshell is entered, traps that are not being ignored shall be
> set to the default actions.

```sh
bash --posix -c 'trap "echo parent" USR1; (trap -p USR1)'
```

**Expected:** empty output (trap was reset in subshell)  
**Actual (5.3):** `trap -- 'echo parent' USR1` — bash exposes the
parent shell's trap inside the subshell via `trap -p`.

The same root cause makes `(trap)` (no operands) in a subshell list
parent traps, which also violates SHALL-2-13-471.

---

## 9. `echo` does not process XSI escape sequences

**Severity:** Medium  
**POSIX reference:** `echo` utility, OPERANDS (SHALL-OPERANDS-5003/5004)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

> The following character sequences shall be recognized within any of
> the arguments: `\a`, `\b`, `\c`, `\f`, `\n`, `\r`, `\t`, `\v`, `\\`, `\0num`

```sh
bash --posix -c 'echo "\a"' | od -An -tx1
```

**Expected:** `07 0a` (BEL + newline)  
**Actual (5.3):** `5c 61 0a` (literal `\a` + newline)

Bash requires `-e` or `shopt -s xpg_echo` to enable escape processing.
Even `--posix` mode does not activate XSI `echo` behavior.

---

## 10. Variable assignment before function call is temporary

**Severity:** Medium  
**POSIX reference:** §2.9.1 Simple Commands (SHALL-2-9-1-2-280)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

> If the command name is a function that is not a standard utility
> implemented as a function, variable assignments shall affect the
> current execution environment.

```sh
bash --posix -c '
f() { echo "func"; }
x="old"
x="new" f >/dev/null
echo "$x"
'
```

**Expected:** `new` (assignment persists after function returns)  
**Actual (5.3):** `old` — bash scopes prefix assignments to the
function call as temporary, reverting on return.

---

## 11. Tilde expansion result not protected from field splitting/globbing

**Severity:** Low  
**POSIX reference:** §2.6.1 Tilde Expansion (SHALL-2-6-1-117)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

> The pathname that replaces the tilde-prefix shall be treated as if
> quoted to prevent it being altered by field splitting and pathname
> expansion.

```sh
bash --posix -c 'HOME="home with * spaces"; printf "%s\n" ~'
```

**Expected:** single line `home with * spaces`  
**Actual (5.3):** multiple lines — `~` expansion is subject to field
splitting, breaking the value on spaces and potentially expanding `*`.

---

## 12. Tilde expansion does not reflect mid-script HOME reassignment

**Severity:** Low  
**POSIX reference:** §2.6.1 Tilde Expansion, XBD §8 (SHALL-XBD-8-3010)  
**Bash 3.2:** Fixed — **Bash 5.3:** Broken

```sh
bash --posix -c 'HOME=/tmp/newdir; echo ~'
```

**Expected:** `/tmp/newdir`  
**Actual (5.3):** original HOME value — bash performs tilde expansion at
parse time rather than during word expansion at execution time, so a
subsequent `HOME=...` assignment does not affect `~` that was already
parsed.

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
| 7 | `trap <number>` reset | `trap` DESCRIPTION | Medium | Fixed | Broken |
| 8 | Subshell trap reset | `trap` / §2.13 | Medium | Fixed | Broken |
| 9 | `echo` XSI escapes | `echo` OPERANDS | Medium | Fixed | Broken |
| 10 | Function prefix assignment | §2.9.1 | Medium | Fixed | Broken |
| 11 | Tilde field splitting | §2.6.1 | Low | Fixed | Broken |
| 12 | Tilde parse-time expansion | §2.6.1 / XBD §8 | Low | Fixed | Broken |

Items 1–4 are fixed in bash 5.3. Items 5–6 persist across both versions.
Items 7–12 are regressions or new non-compliances in bash 5.3.

The `interactive.sh` test also fails on bash 5.3 because bracketed paste
mode (`\e[?2004h`/`\e[?2004l`) pollutes PTY output. This is not a POSIX
non-compliance — it is a bash feature that interferes with the test harness.

Item 3 (`;&`) was added in POSIX Issue 8 (2024). All other items
represent deviations from requirements that existed in earlier POSIX
versions.
