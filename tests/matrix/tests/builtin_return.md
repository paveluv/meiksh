# Test Suite for 2.15 Special Built-In: return

This test suite covers the **return** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities return](#215-special-built-in-utilities-return)

## 2.15 Special Built-In Utilities return

#### NAME

> return — return from a function or dot script

#### SYNOPSIS

> `return [n]`

#### DESCRIPTION

> The [*return*](#return) utility shall cause the shell to stop executing the current function or [*dot*](#dot) script. If the shell is not currently executing a function or [*dot*](#dot) script, the results are unspecified.

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

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The exit status shall be *n*, if specified, except that the behavior is unspecified if *n* is not an unsigned decimal integer or is greater than 255. If *n* is not specified, the result shall be as if *n* were specified with the current value of the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)), except that if the [*return*](#return) command would cause the end of execution of a [*trap*](#trap) action, the value for the special parameter `'?'` that is considered "current" shall be the value it had immediately preceding the [*trap*](#trap) action.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> None.

#### RATIONALE

> The behavior of [*return*](#return) when not in a function or [*dot*](#dot) script differs between the System V shell and the KornShell. In the System V shell this is an error, whereas in the KornShell, the effect is the same as [*exit*](#exit).
>
> The results of returning a number greater than 255 are undefined because of differing practices in the various historical implementations. Some shells AND out all but the low-order 8 bits; others allow larger values, but not of unlimited size.
>
> See the discussion of appropriate exit status values under [exit](#tag_19_22).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.9.5 Function Definition Command](#295-function-definition-command), [2.15 Special Built-In Utilities](#215-special-built-in-utilities), [dot](#tag_19_19)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0044 [214] and XCU/TC1-2008/0045 [214] are applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0052 [960] is applied.

#### Issue 8

> Austin Group Defect 1309 is applied, changing the EXIT STATUS section.
>
> Austin Group Defect 1602 is applied, clarifying the behavior of [*return*](#return) in a [*trap*](#trap) action.

*End of informative text.*

### Tests

#### Test: return with explicit n sets exit status

`return n` causes a function to return with exit status `n`.

```
begin test "return with explicit n sets exit status"
  script
    myfunc() { return 5; }
    myfunc
    echo "$?"
  expect
    stdout "5"
    stderr ""
    exit_code 0
end test "return with explicit n sets exit status"
```

#### Test: return without n inherits previous exit status

When `n` is not specified, return uses the current `$?`.

```
begin test "return without n inherits previous exit status"
  script
    myfunc() { false; return; }
    myfunc
    echo "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "return without n inherits previous exit status"
```

#### Test: return from dot-sourced file stops the dot command

`return` ends execution of the current dot script; the dot command’s
exit status becomes `n` when `n` is given.

```
begin test "return from dot-sourced file stops the dot command"
  script
    echo 'return 9' > tmp_dot_return.sh
    . ./tmp_dot_return.sh
    echo "status=$?"
    rm -f tmp_dot_return.sh
  expect
    stdout "status=9"
    stderr ""
    exit_code 0
end test "return from dot-sourced file stops the dot command"
```

#### Test: return without n in dot script uses current exit status

When `return` is used without `n` in a dot script, the exit status of
the dot command is the current value of `$?`.

```
begin test "return without n in dot script uses current exit status"
  script
    printf 'false\nreturn\necho no\n' > tmp_dot_ret_noval.sh
    . ./tmp_dot_ret_noval.sh
    echo "status=$?"
    rm -f tmp_dot_ret_noval.sh
  expect
    stdout "status=1"
    stderr ""
    exit_code 0
end test "return without n in dot script uses current exit status"
```

#### Test: return stops execution inside dot script

After `return`, no further commands from the dot script are
executed. Only commands before `return` produce output.

```
begin test "return stops execution inside dot script"
  script
    printf 'echo before\nreturn 0\necho after\n' > tmp_dot_ret_stop.sh
    . ./tmp_dot_ret_stop.sh
    echo "parent"
    rm -f tmp_dot_ret_stop.sh
  expect
    stdout "before\nparent"
    stderr ""
    exit_code 0
end test "return stops execution inside dot script"
```
