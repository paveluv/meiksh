# Test Suite for 2.15 Special Built-In: shift

This test suite covers the **shift** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities shift](#215-special-built-in-utilities-shift)

## 2.15 Special Built-In Utilities shift

#### NAME

> shift — shift positional parameters

#### SYNOPSIS

> `shift [n]`

#### DESCRIPTION

> The positional parameters shall be shifted. Positional parameter 1 shall be assigned the value of parameter (1+*n*), parameter 2 shall be assigned the value of parameter (2+*n*), and so on. The parameters represented by the numbers `"$#"` down to `"$#-n+1"` shall be unset, and the parameter `'#'` is updated to reflect the new number of positional parameters.
>
> The value *n* shall be an unsigned decimal integer less than or equal to the value of the special parameter `'#'`. If *n* is not given, it shall be assumed to be 1. If *n* is 0, the positional and special parameters are not changed.

#### OPTIONS

> None.

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

> The standard error shall be used only for diagnostic messages and the warning message specified in EXIT STATUS.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> If the *n* operand is invalid or is greater than `"$#"`, this may be treated as an error and a non-interactive shell may exit; if the shell does not exit in this case, a non-zero exit status shall be returned and a warning message shall be written to standard error. Otherwise, zero shall be returned.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> ```
> $
>  set a b c d e
>
> $
>  shift 2
>
> $
>  echo $*
>
> c d e
> ```

#### RATIONALE

> None.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0051 [459] is applied.

#### Issue 8

> Austin Group Defect 1265 is applied, updating the EXIT STATUS and STDERR sections to align with the changes made to [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) between Issue 6 and Issue 7.

*End of informative text.*

### Tests

#### Test: shift 1 without arguments shifts $2 to $1

When `n` is not given, `shift` defaults to 1.

```
begin test "shift 1 without arguments shifts $2 to $1"
  script
    set -- a b c
    echo $#
    shift
    echo "$1 $#"
  expect
    stdout "3\nb 2"
    stderr ""
    exit_code 0
end test "shift 1 without arguments shifts $2 to $1"
```

#### Test: shift 2 shifts $3 to $1

`shift 2` removes the first two positional parameters.

```
begin test "shift 2 shifts $3 to $1"
  script
    set -- a b c
    echo $#
    shift 2
    echo "$1 $#"
  expect
    stdout "3\nc 1"
    stderr ""
    exit_code 0
end test "shift 2 shifts $3 to $1"
```

#### Test: shift greater than $# fails and leaves parameters intact

When `n` exceeds `$#`, the shell reports an error and parameters
are unchanged.

```
begin test "shift greater than $# fails and leaves parameters intact"
  script
    set -- a b c
    shift 5 2>/dev/null
    echo "$? $1"
  expect
    stdout ".*[1-9].* a"
    stderr ""
    exit_code 0
end test "shift greater than $# fails and leaves parameters intact"
```
