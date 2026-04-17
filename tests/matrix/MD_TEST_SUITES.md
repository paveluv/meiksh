# Markdown Test Suites

This document describes how to create and improve `.md` test suites for
`expect_pty`, the POSIX shell conformance test runner.

## What gets its own test suite?

Each test suite covers one logical piece of the POSIX standard. The three
categories, with their naming conventions, are:

| Category | Source document | File name pattern | Example |
|----------|---------------|-------------------|---------|
| Shell language section | `docs/posix/md/utilities/V3_chap02.md` | `2_<section>.md` | `2_6_word_expansions.md` |
| Utility spec page | `docs/posix/md/utilities/<utility>.md` | see below | `utility_sh.md` |
| Base Definitions section | `docs/posix/md/basedefs/V1_chap<NN>.md` | `xbd_<section>.md` | `xbd_8_environment_variables.md` |

### Utility naming

Utilities are classified by how the standard categorises them:

| Classification | File name pattern | Example |
|----------------|-------------------|---------|
| Special built-in (2.15) | `builtin_<name>.md` | `builtin_set.md` |
| Intrinsic utility (1.7) | `intrinsic_utility_<name>.md` | `intrinsic_utility_cd.md` |
| Commonly-builtin utility | `maybe_builtin_<name>.md` | `maybe_builtin_echo.md` |
| Other utility | `utility_<name>.md` | `utility_sh.md`, `utility_time.md` |

## Prerequisites

- The POSIX standard text lives under `docs/posix/md/`. Key files:
  - `docs/posix/md/utilities/V3_chap02.md` — Shell Command Language
  - `docs/posix/md/utilities/<utility>.md` — individual utility spec pages
  - `docs/posix/md/basedefs/V1_chap<NN>.md` — Base Definitions chapters
- For known `bash --posix` non-compliances, see `tests/matrix/bash_compliance.md`.
- Test suites go in `tests/matrix/tests/` and must have a `.md` extension.

## File Structure

