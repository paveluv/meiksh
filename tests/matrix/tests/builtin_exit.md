# Test Suite for 2.15 Special Built-In: exit

This test suite covers the **exit** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities exit](#215-special-built-in-utilities-exit)

## 2.15 Special Built-In Utilities exit

#### NAME

> exit — cause the shell to exit

#### SYNOPSIS

> `exit [n]`

#### DESCRIPTION

> The [*exit*](#exit) utility shall cause the shell to exit from its current execution environment. If the current execution environment is a subshell environment, the shell shall exit from the subshell environment and continue in the environment from which that subshell environment was invoked; otherwise, the shell utility shall terminate. The wait status of the shell or subshell shall be determined by the unsigned decimal integer *n*, if specified.
>
> If *n* is specified and has a value between 0 and 255 inclusive, the wait status of the shell or subshell shall indicate that it exited with exit status *n*. If *n* is specified and has a value greater than 256 that corresponds to an exit status the shell assigns to commands terminated by a valid signal (see [2.8.2 Exit Status for Commands](#282-exit-status-for-commands)), the wait status of the shell or subshell shall indicate that it was terminated by that signal. No other actions associated with the signal, such as execution of [*trap*](#trap) actions or creation of a core image, shall be performed by the shell.
>
> If *n* is specified and is not an unsigned decimal integer, or has a value of 256, or has a value greater than 256 but not corresponding to an exit status the shell assigns to commands terminated by a valid signal, the wait status of the shell or subshell is unspecified.
>
> If *n* is not specified, the result shall be as if *n* were specified with the current value of the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)), except that if the [*exit*](#exit) command would cause the end of execution of a [*trap*](#trap) action, the value for the special parameter `'?'` that is considered "current" shall be the value it had immediately preceding the [*trap*](#trap) action.
>
> A [*trap*](#trap) action on **EXIT** shall be executed before the shell terminates, except when the [*exit*](#exit) utility is invoked in that [*trap*](#trap) action itself, in which case the shell shall exit immediately. It is unspecified whether setting a new [*trap*](#trap) action on **EXIT** during execution of a [*trap*](#trap) action on **EXIT** will cause the new [*trap*](#trap) action to be executed before the shell terminates.

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

> The [*exit*](#exit) utility causes the shell to exit from its current execution environment, and therefore does not itself return an exit status.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> As explained in other sections, certain exit status values have been reserved for special uses and should be used by applications only for those purposes:
>
> - 126: A file to be executed was found, but it was not an executable utility.
> - 127: A utility to be executed was not found.
> - 128: An unrecoverable read error was detected by the shell while reading commands, except from the *file* operand of the [*dot*](#dot) special built-in.
> - \>128: A command was interrupted by a signal.

#### EXAMPLES

> Exit with a *true* value:
>
> ```
> exit 0
> ```
>
> Exit with a *false* value:
>
> ```
> exit 1
> ```
>
> Propagate error handling from within a subshell:
>
> ```
> (
>     command1 || exit 1
>     command2 || exit 1
>     exec command3
> ) > outputfile || exit 1
> echo "outputfile created successfully"
> ```

#### RATIONALE

> The behavior of [*exit*](#exit) when given an invalid argument or unknown option is unspecified, because of differing practices in the various historical implementations. A value larger than 255 might be truncated by the shell, and be unavailable even to a parent process that uses [*waitid*()](docs/posix/md/functions/waitid.md) to get the full exit value. It is recommended that implementations that detect any usage error should cause a non-zero exit status (or, if the shell is interactive and the error does not cause the shell to abort, store a non-zero value in `"$?"`), but even this was not done historically in all shells.
>
> See also [*C.2.8.2 Exit Status for Commands*](docs/posix/md/xrat/V4_xcu_chap01.md#c282-exit-status-for-commands).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0047 [717], XCU/TC2-2008/0048 [960], XCU/TC2-2008/0049 [717], and XCU/TC2-2008/0050 [960] are applied.

#### Issue 8

> Austin Group Defect 51 is applied, specifying the behavior when *n* has a value greater than 256 that corresponds to an exit status the shell assigns to commands terminated by a valid signal.
>
> Austin Group Defect 1029 is applied, changing "[*trap*](#trap)" to "[*trap*](#trap) action" in the DESCRIPTION section.
>
> Austin Group Defect 1309 is applied, changing the EXIT STATUS section.
>
> Austin Group Defect 1425 is applied, clarifying the requirements for a [*trap*](#trap) action on **EXIT**.
>
> Austin Group Defect 1602 is applied, clarifying the behavior of [*exit*](#exit) in a [*trap*](#trap) action.
>
> Austin Group Defect 1629 is applied, adding exit status 128 to the APPLICATION USAGE section.

*End of informative text.*

### Tests

#### Test: exit n sets explicit exit status

`exit n` causes the shell to exit with status `n`.

```
begin test "exit n sets explicit exit status"
  script
    exit 42
  expect
    stdout ""
    stderr ""
    exit_code 42
end test "exit n sets explicit exit status"
```

#### Test: exit inside subshell only terminates the subshell

In a subshell, `exit` exits only the subshell.

```
begin test "exit inside subshell only terminates the subshell"
  script
    (exit 99)
    echo "$?"
  expect
    stdout "99"
    stderr ""
    exit_code 0
end test "exit inside subshell only terminates the subshell"
```

#### Test: exit with no n uses last command exit status

When `n` is not specified, exit uses the current value of `$?`.

```
begin test "exit with no n uses last command exit status"
  script
    false
    exit
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "exit with no n uses last command exit status"
```

#### Test: EXIT trap fires on normal exit

A trap on EXIT fires when the shell terminates via `exit`.

```
begin test "EXIT trap fires on normal exit"
  script
    trap "echo exit_trap" EXIT
    exit 0
  expect
    stdout "exit_trap"
    stderr ""
    exit_code 0
end test "EXIT trap fires on normal exit"
```

#### Test: exit with value 200

The `exit` utility accepts values outside the 0-125 range. A value
of 200 is passed through as the subshell's exit status.

```
begin test "exit with value 200"
  script
    (exit 200)
    echo $?
  expect
    stdout "200"
    stderr ""
    exit_code 0
end test "exit with value 200"
```
