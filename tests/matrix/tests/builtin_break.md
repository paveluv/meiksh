# Test Suite for 2.15 Special Built-In: break

This test suite covers the **break** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities break](#215-special-built-in-utilities-break)

## 2.15 Special Built-In Utilities break

#### NAME

> break — exit from for, while, or until loop

#### SYNOPSIS

> `break [n]`

#### DESCRIPTION

> If *n* is specified, the [*break*](#break) utility shall exit from the *n*th enclosing **for**, **while**, or **until** loop. If *n* is not specified, [*break*](#break) shall behave as if *n* was specified as 1. Execution shall continue with the command immediately following the exited loop. The application shall ensure that the value of *n* is a positive decimal integer. If *n* is greater than the number of enclosing loops, the outermost enclosing loop shall be exited. If there is no enclosing loop, the behavior is unspecified.
>
> A loop shall enclose a *break* or *continue* command if the loop lexically encloses the command. A loop lexically encloses a *break* or *continue* command if the command is:
>
> - Executing in the same execution environment (see [2.13 Shell Execution Environment](#213-shell-execution-environment)) as the compound-list of the loop's do-group (see [2.10.2 Shell Grammar Rules](#2102-shell-grammar-rules)), and
> - Contained in a compound-list associated with the loop (either in the compound-list of the loop's do-group or, if the loop is a **while** or **until** loop, in the compound-list following the **while** or **until** reserved word), and
> - Not in the body of a function whose function definition command (see [2.9.5 Function Definition Command](#295-function-definition-command)) is contained in a compound-list associated with the loop.
>
> If *n* is greater than the number of lexically enclosing loops and there is a non-lexically enclosing loop in progress in the same execution environment as the *break* or *continue* command, it is unspecified whether that loop encloses the command.

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
> foo() {
> for j in 1 2; do
> echo 'break 2' >/tmp/do_break
> echo "  sourcing /tmp/do_break ($j)..."
> # the behavior of the break from running the following command
> # results in unspecified behavior:
> . /tmp/do_break
> do_continue() { continue 2; }
> echo "  running do_continue ($j)..."
> # the behavior of the continue in the following function call
> # results in unspecified behavior (if execution reaches this
> # point):
> do_continue
> trap 'continue 2' USR1
> echo "  sending SIGUSR1 to self ($j)..."
> # the behavior of the continue in the trap invoked from the
> # following signal results in unspecified behavior (if
> # execution reaches this point):
> kill -s USR1 $$
> sleep 1
> done
> }
> for i in 1 2; do
> echo "running foo ($i)..."
> foo
> done
> ```

#### RATIONALE

> In early proposals, consideration was given to expanding the syntax of [*break*](#break) and [*continue*](#continue) to refer to a label associated with the appropriate loop as a preferable alternative to the *n* method. However, this volume of POSIX.1-2024 does reserve the name space of command names ending with a `<colon>`. It is anticipated that a future implementation could take advantage of this and provide something like:
>
> ```
> outofloop: for i in a b c d e
> do
>     for j in 0 1 2 3 4 5 6 7 8 9
>     do
>         if test -r "${i}${j}"
>         then break outofloop
>         fi
>     done
> done
> ```
>
> and that this might be standardized after implementation experience is achieved.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0046 [842] is applied.

#### Issue 8

> Austin Group Defect 1058 is applied, clarifying that the requirement for *n* to be a positive decimal integer is a requirement on the application.

*End of informative text.*

### Tests

#### Test: break without n defaults to 1

When `n` is not specified, `break` exits from the innermost enclosing loop.

```
begin test "break without n defaults to 1"
  script
    for i in 1 2 3; do
      echo $i
      break
      echo "no"
    done
    echo "done"
  expect
    stdout "1\ndone"
    stderr ""
    exit_code 0
end test "break without n defaults to 1"
```

#### Test: break 2 exits two loops

`break 2` exits from the 2nd enclosing loop.

```
begin test "break 2 exits two loops"
  script
    for i in a b; do
      for j in 1 2
      do echo "$i$j"
      break 2
    done
    done
    echo "done"
  expect
    stdout "a1\ndone"
    stderr ""
    exit_code 0
end test "break 2 exits two loops"
```

#### Test: break n greater than enclosing loops

When `n` exceeds the number of enclosing loops, the outermost loop is exited.

```
begin test "break n greater than enclosing loops"
  script
    for i in a b; do
      echo $i
      break 5
    done
    echo "done"
  expect
    stdout "a\ndone"
    stderr ""
    exit_code 0
end test "break n greater than enclosing loops"
```

#### Test: break with invalid n returns non-zero in a loop

If `n` is not an unsigned decimal integer greater than or equal to 1,
the `break` utility yields a non-zero exit status. The failure is
exercised inside a subshell so the surrounding script can still print
the subshell’s exit status.

```
begin test "break with invalid n returns non-zero in a loop"
  script
    ( for i in 1; do break 0; done )
    echo "outer=$?"
  expect
    stdout "outer=1"
    stderr "(.|\n)*"
    exit_code 0
end test "break with invalid n returns non-zero in a loop"
```

#### Test: break with non-numeric operand fails in a loop

The loop count must be an unsigned decimal integer; otherwise `break`
fails and the subshell exits non-zero.

```
begin test "break with non-numeric operand fails in a loop"
  script
    ( for i in 1; do break bogus; done )
    echo "outer=$?"
  expect
    stdout "outer=[1-9][0-9]*"
    stderr "(.|\n)*"
    exit_code 0
end test "break with non-numeric operand fails in a loop"
```
