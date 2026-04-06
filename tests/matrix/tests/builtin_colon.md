# Test Suite for 2.15 Special Built-In: : (colon)

This test suite covers the **colon** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities colon](#215-special-built-in-utilities-colon)

## 2.15 Special Built-In Utilities colon

#### NAME

> colon — null utility

#### SYNOPSIS

> `: [argument...]`

#### DESCRIPTION

> This utility shall do nothing except return a 0 exit status. It is used when a command is needed, as in the **then** condition of an **if** command, but nothing is to be done by the command.

#### OPTIONS

> This utility shall not recognize the `"--"` argument in the manner specified by Guideline 10 of XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> Implementations shall not support any options.

#### OPERANDS

> See the DESCRIPTION.

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

> See the APPLICATION USAGE for [*true*](docs/posix/md/utilities/true.md).

#### EXAMPLES

> ```
> : "${X=abc}"
> if     false
> then   :
> else   printf '%s\n' "$X"
> fi
>
> abc
> ```
>
> As with any of the special built-ins, the null utility can also have variable assignments and redirections associated with it, such as:
>
> ```
> x=y : > z
> ```
>
> which sets variable *x* to the value *y* (so that it persists after the null utility completes) and creates or truncates file **z**; if the file cannot be created or truncated, a non-interactive shell exits (see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)).

#### RATIONALE

> None.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities), [*true*](docs/posix/md/utilities/true.md)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 1272 is applied, clarifying that the null utility does not process its arguments, does not recognize the `"--"` end-of-options delimiter, does not support any options, and does not write to standard error.
>
> Austin Group Defect 1640 is applied, changing the APPLICATION USAGE section.

*End of informative text.*

### Tests

#### Test: colon returns exit code 0

The `:` utility does nothing and returns exit status 0.

```
begin test "colon returns exit code 0"
  script
    :
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "colon returns exit code 0"
```

#### Test: colon with arguments still returns 0

The `:` utility ignores its arguments.

```
begin test "colon with arguments still returns 0"
  script
    : arg1 arg2 arg3
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "colon with arguments still returns 0"
```

#### Test: colon with variable assignment persists

Prefix assignments on a simple command still apply when the command is
the null utility, so the variable remains set afterward.

```
begin test "colon with variable assignment persists"
  script
    MY_COLON_VAR=hello :
    echo "$MY_COLON_VAR"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "colon with variable assignment persists"
```

#### Test: colon does not treat -- as end of options

The null utility does not recognize `--` as an end-of-options delimiter;
operands that look like options are still ordinary arguments.

```
begin test "colon does not treat -- as end of options"
  script
    : -- -f
    echo "ok"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "colon does not treat -- as end of options"
```

#### Test: colon with assignment and redirection

As with other special built-ins, prefix assignment and redirections on
`:` run in the usual way: the file is created or truncated and the
assignment persists.

```
begin test "colon with assignment and redirection"
  script
    rm -f tmp_colon_z.txt
    x=y : > tmp_colon_z.txt
    echo "x=$x"
    test -f tmp_colon_z.txt && echo "file_ok"
    rm -f tmp_colon_z.txt
  expect
    stdout "x=y\nfile_ok"
    stderr ""
    exit_code 0
end test "colon with assignment and redirection"
```