Every `.md` test suite follows this exact layout:

    # Test Suite for <Title>

    Brief paragraph describing what is being tested and what it covers.

    ## Table of contents

    - [Section Name](#section-name)
    - [Subsection Name](#subsection-name)
    - ...

    ## Section Name

    <verbatim text of the section from docs/posix/md — stop before the first
    subsection heading>

    ### Tests

    #### Test: test name here

    A brief, plain-English explanation of what this test verifies and why.
    Do not use blockquotes to repeat the standard verbatim here; instead,
    conveniently paraphrase or explain the rule being tested so a random reader
    can easily follow along.

    ```
    begin test "test name here"
      script
        <shell commands>
      expect
        stdout "<pattern>"
        stderr "<pattern>"
        exit_code <expr>
    end test "test name here"
    ```

    #### Test: another test

    ...more tests...

    ## Subsection Name

    <verbatim text of the subsection from docs/posix/md>

    ### Tests

    #### Test: subsection test

    ...tests for this subsection...

### Key rules

1. **Section headings** use `##` (level 2). Subsection headings also use `##`.
2. **"Tests" headings** use `###` (level 3), always literally `### Tests`.
3. **Test headings** use `####` (level 4), always in the form `#### Test: <name>`.
4. The test name after `#### Test: ` must **exactly match** the name in the
   `begin test "..."` and `end test "..."` lines inside the code block.
5. Each `#### Test:` section must contain **exactly one** fenced code block
   (`` ``` ``). The code block must not contain triple backticks internally.
6. **Standard text** is copied verbatim from the relevant `docs/posix/md/` file.
   Code blocks in the standard text (outside `#### Test:` sections) are ignored
   by the parser and are safe to include.
7. Markdown text between the `#### Test:` heading and the code block (the
   description) must not contain any headings (lines starting with `#`).
8. Everything outside `#### Test:` sections is treated as documentation and
   ignored by the test runner.

### Source text for each category

| Category | Copy verbatim from |
|----------|--------------------|
| Shell language section | `docs/posix/md/utilities/V3_chap02.md` |
| Utility spec page | `docs/posix/md/utilities/<utility>.md` |
| Base Definitions section | The relevant `docs/posix/md/basedefs/V1_chap<NN>.md` |

For utility spec pages, the standard text is structured differently from
chapter 2. A utility page has sections like NAME, SYNOPSIS, DESCRIPTION,
OPTIONS, OPERANDS, STDIN, INPUT FILES, ENVIRONMENT VARIABLES, STDOUT,
STDERR, EXIT STATUS, etc. Include the sections that are relevant to the
tests being written. The most useful sections are usually DESCRIPTION,
OPTIONS, OPERANDS, STDOUT, STDERR, and EXIT STATUS.

### Hyperlinks in the standard text

If a hyperlink in the standard text needs to be followed, links are
root-relative from the project directory (e.g.
`docs/posix/md/basedefs/V1_chap03.md`). Links that begin with `#` are
internal to `docs/posix/md/utilities/V3_chap02.md`.

## Test Block Format

Inside each fenced code block, the test uses the `.epty` DSL:

### Non-interactive tests

```
begin test "descriptive test name"
  script
    <shell script lines, indented by 4 spaces from column 0>
  expect
    stdout "<regex pattern>"
    stderr "<regex pattern>"
    exit_code <expression>
end test "descriptive test name"
```

- The `script` keyword must be at column 2 (2-space indent from block start).
- Script body lines must be at column 4 (4-space indent from block start).
- The `expect` keyword must be at column 2.
- Assertions must be at column 4, in **exactly this order**: `stdout`, `stderr`,
  `exit_code`. All three are **required**.
- The `begin test` / `end test` lines are at column 0.

### Environment variables (`setenv`)

A test can set environment variables that will be present when the shell
process is spawned, using `setenv` directives between `begin test` and
`script`/`spawn`:

```
begin test "bracket expression in custom locale"
  setenv "LC_ALL" "C.UTF-8"
  script
    case "a" in [[:alpha:]]) echo match;; esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "bracket expression in custom locale"
```

- `setenv` takes two double-quoted arguments: the variable name and the value.
- Quoted strings in `setenv` support backslash escapes: `\"`, `\\`, `\n`,
  `\r`, `\t`.
- Multiple `setenv` directives are allowed per test.
- `setenv` works in both non-interactive and interactive tests.

### Interactive tests

```
begin interactive test "descriptive test name"
  spawn -i
  expect "$ "
  send "echo hello"
  expect "hello"
  expect "$ "
  sendeof
  wait
end interactive test "descriptive test name"
```

Interactive tests use PTY commands (`spawn`, `expect`, `send`, `sendraw`,
`sendeof`, `wait`, `sleep`, etc.) instead of `script`/`expect` blocks.

Key interactive commands:

| Command | Description |
|---------|-------------|
| `spawn -i` | Start the shell in interactive mode |
| `send "text"` | Send text followed by a newline |
| `sendraw <hex bytes>` | Send raw bytes (space-separated hex, e.g. `1b` for ESC) |
| `sendeof` | Send EOF (Ctrl-D) |
| `expect "pattern"` | Wait for pattern to appear in output (0.2s default timeout) |
| `expect timeout=<N>s "pattern"` | Wait with custom timeout |
| `sleep <N>ms` | Pause for N milliseconds |
| `wait` | Wait for the shell process to exit |

## Pattern Syntax (stdout/stderr)

Patterns are enclosed in double-quotes. The quoting rules for patterns are
**different from** the quoting rules for `send`/`setenv` strings:

### Pattern quoting

- **Backslash passes through verbatim** to the regex engine. There is no
  escape processing at the quoting layer. What you write between the quotes
  is exactly what the regex engine receives.
- To embed a **literal double-quote** in a pattern, use `""` (doubled quote).
  Example: `stdout """"` matches a single `"` character.

### Regex features

The built-in regex engine supports:

| Syntax      | Meaning                                            |
|-------------|----------------------------------------------------|
| `.`         | Any character **except** `\n`                      |
| `*`         | Zero or more of preceding element (greedy)         |
| `+`         | One or more of preceding element (greedy)          |
| `?`         | Zero or one of preceding element (greedy)          |
| `\|`        | Alternation                                        |
| `(...)`     | Grouping                                           |
| `[abc]`     | Character class                                    |
| `[a-z]`     | Character range                                    |
| `[^abc]`    | Negated character class                            |
| `\n`        | Literal newline                                    |
| `\t`        | Literal tab                                        |
| `\r`        | Literal carriage return                             |
| `\X`        | Literal character `X` (for any other `X`)          |

### Matching behavior

- **stdout and stderr patterns are full-match** (anchored at both ends).
  The pattern must match the **entire** output, not just a substring.
- Output is **trimmed** of trailing whitespace before matching.
- `.` does **not** match `\n`. To match across lines, use `(.|\n)*` or
  `(.|\n)+`.

### Common patterns

| Pattern              | Matches                                     |
|----------------------|---------------------------------------------|
| `""`                 | Empty output (no stdout/stderr)             |
| `".+"`               | Any non-empty single-line output            |
| `"(.\|\n)+"`         | Any non-empty output (including multiline)  |
| `"(.\|\n)*"`         | Any output (including empty and multiline)  |
| `"hello world"`      | Exactly `hello world`                       |
| `"hello\nworld"`     | Two lines: `hello` and `world`              |
| `".*foo.*"`          | Any single line containing `foo`            |
| `"\$VAR"`            | Literal `$VAR` (`\` escapes `$` for regex)  |
| `"\(parens\)"`       | Literal `(parens)` (`\` escapes for regex)  |
| `"line1\n.*"`        | `line1` followed by any second line         |

### Characters that need regex escaping

These regex metacharacters must be backslash-escaped when you want them literal:

```
.  *  +  ?  |  (  )  [  ]  ^  $  \  {  }
```

Since backslash passes through verbatim to the regex engine, write `\(` in the
pattern to match a literal `(`. For example:

- `stdout "\(hello\)"` — matches the literal string `(hello)`
- `stdout "\$foo"` — matches the literal string `$foo`
- `stdout "a\*b"` — matches the literal string `a*b`

## Exit Code Expressions

The `exit_code` field supports an expression language:

| Expression          | Meaning                                     |
|---------------------|---------------------------------------------|
| `0`                 | Exactly 0                                   |
| `!=0`               | Any non-zero value                          |
| `>128`              | Greater than 128                            |
| `>=1`               | Greater than or equal to 1                  |
| `<128`              | Less than 128                               |
| `<=2`               | Less than or equal to 2                     |
| `==0`               | Exactly 0 (same as bare `0`)                |
| `(>128 && <130)`    | Between 129 and 129 (exclusive range)       |
| `(>=1 && <=2) \|\| 127` | 1, 2, or 127                           |

Supported operators: `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, parentheses
for grouping, and bare integer literals.

## Writing Good Tests

Every test should target a specific normative statement from the standard —
typically a "shall" or "shall not" clause. The goal is 100% coverage of
testable normative requirements.

### What to test

- Every testable "shall" / "shall not" / "if ... shall" statement.
- Both the positive case (correct behavior) and, where meaningful, the
  negative case (error/rejection).

### What not to test

- "May", unspecified, or implementation-defined behavior.
- Untestable statements (e.g. "the order is unspecified").
- Informational notes (text after "**Note:**").

### Test quality checklist

- **One concern per test.** Each test targets a single normative statement
  (or a tightly related cluster).
- **Descriptive name.** The intent should be clear without reading the standard.
- **Description required.** Every test must have a brief, plain-English
  explanation between the `#### Test:` heading and the code block. The
  description should explain the intent to a random reader — paraphrase or
  summarize the requirement. Since the verbatim standard text is already above
  the `### Tests` heading, blockquotes of the standard in test descriptions are
  redundant and discouraged.
- **No overassertion.** Assert only what POSIX explicitly specifies. If the
  standard says "a diagnostic message shall be written to standard error", use
  `stderr ".+"` — not a specific wording.
- **No underassertion.** If POSIX requires a diagnostic on stderr, the stderr
  pattern must not be `""` or `".*"`. Use `".+"` or a more specific pattern.
- **Correct exit codes.** Match exactly what the standard says.
- **Minimal commands.** Use concrete, minimal shell commands — avoid unnecessary
  complexity.
- **Not tailored to a specific shell.** Tests must assert POSIX-required
  behavior, not bash/dash/zsh quirks. If `bash --posix` deviates from the
  standard, the test should still assert the correct behavior — document the
  deviation in the test description and in `tests/matrix/bash_compliance.md`.
- **Locale coverage.** If the operation is locale-sensitive (character
  counting, pattern matching, collation, etc.), write paired tests for `C`
  and `C.UTF-8`. See "Testing Locale-Sensitive Behavior" below for details.

## Testing Locale-Sensitive Behavior

Many POSIX shell operations are defined in terms of *characters* rather than
bytes, or depend on locale categories like `LC_COLLATE`, `LC_CTYPE`, or
`LC_NUMERIC`. Any test that touches such an operation must verify correct
behavior across locales — a single-locale test is insufficient.

See `docs/LOCALE.md` for the full list of locale-sensitive areas in the shell
and the code-level API.

### Required locale matrix

At minimum, test with these two locales:

| Locale | Set with | What it exercises |
|--------|----------|-------------------|
| `C` | `setenv "LC_ALL" "C"` (or omit — it is the default) | Single-byte, ASCII-only. Byte = character. |
| `C.UTF-8` | `setenv "LC_ALL" "C.UTF-8"` | Multi-byte UTF-8. Characters may be 1–4 bytes. |

A test that only runs in one of these locales is a locale correctness gap.
If the standard says "character" (not "byte"), both locales must be covered.

### When both locales are needed

Write separate `C` and `C.UTF-8` variants whenever the behavior under test
involves any of the following:

- **Character counting** — `${#var}` must return characters, not bytes.
- **Pattern matching** — `?` matches one character; `[[:alpha:]]` classifies
  wide characters; `*` advances by characters.
- **String splitting** — IFS characters or `$*` separator may be multi-byte.
- **Prefix/suffix removal** — `${var#pat}`, `${var%pat}` split at character
  boundaries.
- **String comparison** — `test s1 \< s2` uses `strcoll`, which is
  locale-dependent.
- **Sorted output** — `set`, `alias`, glob results use locale collation.
- **Character classification** — `[[:upper:]]`, `[[:digit:]]`, etc.
- **Case conversion** — `~` in vi mode, `typeset -u`/`-l`.
- **Line editing** — cursor movement, deletion, replacement in vi/emacs mode.
- **`printf` character constants** — `printf '%d' "'é"` must decode a
  multi-byte character.
- **`read` field splitting** — same IFS concerns as word expansion.
- **`getopts` option characters** — option letters may be multi-byte.

### Example: paired locale tests

For `${#var}` (character count), write two tests:

```
begin test "parameter length in C locale counts bytes"
  setenv "LC_ALL" "C"
  script
    v='abc'
    echo "${#v}"
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "parameter length in C locale counts bytes"
```

```
begin test "parameter length in UTF-8 locale counts characters"
  setenv "LC_ALL" "C.UTF-8"
  script
    v='aéb'
    echo "${#v}"
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "parameter length in UTF-8 locale counts characters"
```

The first test passes even with a byte-counting implementation. The second
test fails if the shell counts bytes (it would return 4 for `aéb` since `é`
is two bytes in UTF-8). Both are needed to prove correctness.

### Testing numeric and time formatting (LC_NUMERIC, LC_TIME)

Some operations depend on `LC_NUMERIC` (decimal point) or `LC_TIME` (date
formatting). The `C` locale uses `.` as the radix character. To verify that
the shell respects a different radix character, use a system locale that
defines a comma — common choices include:

| Locale | Radix | Availability |
|--------|-------|-------------|
| `de_DE.UTF-8` | `,` | Common on Linux/macOS |
| `fr_FR.UTF-8` | `,` | Common on Linux/macOS/FreeBSD |
| `ru_RU.UTF-8` | `,` | Common on Linux/FreeBSD |

Since no single non-C locale is guaranteed to exist on all systems, numeric
and time formatting tests that require a non-C locale should be documented
with a note that they depend on the locale being installed. Prefer
`fr_FR.UTF-8` as the first choice since it is widely available.

Example:

```
begin test "arithmetic radix respects LC_NUMERIC"
  setenv "LC_NUMERIC" "fr_FR.UTF-8"
  script
    printf '%f\n' 1.5
  expect
    stdout "1,500000"
    stderr ""
    exit_code 0
end test "arithmetic radix respects LC_NUMERIC"
```

### Locale testing during improvement audits

When improving an existing test suite (see procedure below), the locale audit
is an explicit step:

1. Identify every test that exercises a locale-sensitive operation.
2. Check whether both `C` and `C.UTF-8` variants exist.
3. For missing variants, add them.
4. For numeric/time formatting operations, check whether a non-C numeric
   locale test exists and add one if feasible.

## Procedure: Creating a New Test Suite

### Step 1: Identify what to test

Choose a piece of the POSIX standard to cover:

- **Shell language section** — a section or subsection from `V3_chap02.md`
  (e.g., "2.6.2 Parameter Expansion").
- **Utility** — any utility whose behaviour the shell must implement or
  interact with. The spec page lives in `docs/posix/md/utilities/<utility>.md`.
  Special built-ins, intrinsic utilities, and commonly-builtin utilities are
  the highest priority since they are part of the shell itself.
- **Base Definitions section** — sections from `docs/posix/md/basedefs/` that
  define behaviour the shell must respect (e.g., XBD 8 Environment Variables,
  XBD 9 Regular Expressions, XBD 12 Utility Syntax Guidelines).

### Step 2: Create the file

Create the file in `tests/matrix/tests/` using the naming convention from the
table at the top of this document.

Follow the file structure described above:

1. Write the **title** (`# Test Suite for ...`) and **introduction** paragraph.
2. Write the **Table of contents** with links to all sections.
3. For **each section/subsection**:
   - Copy the **verbatim standard text** from the relevant `docs/posix/md/` file.
   - Add a `### Tests` heading.
   - Write tests: one `#### Test:` per test.
   - Write a brief **description** above each test's code block explaining what
     the test verifies (see the test quality checklist above).
   - For any locale-sensitive operation, write both `C` and `C.UTF-8` variants
     (see "Testing Locale-Sensitive Behavior" above).

### Step 3: Verify and run

Follow the verification steps below.

## Procedure: Improving an Existing Test Suite

The goal is to bring an existing suite to full coverage of all testable
normative statements in its POSIX standard sections.

**Important constraint:** only the target `.md` test suite file is modified.
`meiksh` source code (`src/`, `tests/`, etc.) must not be changed during this
procedure.

### Step 1: Audit the standard text for untested statements

Read each `## Section` block in the suite. For every normative statement
("shall", "shall not", "if ... then ... shall"), check whether an existing
test exercises it. Make a list of gaps.

Ignore:
- Statements about "may", unspecified, or implementation-defined behavior.
- Statements that are untestable (e.g. "the order is unspecified").
- Informational notes (text after "**Note:**").

### Step 2: Review existing tests against the standard

Scrutinize every existing test for correctness:

- Does it exercise at least one "shall" normative statement quoted in the
  suite's standard text sections?
- Is it asserting only what POSIX specifies (no overassertion)?
- Is it asserting everything POSIX requires (no underassertion)?
- Does the expected exit code match exactly what the standard says?
- Does the test description accurately paraphrase the standard's requirement?
- Does the test have a proper description? (See test quality checklist.)
- Is it asserting POSIX-required behavior, not bash/dash quirks?
- If the test was written to pass on `bash --posix`, does `bash` actually
  comply with the standard here? (Check `tests/matrix/bash_compliance.md`.)

When a test encodes non-standard behavior or overasserts, fix it to assert only
what the standard requires. If `bash --posix` deviates, the test will fail
against bash — that is correct and expected; document the deviation in the test
description and in `bash_compliance.md`.

### Step 3: Audit locale coverage

Follow the locale audit steps in "Testing Locale-Sensitive Behavior" above:
identify every test that exercises a locale-sensitive operation, check whether
both `C` and `C.UTF-8` variants exist, and add missing variants. Also check
for numeric/time formatting tests that need a non-C `LC_NUMERIC`/`LC_TIME`
locale.

### Step 4: Write new tests to fill gaps

For each gap (including locale gaps from Step 3), write a test following the
test quality checklist above.

### Step 5: Remove redundant tests

Within the suite, remove tests that are fully redundant with other tests in
the same file. Redundancy across different suites is acceptable and expected.

A test is redundant if another test in the same suite already covers the exact
same normative statement with equivalent rigor. When in doubt, keep the test.

### Step 6: Verify and run

Follow the verification steps below.

### Step 7: Summary

Report what was done: how many tests were added, how many removed, and which
normative statements are now covered that were not before.

## Verification Steps

These steps apply to both creating and improving a test suite.

### Parse check

```bash
cargo run --quiet --bin expect_pty -- --shell /usr/bin/bash --parse-only tests/matrix/tests/<file>.md
```

This checks that the file is syntactically valid. It reports the number of tests
found. Fix any parse errors before proceeding.

### Run the tests

```bash
cargo run --quiet --bin expect_pty -- --shell "/usr/bin/bash --posix" tests/matrix/tests/<file>.md
```

All tests must pass, except for known `bash --posix` non-compliances
(see `tests/matrix/bash_compliance.md`). If a test fails:

- Check that the regex pattern correctly matches the expected output. Remember
  that patterns are **full-match** and `.` does not match `\n`.
- Check that exit codes are correct for the shell being tested.
- Use `--test "test name"` to run a single test in isolation for debugging.

### Citation integrity

```bash
cargo run --quiet --bin check_integrity -- tests/matrix
```

This verifies that every `## Section Name` block in `.md` test suites
contains text that is **exactly verbatim** from the corresponding section in
the POSIX source documents. Any deviation — even a single character — is
flagged as an error.

## Reference Examples

- `tests/matrix/tests/2_2_quoting.md` — shell language section (Section 2.2 Quoting)
- `tests/matrix/tests/builtin_set.md` — special built-in utility
- `tests/matrix/tests/intrinsic_utility_cd.md` — intrinsic utility
- `tests/matrix/tests/maybe_builtin_echo.md` — commonly-builtin utility
- `tests/matrix/tests/utility_sh.md` — other utility (with interactive tests)
- `tests/matrix/tests/xbd_8_environment_variables.md` — Base Definitions section
