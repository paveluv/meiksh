# Test Suite for Maybe-Builtin Utility: true

This test suite covers the **true** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: true](#utility-true)

## utility: true

#### NAME

> true — return true value

#### SYNOPSIS

> `true`

#### DESCRIPTION

> The *true* utility shall return with exit code zero.

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

> Zero.

#### CONSEQUENCES OF ERRORS

> None.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is typically used in shell scripts, as shown in the EXAMPLES section.
>
> Although the special built-in utility **:** ([*colon*](docs/posix/md/utilities/colon.md)) is similar to *true*, there are some notable differences, including:
>
> - Whereas [*colon*](docs/posix/md/utilities/colon.md) is required to accept, and do nothing with, any number of arguments, *true* is only required to accept, and discard, a first argument of `"--"`. Passing any other argument(s) to *true* may cause its behavior to differ from that described in this standard.
> - A non-interactive shell exits when a redirection error occurs with [*colon*](docs/posix/md/utilities/colon.md) (unless executed via [*command*](docs/posix/md/utilities/command.md)), whereas with *true* it does not.
> - Variable assignments preceding the command name persist after executing [*colon*](docs/posix/md/utilities/colon.md) (unless executed via [*command*](docs/posix/md/utilities/command.md)), but not after executing *true*.
> - In shell implementations where *true* is not provided as a built-in, using [*colon*](docs/posix/md/utilities/colon.md) avoids the overheads associated with executing an external utility.

#### EXAMPLES

> This command is executed forever:
>
> ```
> while true
> do
>     command
> done
> ```

#### RATIONALE

> None.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.9 Shell Commands*](docs/posix/md/utilities/V3_chap02.md#29-shell-commands), [*colon*](docs/posix/md/utilities/V3_chap02.md#tag_19_17), [*command*](docs/posix/md/utilities/command.md), [*false*](docs/posix/md/utilities/false.md)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/39 is applied, replacing the terms "None" and "Default" from the STDERR and EXIT STATUS sections, respectively, with terms as defined in [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults).

#### Issue 8

> Austin Group Defect 1640 is applied, clarifying the differences between *true* and **:** ([*colon*](docs/posix/md/utilities/colon.md)).

*End of informative text.*

### Tests

#### Test: true returns exit code zero

The `true` utility returns a zero exit status.

```
begin test "true returns exit code zero"
  script
    true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "true returns exit code zero"
```

#### Test: true produces no stdout

The `true` utility writes nothing to standard output.

```
begin test "true produces no stdout"
  script
    output=$(true)
    printf '%s' "$output"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "true produces no stdout"
```

#### Test: true produces no stderr

The `true` utility writes nothing to standard error.

```
begin test "true produces no stderr"
  script
    output=$(true 2>&1 >/dev/null)
    printf '%s' "$output"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "true produces no stderr"
```

#### Test: true with double-dash operand returns zero

The `true` utility accepts and discards a `--` argument and still returns zero.

```
begin test "true with double-dash operand returns zero"
  script
    true --
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "true with double-dash operand returns zero"
```
