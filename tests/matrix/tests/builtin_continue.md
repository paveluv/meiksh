# Test Suite for 2.15 Special Built-In: continue

This test suite covers the **continue** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities continue](#215-special-built-in-utilities-continue)

## 2.15 Special Built-In Utilities continue

#### NAME

> continue — continue for, while, or until loop

#### SYNOPSIS

> `continue [n]`

#### DESCRIPTION

> If *n* is specified, the [*continue*](#continue) utility shall return to the top of the *n*th enclosing **for**, **while**, or **until** loop. If *n* is not specified, [*continue*](#continue) shall behave as if *n* was specified as 1. Returning to the top of the loop involves repeating the condition list of a **while** or **until** loop or performing the next assignment of a **for** loop, and re-executing the loop if appropriate.
>
> The application shall ensure that the value of *n* is a positive decimal integer. If *n* is greater than the number of enclosing loops, the outermost enclosing loop shall be used. If there is no enclosing loop, the behavior is unspecified.
>
> The meaning of "enclosing" shall be as specified in the description of the [*break*](#break) utility.

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

> - 0: Successful completion.
> - \>0: The *n* value was not an unsigned decimal integer greater than or equal to 1.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> ```
> for i in *
> do
>     if test -d "$i"
>     then continue
>     fi
>     printf '"%s" is not a directory.\n' "$i"
> done
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

> The example is changed to use the [*printf*](docs/posix/md/utilities/printf.md) utility rather than [*echo*](docs/posix/md/utilities/echo.md).
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0046 [842] is applied.

#### Issue 8

> Austin Group Defect 1058 is applied, clarifying that the requirement for *n* to be a positive decimal integer is a requirement on the application.

*End of informative text.*

### Tests

#### Test: continue without n defaults to 1

When `n` is not specified, `continue` returns to the top of the
innermost enclosing loop.

```
begin test "continue without n defaults to 1"
  script
    for i in 1 2; do
      echo $i
      continue
      echo "no"
    done
    echo "done"
  expect
    stdout "1\n2\ndone"
    stderr ""
    exit_code 0
end test "continue without n defaults to 1"
```

#### Test: continue 2 returns to outer loop

`continue 2` returns to the top of the 2nd enclosing loop.

```
begin test "continue 2 returns to outer loop"
  script
    for i in a b; do
      echo "outer $i"
      for j in 1 2; do
        echo "inner $j"
        continue 2
      done
    done
    echo "done"
  expect
    stdout "outer a\ninner 1\nouter b\ninner 1\ndone"
    stderr ""
    exit_code 0
end test "continue 2 returns to outer loop"
```

#### Test: continue n greater than enclosing loops uses outermost

When `n` exceeds the number of enclosing loops, the outermost loop
is continued.

```
begin test "continue n greater than enclosing loops uses outermost"
  script
    for i in a b; do
      echo "$i"
      continue 5
      echo "skipped"
    done
    echo "done"
  expect
    stdout "a\nb\ndone"
    stderr ""
    exit_code 0
end test "continue n greater than enclosing loops uses outermost"
```

#### Test: continue with invalid n returns non-zero in a loop

If `n` is not an unsigned decimal integer greater than or equal to 1,
the `continue` utility yields a non-zero exit status. The check runs in
a subshell so the parent can report that exit status portably.

```
begin test "continue with invalid n returns non-zero in a loop"
  script
    ( for i in 1; do continue 0; done )
    echo "outer=$?"
  expect
    stdout "outer=1"
    stderr "(.|\n)*"
    exit_code 0
end test "continue with invalid n returns non-zero in a loop"
```

#### Test: continue with non-numeric operand fails in a loop

The loop count must be an unsigned decimal integer.

```
begin test "continue with non-numeric operand fails in a loop"
  script
    ( for i in 1; do continue bogus; done )
    echo "outer=$?"
  expect
    stdout "outer=[1-9][0-9]*"
    stderr "(.|\n)*"
    exit_code 0
end test "continue with non-numeric operand fails in a loop"
```

#### Test: continue 0 exits non-interactive shell

Since `continue` is a special built-in, an error from it shall cause a
non-interactive shell to exit per
[2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors).
The operand must be an unsigned decimal integer >= 1; zero is invalid.
Known `bash --posix` non-compliance #13: bash writes a diagnostic but
continues execution instead of exiting.

```
begin test "continue 0 exits non-interactive shell"
  script
    for i in 1; do continue 0; done
    echo survived
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "continue 0 exits non-interactive shell"
```
