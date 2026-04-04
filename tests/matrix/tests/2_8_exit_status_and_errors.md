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

1. The shell shall exit only if the special built-in utility is executed directly. If it is executed via the [*command*](docs/posix/md/utilities/command.md) utility, the shell shall not exit.
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

#### Test: syntax error writes diagnostic to stderr

The shell shall write a diagnostic message to standard error when a shell
language syntax error is detected (table: diagnostic required yes).

```
begin test "syntax error writes diagnostic to stderr"
  script
    echo 'if true; echo wrong; fi' > tmp_syn_diag.sh
    $SHELL tmp_syn_diag.sh
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "syntax error writes diagnostic to stderr"
```

#### Test: syntax error in subshell exits subshell only

If a shall-exit error occurs in a subshell environment, the shell shall exit
from the subshell with a non-zero status and continue in the parent
environment. Here `eval` triggers a syntax error inside a subshell.

```
begin test "syntax error in subshell exits subshell only"
  script
    (eval 'if then') 2>/dev/null
    echo "parent survived"
  expect
    stdout "parent survived"
    stderr ""
    exit_code 0
end test "syntax error in subshell exits subshell only"
```

#### Test: interactive shell does not exit on syntax error

An interactive shell shall not exit on a syntax error but shall not perform
any further processing of the command in which the error occurred.

```
begin interactive test "interactive shell does not exit on syntax error"
  spawn -i
  expect "$ "
  send "fi"
  expect "$ "
  send "echo survived_syntax"
  expect "survived_syntax"
  sendeof
  wait
end interactive test "interactive shell does not exit on syntax error"
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

#### Test: special built-in diagnostic can be redirected

Although special built-ins are part of the shell, a diagnostic message written
by a special built-in is not considered a shell diagnostic message and can be
redirected like any other utility (Note 2).

```
begin test "special built-in diagnostic can be redirected"
  script
    echo 'set -Z 2>tmp_sbi_diag.txt' > tmp_sbi_redir.sh
    $SHELL tmp_sbi_redir.sh 2>/dev/null
    cat tmp_sbi_diag.txt
  expect
    stdout "(.|\n)+"
    stderr ""
    exit_code 0
end test "special built-in diagnostic can be redirected"
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

#### Test: interactive shell does not exit on special built-in error

An interactive shell shall not exit when a special built-in utility encounters
an error.

```
begin interactive test "interactive shell does not exit on special built-in error"
  spawn -i
  expect "$ "
  send "set -Z"
  expect "$ "
  send "echo survived_sbi"
  expect "survived_sbi"
  sendeof
  wait
end interactive test "interactive shell does not exit on special built-in error"
```

#### Test: other utility error does not cause non-interactive shell to exit

An error from a non-special-built-in utility shall not cause the shell to
exit. The shell is not required to write a diagnostic, but the utility itself
writes its own (Note 3).

```
begin test "other utility error does not cause non-interactive shell to exit"
  script
    ls /nonexistent_other_util_path_8z 2>/dev/null
    echo "survived"
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "other utility error does not cause non-interactive shell to exit"
```

#### Test: redirection error with special built-in causes exit

A redirection error on a special built-in utility shall cause a non-interactive
shell to exit with a diagnostic message.

```
begin test "redirection error with special built-in causes exit"
  script
    echo ': < /nonexistent_sbi_redir_9q; echo survived' > tmp_redir_sbi.sh
    $SHELL tmp_redir_sbi.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "redirection error with special built-in causes exit"
```

#### Test: redirection error with special built-in writes diagnostic to stderr

The shell shall write a diagnostic message to standard error for a redirection
error with a special built-in (table: diagnostic required yes). The shell also
exits with a non-zero status.

```
begin test "redirection error with special built-in writes diagnostic to stderr"
  script
    echo ': < /nonexistent_redir_sbi_diag; echo survived' > tmp_redir_sbi_diag.sh
    $SHELL tmp_redir_sbi_diag.sh
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "redirection error with special built-in writes diagnostic to stderr"
```

#### Test: redirection error with special built-in in subshell exits subshell only

If a redirection error on a special built-in occurs in a subshell, the
subshell shall exit with non-zero status and the parent shall continue.

```
begin test "redirection error with special built-in in subshell exits subshell only"
  script
    (: < /nonexistent_sub_redir_sbi) 2>/dev/null
    echo "parent survived"
  expect
    stdout "parent survived"
    stderr ""
    exit_code 0
end test "redirection error with special built-in in subshell exits subshell only"
```

