# Creating Markdown Test Suites

This document describes how to create a Markdown-based test suite for `expect_pty`,
the POSIX shell conformance test runner. Each `.md` test suite covers one section
of the POSIX Shell Command Language standard.

## Prerequisites

- The POSIX standard text lives in `docs/posix/md/utilities/V3_chap02.md`.
- Existing requirements are catalogued in `tests/matrix/requirements.json`.
- Existing `.epty` tests in `tests/matrix/tests/` already cover many requirements
  and can be migrated into the new `.md` format.
- Test suites go in `tests/matrix/tests/` and must have a `.md` extension.

## File Structure

Every `.md` test suite follows this exact layout:

    # Introduction

    Brief paragraph describing which section is being tested and what it covers.

    ## Table of contents

    - [X.Y Section Name](#xy-section-name)
    - [X.Y.1 Subsection Name](#xy1-subsection-name)
    - ...

    ## X.Y Section Name

    <verbatim text of section X.Y from docs/posix/md, preamble only — stop before
    the first subsection heading>

    ### Tests

    ##### Test: test name here

    Brief description of what this test verifies and why.

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

    ##### Test: another test

    ...more tests...

    ## X.Y.1 Subsection Name

    <verbatim text of subsection X.Y.1 from docs/posix/md>

    ### Tests

    ##### Test: subsection test

    ...tests for this subsection...

    ## X.Y.2 Next Subsection

    ...same pattern repeats for every subsection...

### Key rules

1. **Section headings** use `##` (level 2). Subsection headings also use `##`.
2. **"Tests" headings** use `###` (level 3), always literally `### Tests`.
3. **Test headings** use `#####` (level 5), always in the form `##### Test: <name>`.
4. The test name after `##### Test: ` must **exactly match** the name in the
   `begin test "..."` and `end test "..."` lines inside the code block.
5. Each `##### Test:` section must contain **exactly one** fenced code block
   (`` ``` ``). The code block must not contain triple backticks internally.
6. **Standard text** is copied verbatim from `docs/posix/md/utilities/V3_chap02.md`.
   Code blocks in the standard text (outside `##### Test:` sections) are ignored
   by the parser and are safe to include.
7. Markdown text between the `##### Test:` heading and the code block (the
   description) must not contain any headings (lines starting with `#`).
8. Everything outside `##### Test:` sections is treated as documentation and
   ignored by the test runner.

## Test Block Format (`.epty` DSL)

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

Interactive tests use PTY commands (`spawn`, `expect`, `send`, `sendeof`, `wait`,
etc.) instead of `script`/`expect` blocks. These are less common in section-based
test suites.

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
| `\r`        | Literal carriage return                            |
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

## Procedure

### Step 1: Identify the section

Choose a section from the POSIX standard (e.g., "2.6.2 Parameter Expansion").
The section numbering comes from `docs/posix/md/utilities/V3_chap02.md`.

### Step 2: Find requirements and existing tests

1. Open `tests/matrix/requirements.json` and find all requirements whose
   `section_path` array contains the target section (e.g., entries where
   `section_path` includes `"2.6.2 Parameter Expansion"`).
2. Search existing `.epty` files in `tests/matrix/tests/` for references to
   these requirement IDs to find tests that already cover them.
3. Note which requirements are marked `testable: false` — skip those.

### Step 3: Create the file

Create `tests/matrix/tests/<section_number>.md` using the underscore-separated
section number (e.g., `2_6_2_parameter_expansion.md`). Follow the file structure
described above:

1. Write the **Introduction** paragraph.
2. Write the **Table of contents** with links to all subsections.
3. For **each section/subsection**:
   - Copy the **verbatim standard text** from `docs/posix/md/utilities/V3_chap02.md`.
   - Add a `### Tests` heading.
   - Add test blocks migrated from existing `.epty` files, one `##### Test:` per test.
   - Write a brief **description** above each test's code block explaining what
     the test verifies.

### Step 4: Verify parsing

```bash
cargo run --quiet --bin expect_pty -- --shell /usr/bin/bash --parse-only tests/matrix/tests/<file>.md
```

This checks that the file is syntactically valid. It reports the number of tests
found. Fix any parse errors before proceeding.

### Step 5: Run the tests

```bash
cargo run --quiet --bin expect_pty -- --shell "/usr/bin/bash --posix" tests/matrix/tests/<file>.md
```

All tests must pass, except for known `bash --posix` non-compliances
(see `tests/matrix/bash_compliance.md`). If a test fails:

- Check that the regex pattern correctly matches the expected output. Remember
  that patterns are **full-match** and `.` does not match `\n`.
- Check that exit codes are correct for the shell being tested.
- Use `--test "test name"` to run a single test in isolation for debugging.

## Reference Example

See `tests/matrix/tests/2_2_quoting.md` for a complete example covering
Section 2.2 (Quoting) with all four subsections.
