# Test Suite for 2.15 Special Built-In: trap

This test suite covers the **trap** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities trap](#215-special-built-in-utilities-trap)

## 2.15 Special Built-In Utilities trap

#### NAME

> trap — trap signals

#### SYNOPSIS

> ```
> trap n [condition...]
> trap -p [condition...]
> trap [action condition...]
> ```

#### DESCRIPTION

> ```
> save_traps=$(trap -p)
> ...
> eval "$save_traps"
> ```

#### OPTIONS

> The following option shall be supported:
>
> - **-p**: Write to standard output a list of commands associated with each *condition* operand. The behavior when there are no operands is specified in the DESCRIPTION section. The shell shall format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same trapping results for the specified set of conditions. If a *condition* operand is a condition corresponding to the SIGKILL or SIGSTOP signal, and [*trap*](#trap) **-p** without any operands would not include it in the set of conditions for which it writes output, the behavior is undefined if the output is reinput to the shell.

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

> See the DESCRIPTION.

#### STDERR

> The standard error shall be used only for diagnostic messages and warning messages about invalid signal names or numbers.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> If the trap name or number is invalid, a non-zero exit status shall be returned; otherwise, zero shall be returned. For both interactive and non-interactive shells, invalid signal names or numbers shall not be considered an error and shall not cause the shell to abort.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> When the **-p** option is not used, since [*trap*](#trap) with no operands does not output commands to restore traps that are currently set to default, these need to be restored separately. The RATIONALE section shows examples and describes their drawbacks.

#### EXAMPLES

> Write out a list of all traps and actions:
>
> ```
> trap
> ```
>
> Set a trap so the *logout* utility in the directory referred to by the *HOME* environment variable executes when the shell terminates:
>
> ```
> trap '"$HOME"/logout' EXIT
> ```
>
> or:
>
> ```
> trap '"$HOME"/logout' 0
> ```
>
> Unset traps on INT, QUIT, TERM, and EXIT:
>
> ```
> trap - INT QUIT TERM EXIT
> ```

#### RATIONALE

> ```
> save_traps=$(trap)
> trap "some command" INT QUIT
> save_traps="trap - INT QUIT; $save_traps"
> ...
> eval "$save_traps"
> ```

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)
>
> XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), [*\<signal.h\>*](docs/posix/md/basedefs/signal.h.md)

#### CHANGE HISTORY

#### Issue 6

> XSI-conforming implementations provide the mapping of signal names to numbers given above (previously this had been marked obsolescent). Other implementations need not provide this optional mapping.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> Austin Group Interpretation 1003.1-2001 #116 is applied.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0052 [53,268,440], XCU/TC1-2008/0053 [53,268,440], XCU/TC1-2008/0054 [163], XCU/TC1-2008/0055 [163], and XCU/TC1-2008/0056 [163] are applied.

#### Issue 8

> Austin Group Defect 621 is applied, clarifying when the EXIT condition occurs.
>
> Austin Group Defect 1029 is applied, clarifying the execution of [*trap*](#trap) actions.
>
> Austin Group Defects 1211 and 1212 are applied, adding the **-p** option and clarifying that, when **-p** is not specified, the output of [*trap*](#trap) with no operands does not list conditions that are in the default state.
>
> Austin Group Defect 1265 is applied, updating the DESCRIPTION, STDERR and EXIT STATUS sections to align with the changes made to [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) between Issue 6 and Issue 7.
>
> Austin Group Defect 1285 is applied, inserting a blank line between the two SYNOPSIS lines.

*End of informative text.*

### Tests

#### Test: trap action executed on signal

`trap action condition` causes `action` to be executed when the
specified condition arises.

```
begin test "trap action executed on signal"
  script
    trap "echo GOT_USR1" USR1
    kill -USR1 $$
    echo done
  expect
    stdout "GOT_USR1\ndone"
    stderr ""
    exit_code 0
end test "trap action executed on signal"
```

#### Test: trap - resets trap to default

`trap - condition` resets the condition to its default action.

```
begin test "trap - resets trap to default"
  script
    trap "echo x" EXIT
    trap - EXIT
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "trap - resets trap to default"
```

#### Test: trap with empty action ignores condition

`trap "" condition` causes the shell to ignore the condition.

```
begin test "trap with empty action ignores condition"
  script
    trap "" EXIT
    echo "running"
  expect
    stdout "running"
    stderr ""
    exit_code 0
end test "trap with empty action ignores condition"
```

#### Test: trap action fires on EXIT

A trap on EXIT fires when the shell terminates normally.

```
begin test "trap action fires on EXIT"
  script
    trap "echo exit_action" EXIT
    echo "main"
  expect
    stdout "main\nexit_action"
    stderr ""
    exit_code 0
end test "trap action fires on EXIT"
```

#### Test: trap overrides previous action

Setting a new trap on the same condition replaces the previous action.

```
begin test "trap overrides previous action"
  script
    trap "echo first" EXIT
    trap "echo second" EXIT
    true
  expect
    stdout "second"
    stderr ""
    exit_code 0
end test "trap overrides previous action"
```

#### Test: trap preserves $? value

The value of `$?` after a trap action completes is the value it had
before the trap was executed.

```
begin test "trap preserves $? value"
  script
    trap 'echo qval=$?' EXIT
    false
  expect
    stdout "qval=[1-9][0-9]*"
    stderr ""
    exit_code !=0
end test "trap preserves $? value"
```

#### Test: trap with signal number resets to default

When the first operand is an unsigned decimal integer and `-p` is not
specified, all operands are treated as conditions to be reset.

```
begin test "trap with signal number resets to default"
  script
    trap "echo trapped" INT
    trap 2
    trap -p INT | grep -c 'echo trapped' || true
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "trap with signal number resets to default"
```

#### Test: trap with no args lists active traps

`trap` with no operands writes commands associated with active traps.

```
begin test "trap with no args lists active traps"
  script
    trap "echo hi" INT
    trap
  expect
    stdout "trap -- .*echo hi.* INT"
    stderr ""
    exit_code 0
end test "trap with no args lists active traps"
```

#### Test: trap output is suitable for reinput

The output of `trap -p` is formatted so it can be reinput to the shell
to achieve the same trapping results.

```
begin test "trap output is suitable for reinput"
  script
    trap "echo reinput_trap" EXIT
    _saved=$(trap -p EXIT)
    trap - EXIT
    eval "$_saved"
    true
  expect
    stdout "reinput_trap"
    stderr ""
    exit_code 0
end test "trap output is suitable for reinput"
```

#### Test: subshell resets traps to default

When a subshell is entered, non-ignored traps are set to default.

```
begin test "subshell resets traps to default"
  script
    trap "echo parent_trap" USR1
    (trap "echo child_trap" USR1; trap -p USR1)
  expect
    stdout "trap -- .*child_trap.* USR1"
    stderr ""
    exit_code 0
end test "subshell resets traps to default"
```

#### Test: subshell inherits trap list before entry

`trap` in a subshell reports the commands that were associated with
conditions immediately before the subshell was entered.

```
begin test "subshell inherits trap list before entry"
  script
    trap "echo exit_trap" 0
    ( trap )
  expect
    stdout "trap -- .*exit_trap.* (EXIT|0)\nexit_trap"
    stderr ""
    exit_code 0
end test "subshell inherits trap list before entry"
```

#### Test: EXIT trap sees latest environment

When an EXIT trap fires, it sees the current values of variables,
including any changes made after the trap was set.

```
begin test "EXIT trap sees latest environment"
  script
    MYVAL=hello
    trap 'echo $MYVAL' EXIT
    MYVAL=world
  expect
    stdout "world"
    stderr ""
    exit_code 0
end test "EXIT trap sees latest environment"
```

#### Test: trap persists across commands

A trap, once set, remains in effect across subsequent commands
until explicitly reset.

```
begin test "trap persists across commands"
  script
    trap "echo still_set" EXIT
    echo "first_command"
    echo "second_command"
  expect
    stdout "first_command\nsecond_command\nstill_set"
    stderr ""
    exit_code 0
end test "trap persists across commands"
```

#### Test: trap -p shows trap -- format

`trap -p` displays the trap action in a format suitable for
reinput, using the `trap --` prefix.

```
begin test "trap -p shows trap -- format"
  script
    trap "echo caught" INT
    trap -p INT
  expect
    stdout "trap -- .*INT.*"
    stderr ""
    exit_code 0
end test "trap -p shows trap -- format"
```

#### Test: trap -p lists all traps

When given no condition arguments, `trap -p` lists all active
traps.

```
begin test "trap -p lists all traps"
  script
    trap "echo a" INT
    trap "echo b" TERM
    trap -p
  expect
    stdout "(.|\n)*trap -- .*INT(.|\n)*trap -- .*TERM(.|\n)*"
    stderr ""
    exit_code 0
end test "trap -p lists all traps"
```

#### Test: trap - resets EXIT trap

Setting the action to `-` for EXIT resets it to the default
(no action), so no trap fires on exit.

```
begin test "trap - resets EXIT trap"
  script
    trap "echo bad" EXIT
    trap - EXIT
    exit 0
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "trap - resets EXIT trap"
```

#### Test: trap with valid signal returns zero

Setting a trap on a valid signal name succeeds with exit status 0.

```
begin test "trap with valid signal returns zero"
  script
    trap "" INT
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "trap with valid signal returns zero"
```

#### Test: trap with invalid signal returns non-zero

Setting a trap on an invalid signal name produces an error and
the exit status of the trap command is non-zero.

```
begin test "trap with invalid signal returns non-zero"
  script
    trap "" INVALID_SIGNAL_NAME_XYZ 2>/dev/null
    echo "rc=$?"
  expect
    stdout "rc=[1-9][0-9]*"
    stderr ""
    exit_code 0
end test "trap with invalid signal returns non-zero"
```

#### Test: shell survives invalid signal name

An invalid signal name in a `trap` command produces a diagnostic
but does not cause the shell to abort.

```
begin test "shell survives invalid signal name"
  script
    trap "" INVALID_SIGNAL_NAME_XYZ 2>/dev/null
    echo survived
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "shell survives invalid signal name"
```

#### Test: trap with invalid signal produces diagnostic

When an invalid signal name is given, a diagnostic message is
written to standard error.

```
begin test "trap with invalid signal produces diagnostic"
  script
    trap "" NOSUCHSIGNAL 2>&1 || true
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "trap with invalid signal produces diagnostic"
```