#### Test: interactive shell does not exit on redirection error with special built-in

An interactive shell shall not exit on a redirection error with a special
built-in, even though a non-interactive shell would exit.

```
begin interactive test "interactive shell does not exit on redirection error with special built-in"
  spawn -i
  expect "$ "
  send ": < /nonexistent_interactive_redir"
  expect "$ "
  send "echo survived_redir_sbi"
  expect "survived_redir_sbi"
  sendeof
  wait
end interactive test "interactive shell does not exit on redirection error with special built-in"
```

#### Test: redirection error with compound command does not cause exit

A redirection error on a compound command shall not cause the shell to exit.
The shell shall write a diagnostic message (table: diagnostic required yes).

```
begin test "redirection error with compound command does not cause exit"
  script
    { echo hello; } < /nonexistent_cmpd_redir_7m 2>/dev/null
    echo "survived"
  expect
    stdout "survived"
    stderr "(.|\n)*"
    exit_code 0
end test "redirection error with compound command does not cause exit"
```

#### Test: redirection error with function execution does not cause exit

A redirection error during function execution shall not cause the shell to
exit.

```
begin test "redirection error with function execution does not cause exit"
  script
    redir_func() { echo hello; }
    redir_func < /nonexistent_func_redir_4k 2>/dev/null
    echo "survived"
  expect
    stdout "survived"
    stderr "(.|\n)*"
    exit_code 0
end test "redirection error with function execution does not cause exit"
```

#### Test: redirection error with other utility does not cause exit

A redirection error on a non-special-built-in utility shall not cause the
shell to exit.

```
begin test "redirection error with other utility does not cause exit"
  script
    cat < /nonexistent_util_redir_2j 2>/dev/null
    echo "survived"
  expect
    stdout "survived"
    stderr "(.|\n)*"
    exit_code 0
end test "redirection error with other utility does not cause exit"
```

#### Test: redirection error writes diagnostic to stderr

The shell shall write a diagnostic message to standard error for redirection
errors (table: diagnostic required yes for all redirection error types).

```
begin test "redirection error writes diagnostic to stderr"
  script
    cat < /nonexistent_redir_diag_5n
    echo "survived_diag"
  expect
    stdout "survived_diag"
    stderr ".+"
    exit_code 0
end test "redirection error writes diagnostic to stderr"
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

#### Test: variable assignment error writes diagnostic to stderr

The shell shall write a diagnostic message to standard error when a variable
assignment error occurs (table: diagnostic required yes).

```
begin test "variable assignment error writes diagnostic to stderr"
  script
    echo 'readonly V=1; V=2 :' > tmp_varerr_diag.sh
    $SHELL tmp_varerr_diag.sh
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "variable assignment error writes diagnostic to stderr"
```

#### Test: variable assignment error in subshell exits subshell only

If a variable assignment error occurs in a subshell, the subshell shall exit
with non-zero status and the parent shall continue.

```
begin test "variable assignment error in subshell exits subshell only"
  script
    (readonly V=1; V=2 :) 2>/dev/null
    echo "parent survived"
  expect
    stdout "parent survived"
    stderr ""
    exit_code 0
end test "variable assignment error in subshell exits subshell only"
```

#### Test: interactive shell does not exit on variable assignment error

An interactive shell shall not exit on a variable assignment error but shall
not perform any further processing of the command in which the error occurred.

```
begin interactive test "interactive shell does not exit on variable assignment error"
  spawn -i
  expect "$ "
  send "readonly IVAR_A=1"
  expect "$ "
  send "IVAR_A=2 :"
  expect "$ "
  send "echo survived_var"
  expect "survived_var"
  sendeof
  wait
end interactive test "interactive shell does not exit on variable assignment error"
```

#### Test: expansion error causes non-interactive shell to exit

An expansion error shall cause a non-interactive shell to exit. Here the
`:?` operator on an unset variable triggers an expansion-time error.

```
begin test "expansion error causes non-interactive shell to exit"
  script
    echo 'unset _EXP_V; echo "${_EXP_V:?}"; echo survived' > tmp_exp_exit.sh
    $SHELL tmp_exp_exit.sh 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "expansion error causes non-interactive shell to exit"
