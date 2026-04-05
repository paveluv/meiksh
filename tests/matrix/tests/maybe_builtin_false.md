# Test Suite for Maybe-Builtin Utility: false

This test suite covers the **false** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: false](#utility-false)

## utility: false

#### NAME

> false — return false value

#### SYNOPSIS

> `false`

#### DESCRIPTION

> The *false* utility shall return with a non-zero exit code.

#### OPTIONS

> None.

#### OPERANDS

> None.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> None.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> Not used.

#### STDERR

> Not used.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The *false* utility shall always exit with a value between 1 and 125, inclusive.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> None.

#### RATIONALE

> None.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*true*](docs/posix/md/utilities/true.md)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/24 is applied, changing the STDERR section from "None." to "Not used." for alignment with [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults).

#### Issue 8

> Austin Group Defect 1321 is applied, changing the EXIT STATUS section.

*End of informative text.*

### Tests

#### Test: false returns non-zero exit status

The `false` utility returns a non-zero exit status.

```
begin test "false returns non-zero exit status"
  script
    false
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "false returns non-zero exit status"
```

#### Test: false produces no output

`false` writes nothing to standard output or standard error.

```
begin test "false produces no output"
  script
    output=$(false 2>&1)
    echo "len=${#output}"
  expect
    stdout "len=0"
    stderr ""
    exit_code 0
end test "false produces no output"
```

#### Test: false ignores operands

Any operands or options given to `false` are ignored.

```
begin test "false ignores operands"
  script
    false --help 2>/dev/null
    echo $?
  expect
    stdout "[1-9].*"
    stderr ""
    exit_code 0
end test "false ignores operands"
```

#### Test: false returns non-zero exit code

The `false` utility shall return with a non-zero exit code.

```
begin test "false returns non-zero exit code"
  script
    false
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "false returns non-zero exit code"
```

#### Test: false exit code is between 1 and 125

The `false` utility shall always exit with a value between 1 and
125, inclusive.

```
begin test "false exit code is between 1 and 125"
  script
    false
    rc=$?
    [ $rc -ge 1 ] && [ $rc -le 125 ] && echo "in_range"
  expect
    stdout "in_range"
    stderr ""
    exit_code 0
end test "false exit code is between 1 and 125"
```

#### Test: false exits 1

The `false` utility shall always return a non-zero exit code. This test checks that the specific exit code is 1, the most common value used by implementations.

```
begin test "false exits 1"
  script
    false
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "false exits 1"
```
