# Improving Markdown Test Suites

This document describes how to improve an existing `.md` test suite to achieve
full coverage of all testable normative statements in its POSIX standard
sections.

## Goal

Every testable "shall" statement in the verbatim standard text embedded in the
suite should be exercised by at least one test. The target is 100% coverage of
normative requirements — catching all POSIX non-compliances, including subtle
ones.

## Context and references

The test suite file itself is the primary source of context. Each
`## X.Y Section Name` block contains the verbatim standard text for that
section. Read it carefully — every normative statement ("shall", "shall not",
"if ... shall") is a candidate for testing.

If a hyperlink in the standard text needs to be followed, links are
root-relative from the project directory (e.g.
`docs/posix/md/basedefs/V1_chap03.md`). Links that begin with `#` are
internal to `docs/posix/md/utilities/V3_chap02.md`.

For known `bash --posix` non-compliances, see `tests/matrix/bash_compliance.md`.
Tests exercising known non-compliances are acceptable (they document the
deviation) but should be noted in the test description.

## Test format reference

### Structure

Tests live under `### Tests` headings within each section. Each test has:

1. A heading: `#### Test: descriptive test name`
2. A brief plain-English description of what the test verifies.
3. A single fenced code block containing the test body.

The test body inside the code block follows this format:

```
begin test "descriptive test name"
  script
    <shell commands>
  expect
    stdout "<pattern>"
    stderr "<pattern>"
    exit_code <expr>
end test "descriptive test name"
```

**Rules:**
- The name after `#### Test: ` must exactly match `begin test "..."` / `end test "..."`.
- Each test section has exactly one fenced code block.
- `script` at column 2, body at column 4, `expect` at column 2, assertions at column 4.
- Assertions must appear in order: `stdout`, `stderr`, `exit_code` (all required).

### Interactive tests

```
begin interactive test "name"
  spawn -i
  expect "$ "
  send "command"
  expect "output"
  sendeof
  wait
end interactive test "name"
```

### Pattern syntax

- Patterns are **full-match** (anchored both ends); `.` does not match `\n`.
- Backslash passes through verbatim to the regex engine — `\(` matches literal `(`.
- To embed a literal `"` in a pattern, use `""`.
- Common: `""` (empty), `".+"` (any line), `"(.|\n)*"` (any including multiline).
- Metacharacters needing escaping: `.  *  +  ?  |  (  )  [  ]  ^  $  \  {  }`

### Exit code expressions

Bare integers, comparison operators (`==`, `!=`, `<`, `<=`, `>`, `>=`),
boolean operators (`&&`, `||`), and parentheses for grouping.
Examples: `0`, `!=0`, `>128`, `(>=1 && <=2) || 127`.

## Procedure

### Step 1: Audit the standard text for untested statements

Read each `## Section` block in the suite. For every normative statement
("shall", "shall not", "if ... then ... shall"), check whether an existing
test exercises it. Make a list of gaps.

Ignore:
- Statements about unspecified or implementation-defined behavior.
- Statements that are untestable (e.g. "the order is unspecified").
- Informational notes (text after "**Note:**").

### Step 1b: Verify existing tests match the standard

Review every existing test in the suite and confirm that its assertions reflect
the behavior required by the POSIX standard — not merely the behavior of
`bash --posix` or any other specific implementation. Migrated tests are
especially prone to encoding implementation quirks rather than normative
requirements.

For each test, ask:
- Does the expected exit code match what the standard says?
- Does the test description accurately paraphrase the standard's requirement?
- Does the test have a proper description? Every test must have a brief,
  plain-English explanation between the `#### Test:` heading and the code
  block. The description should explain the intent of the test to a random
  reader — conveniently paraphrase or summarize the relevant requirement(s)
  rather than pasting a blockquote of the standard text. Since the verbatim
  standard text is already provided above the `### Tests` heading,
  blockquotes of the standard in the test description are redundant and
  discouraged. Tests with missing or empty descriptions must be fixed.
- If the test was written to pass on `bash --posix`, does `bash` actually
  comply with the standard here? (Check `tests/matrix/bash_compliance.md`.)

When a test encodes non-standard behavior, fix it to assert what the standard
requires. If `bash --posix` deviates, the test will fail against bash — that
is correct and expected; document the deviation in the test description and
in `bash_compliance.md`.

### Step 2: Write new tests to fill gaps

For each gap, write a test that:
- Targets a single normative statement (or a tightly related cluster).
- Has a descriptive name that makes the intent clear without reading the standard.
- Includes a brief description above the code block explaining what rule is
  being tested.
- Exercises both the positive case (correct behavior) and, where meaningful,
  the negative case (error/rejection).
- Uses concrete, minimal shell commands — avoid unnecessary complexity.

### Step 3: Remove redundant tests

Within the suite, remove tests that are fully redundant with other tests in
the same file. Redundancy across different suites is acceptable and expected.

A test is redundant if another test in the same suite already covers the exact
same normative statement with equivalent rigor. When in doubt, keep the test.

### Step 4: Verify parsing

```bash
cargo run --quiet --bin expect_pty -- --shell /usr/bin/bash --parse-only tests/matrix/tests/<file>.md
```

Fix any parse errors before proceeding.

### Step 5: Run the tests

```bash
cargo run --quiet --bin expect_pty -- --shell "/usr/bin/bash --posix" tests/matrix/tests/<file>.md
```

All tests must pass, except for known `bash --posix` non-compliances
(see `tests/matrix/bash_compliance.md`). Debug failures with:

```bash
cargo run --quiet --bin expect_pty -- --shell "/usr/bin/bash --posix" --test "test name" tests/matrix/tests/<file>.md
```

### Step 6: Verify citation integrity

```bash
cargo run --quiet --bin check_integrity -- tests/matrix
```

This must pass with zero errors. The improvement procedure does not modify
standard text sections, so this step is a sanity check.

### Step 7: Summary

Report what was done: how many tests were added, how many removed, and which
normative statements are now covered that were not before.