```

#### Test: expansion error writes diagnostic to stderr

The shell shall write a diagnostic message to standard error on an expansion
error (table: diagnostic required yes).

```
begin test "expansion error writes diagnostic to stderr"
  script
    echo 'unset _EXP_D; echo "${_EXP_D:?}"' > tmp_exp_diag.sh
    $SHELL tmp_exp_diag.sh
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "expansion error writes diagnostic to stderr"
```

#### Test: expansion error in subshell exits subshell only

If an expansion error occurs in a subshell environment, the subshell shall
exit with a non-zero status and the parent shall continue.

```
begin test "expansion error in subshell exits subshell only"
  script
    unset _NOSUB 2>/dev/null
    (: "${_NOSUB:?sub_err}") 2>/dev/null
    echo "parent survived"
  expect
    stdout "parent survived"
    stderr ""
    exit_code 0
end test "expansion error in subshell exits subshell only"
```

#### Test: interactive shell does not exit on expansion error

An interactive shell shall not exit on an expansion error.

```
begin interactive test "interactive shell does not exit on expansion error"
  spawn -i
  expect "$ "
  send "echo ${}"
  expect "$ "
  send "echo survived_exp"
  expect "survived_exp"
  sendeof
  wait
end interactive test "interactive shell does not exit on expansion error"
```

#### Test: command not found writes diagnostic to stderr

The shell shall write a diagnostic message when a command is not found
(table: diagnostic required yes).

```
begin test "command not found writes diagnostic to stderr"
  script
    nonexistent_cmd_diag_3x
    echo "survived_diag"
  expect
    stdout "survived_diag"
    stderr ".+"
    exit_code 0
end test "command not found writes diagnostic to stderr"
```

#### Test: missing command does not cause shell to exit

The table allows a non-interactive shell to optionally exit when a command is
not found ("may exit"). This test verifies the shell can continue execution,
which is the permitted behavior in `bash --posix`.

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

#### Test: interactive shell does not exit on command not found

An interactive shell shall not exit when a command is not found (table:
interactive "shall not exit").

```
begin interactive test "interactive shell does not exit on command not found"
  spawn -i
  expect "$ "
  send "nonexistent_cmd_interactive_7q"
  expect "$ "
  send "echo survived_notfound"
  expect "survived_notfound"
  sendeof
  wait
end interactive test "interactive shell does not exit on command not found"
```

## 2.8.2 Exit Status for Commands

Each command has an exit status that can influence the behavior of other shell commands. The exit status of commands that are not utilities is documented in this section. The exit status of the standard utilities is documented in their respective sections.

The exit status of a command shall be determined as follows:

- If the command is not found, the exit status shall be 127.
- Otherwise, if the command name is found, but it is not an executable utility, the exit status shall be 126.
- Otherwise, if the command terminated due to the receipt of a signal, the shell shall assign it an exit status greater than 128. The exit status shall identify, in an implementation-defined manner, which signal terminated the command. Note that shell implementations are permitted to assign an exit status greater than 255 if a command terminates due to a signal.
- Otherwise, the exit status shall be the value obtained by the equivalent of the WEXITSTATUS macro applied to the status obtained by the [*wait*()](docs/posix/md/functions/wait.md) function (as defined in the System Interfaces volume of POSIX.1-2024). Note that for C programs, this value is equal to the result of performing a modulo 256 operation on the value passed to [*_Exit*()](docs/posix/md/functions/_Exit.md), [*_exit*()](docs/posix/md/functions/_exit.md), or [*exit*()](docs/posix/md/functions/exit.md) or returned from *main*().

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

#### Test: signal exit status encodes the signal number

The exit status of a signal-terminated command shall identify which signal
caused the termination. Bash encodes this as 128 plus the signal number;
SIGTERM (signal 15) produces exit status 143.

```
begin test "signal exit status encodes the signal number"
  script
    $SHELL -c 'kill -TERM $$'
    echo $?
  expect
    stdout "143"
    stderr ""
    exit_code 0
end test "signal exit status encodes the signal number"
```

#### Test: normal exit status reflects WEXITSTATUS value

The exit status of a normally terminated command is the low-order 8 bits of
the value passed to `exit`, equivalent to applying WEXITSTATUS to the
`wait()` status.

```
begin test "normal exit status reflects WEXITSTATUS value"
  script
    $SHELL -c 'exit 42'
    r1=$?
    $SHELL -c 'exit 300'
    r2=$?
    echo "$r1 $r2"
  expect
    stdout "42 44"
    stderr ""
    exit_code 0
end test "normal exit status reflects WEXITSTATUS value"
```
