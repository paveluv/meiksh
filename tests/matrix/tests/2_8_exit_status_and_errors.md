# Test Suite for 2.8 Exit Status and Errors

This test suite covers **Section 2.8 Exit Status and Errors** of the
POSIX.1-2024 Shell Command Language specification, including both subsections:
Consequences of Shell Errors and Exit Status for Commands.

## Table of contents

- [2.8 Exit Status and Errors](#28-exit-status-and-errors)
- [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)
- [2.8.2 Exit Status for Commands](#282-exit-status-for-commands)

## 2.8 Exit Status and Errors

## 2.8.1 Consequences of Shell Errors

Certain errors shall cause the shell to write a diagnostic message to standard error and exit as shown in the following table:

| **Error** | **Non-Interactive Shell** | **Interactive Shell** | **Shell Diagnostic Message Required** |
| --- | --- | --- | --- |
| Shell language syntax error | shall exit | shall not exit | yes |
| Special built-in utility error | shall exit1 | shall not exit | no2 |
| Other utility (not a special built-in) error | shall not exit | shall not exit | no3 |
| Redirection error with special built-in utilities | shall exit | shall not exit | yes |
| Redirection error with compound commands | shall not exit | shall not exit | yes |
| Redirection error with function execution | shall not exit | shall not exit | yes |
| Redirection error with other utilities (not special built-ins) | shall not exit | shall not exit | yes |
| Variable assignment error | shall exit | shall not exit | yes |
| Expansion error | shall exit | shall not exit | yes |
| Command not found | may exit | shall not exit | yes |
| Unrecoverable read error when reading commands | shall exit4 | shall exit4 | yes |

Notes:

1. The shell shall exit only if the special built-in utility is executed directly. If it is executed via the [*command*](../utilities/command.md) utility, the shell shall not exit.
2. Although special built-ins are part of the shell, a diagnostic message written by a special built-in is not considered to be a shell diagnostic message, and can be redirected like any other utility.
3. The shell is not required to write a diagnostic message, but the utility itself shall write a diagnostic message if required to do so.
4. If an unrecoverable read error occurs when reading commands, other than from the *file* operand of the [*dot*](#dot) special built-in, the shell shall execute no further commands (including any already successfully read but not yet executed) other than any specified in a previously defined EXIT [*trap*](#trap) action. An unrecoverable read error while reading from the *file* operand of the [*dot*](#dot) special built-in shall be treated as a special built-in utility error.

An expansion error is one that occurs when the shell expansions defined in [2.6 Word Expansions](#26-word-expansions) are carried out (for example, `"${x!y}"`, because `'!'` is not a valid operator); an implementation may treat these as syntax errors if it is able to detect them during tokenization, rather than during expansion.

If any of the errors shown as "shall exit" or "may exit" occur in a subshell environment, the shell shall (respectively, may) exit from the subshell environment with a non-zero status and continue in the environment from which that subshell environment was invoked.

In all of the cases shown in the table where an interactive shell is required not to exit and a non-interactive shell is required to exit, an interactive shell shall not perform any further processing of the command in which the error occurred.

### Tests

#### Test: syntax error causes non-interactive shell to exit

A non-interactive shell shall exit on a shell language syntax error. A script
containing `if true; echo ...` (missing `then`) is invalid syntax and causes
the shell to exit with a non-zero status.

```
begin test "syntax error causes non-interactive shell to exit"
  script
    echo 'if true; echo "no_then"; fi; echo "survived"' > tmp_err1.sh
    $SHELL tmp_err1.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "syntax error causes non-interactive shell to exit"
```

#### Test: syntax error prevents further execution

When a syntax error is detected, the shell shall not perform any further
processing of the command in which the error occurred. The `echo "survived"`
after the syntax error must not execute.

```
begin test "syntax error prevents further execution"
  script
    echo 'if true; echo "no_then"; fi; echo "survived"' > tmp_err1.sh
    $SHELL tmp_err1.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "syntax error prevents further execution"
```

#### Test: readonly assignment before special built-in causes exit

A variable assignment error (assigning to a readonly variable) preceding a
special built-in utility shall cause a non-interactive shell to exit.

```
begin test "readonly assignment before special built-in causes exit"
  script
    echo 'readonly RO_VAR=1; RO_VAR=2 export OTHER=3; echo "survived"' > tmp_err2.sh
    $SHELL tmp_err2.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "readonly assignment before special built-in causes exit"
```

#### Test: invalid option on special built-in causes exit

An error on a special built-in utility (such as `set -Z`, an invalid option)
shall cause a non-interactive shell to exit.

```
begin test "invalid option on special built-in causes exit"
  script
    echo 'set -Z; echo "survived"' > tmp_err3.sh
    $SHELL tmp_err3.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "invalid option on special built-in causes exit"
```

#### Test: command utility prevents exit on special built-in error

The shell shall exit on a special built-in error only if the built-in is
executed directly. When executed via the `command` utility, the shell shall
not exit, and subsequent commands continue to run.

```
begin test "command utility prevents exit on special built-in error"
  script
    echo 'command set -Z; echo "survived"' > tmp_err4.sh
    $SHELL tmp_err4.sh 2>/dev/null
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "command utility prevents exit on special built-in error"
```

#### Test: special built-in error in subshell only terminates subshell

If a "shall exit" error occurs in a subshell environment, the shell shall exit
from the subshell with a non-zero status but continue in the parent
environment.

```
begin test "special built-in error in subshell only terminates subshell"
  script
    ( set -Z ) 2>/dev/null
    echo "parent survived"
  expect
    stdout "parent survived"
    stderr ""
    exit_code 0
end test "special built-in error in subshell only terminates subshell"
```

#### Test: missing command does not cause shell to exit

An "other utility" error (command not found) shall not cause a non-interactive
shell to exit. The shell continues executing subsequent commands.

```
begin test "missing command does not cause shell to exit"
  script
    missing_command_123 2>/dev/null
    echo "survived missing command"
  expect
    stdout "survived missing command"
    stderr ""
    exit_code 0
end test "missing command does not cause shell to exit"
```

## 2.8.2 Exit Status for Commands

Each command has an exit status that can influence the behavior of other shell commands. The exit status of commands that are not utilities is documented in this section. The exit status of the standard utilities is documented in their respective sections.

The exit status of a command shall be determined as follows:

- If the command is not found, the exit status shall be 127.
- Otherwise, if the command name is found, but it is not an executable utility, the exit status shall be 126.
- Otherwise, if the command terminated due to the receipt of a signal, the shell shall assign it an exit status greater than 128. The exit status shall identify, in an implementation-defined manner, which signal terminated the command. Note that shell implementations are permitted to assign an exit status greater than 255 if a command terminates due to a signal.
- Otherwise, the exit status shall be the value obtained by the equivalent of the WEXITSTATUS macro applied to the status obtained by the [*wait*()](../functions/wait.md) function (as defined in the System Interfaces volume of POSIX.1-2024). Note that for C programs, this value is equal to the result of performing a modulo 256 operation on the value passed to [*_Exit*()](../functions/_Exit.md), [*_exit*()](../functions/_exit.md), or [*exit*()](../functions/exit.md) or returned from *main*().

### Tests

#### Test: non-existent command returns 127

If the command is not found, the exit status shall be 127.

```
begin test "non-existent command returns 127"
  script
    this_command_does_not_exist_xyz123
    echo "$?"
  expect
    stdout "127"
    stderr ".+"
    exit_code 0
end test "non-existent command returns 127"
```

#### Test: non-executable file returns 126

If the command name is found but it is not an executable utility, the exit
status shall be 126.

```
begin test "non-executable file returns 126"
  script
    touch tmp_not_exec
    chmod -x tmp_not_exec
    ./tmp_not_exec
    echo "$?"
  expect
    stdout "126"
    stderr ".+"
    exit_code 0
end test "non-executable file returns 126"
```

#### Test: signal-terminated process has exit status > 128

If the command terminated due to the receipt of a signal, the shell shall
assign it an exit status greater than 128.

```
begin test "signal-terminated process has exit status > 128"
  script
    sleep 10 &
    pid=$!
    kill -9 $pid >/dev/null 2>&1
    wait $pid
    status=$?
    [ $status -gt 128 ] && echo "signal_exit"
  expect
    stdout "signal_exit"
    stderr ""
    exit_code 0
end test "signal-terminated process has exit status > 128"
```
