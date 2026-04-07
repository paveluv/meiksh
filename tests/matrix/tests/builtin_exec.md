# Test Suite for 2.15 Special Built-In: exec

This test suite covers the **exec** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities exec](#215-special-built-in-utilities-exec)

## 2.15 Special Built-In Utilities exec

#### NAME

> exec — perform redirections in the current shell or execute a utility

#### SYNOPSIS

> `exec [utility [argument...]]`

#### DESCRIPTION

> If [*exec*](#exec) is specified with no operands, any redirections associated with the [*exec*](#exec) command shall be made in the current shell execution environment. If any file descriptors with numbers greater than 2 are opened by those redirections, it is unspecified whether those file descriptors remain open when the shell invokes another utility. Scripts concerned that child shells could misuse open file descriptors can always close them explicitly, as shown in one of the following examples. If the result of the redirections would be that file descriptor 0, 1, or 2 is closed, implementations may open the file descriptor to an unspecified file.
>
> If [*exec*](#exec) is specified with a *utility* operand, the shell shall execute a non-built-in utility as described in [2.9.1.6 Non-built-in Utility Execution](#2916-non-built-in-utility-execution) with *utility* as the command name and the *argument* operands (if any) as the command arguments.
>
> If the [*exec*](#exec) command fails, a non-interactive shell shall exit from the current shell execution environment; an interactive shell may exit from a subshell environment but shall not exit if the current shell environment is not a subshell environment.
>
> If the [*exec*](#exec) command fails and the shell does not exit, any redirections associated with the [*exec*](#exec) command that were successfully made shall take effect in the current shell execution environment.
>
> The [*exec*](#exec) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).

#### OPTIONS

> None.

#### OPERANDS

> See the DESCRIPTION.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variable shall affect the execution of [*exec*](#exec):
>
> - *PATH*: Determine the search path when looking for the utility given as the *utility* operand; see XBD [*8.3 Other Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#83-other-environment-variables).

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

> If *utility* is specified and is executed, [*exec*](#exec) shall not return to the shell; rather, the exit status of the current shell execution environment shall be the exit status of *utility*. If *utility* is specified and an attempt to execute it as a non-built-in utility fails, the exit status shall be as described in [2.9.1.6 Non-built-in Utility Execution](#2916-non-built-in-utility-execution). If a redirection error occurs (see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)), the exit status shall be a value in the range 1-125. Otherwise, [*exec*](#exec) shall return a zero exit status.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> Open *readfile* as file descriptor 3 for reading:
>
> ```
> exec 3< readfile
> ```
>
> Open *writefile* as file descriptor 4 for writing:
>
> ```
> exec 4> writefile
> ```
>
> Make file descriptor 5 a copy of file descriptor 0:
>
> ```
> exec 5<&0
> ```
>
> Close file descriptor 3:
>
> ```
> exec 3<&-
> ```
>
> Cat the file **maggie** by replacing the current shell with the [*cat*](docs/posix/md/utilities/cat.md) utility:
>
> ```
> exec cat maggie
> ```
>
> An application that is not concerned with strict conformance can make use of optional `%g` support known to be present in the implementation's [*printf*](docs/posix/md/utilities/printf.md) utility by ensuring that any shell built-in version is not executed instead, and using a subshell so that the shell continues afterwards:
>
> ```
> (exec printf '%g\n' "$float_value")
> ```

#### RATIONALE

> Most historical implementations were not conformant in that:
>
> ```
> foo=bar exec cmd
> ```
>
> did not pass **foo** to **cmd**.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 252 is applied, adding a requirement for [*exec*](#exec) to support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> Austin Group Defect 1157 is applied, clarifying the execution of non-built-in utilities.
>
> Austin Group Defect 1587 is applied, changing the ENVIRONMENT VARIABLES section.

*End of informative text.*

### Tests

#### Test: exec with operands replaces the shell

`exec` with a utility operand replaces the shell process.

```
begin test "exec with operands replaces the shell"
  script
    exec printf "%s" "exec test"
    echo "never runs"
  expect
    stdout "exec test"
    stderr ""
    exit_code 0
end test "exec with operands replaces the shell"
```

#### Test: exec prefix assignment is visible to the new program

Assignments that precede `exec` are part of the simple command; the
replaced utility runs with those variables in its environment.

```
begin test "exec prefix assignment is visible to the new program"
  script
    MY_EXEC_VAR=42 exec sh -c 'printf "%s\n" "$MY_EXEC_VAR"'
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "exec prefix assignment is visible to the new program"
```

#### Test: exec with no operands manipulates file descriptors

Without operands, `exec` applies redirections to the current shell.

```
begin test "exec with no operands manipulates file descriptors"
  script
    exec 3>tmp_fd3.txt
    echo "fd3 test" >&3
    exec 3>&-
    cat tmp_fd3.txt
  expect
    stdout "fd3 test"
    stderr ""
    exit_code 0
end test "exec with no operands manipulates file descriptors"
```

#### Test: exec of nonexistent command causes non-interactive shell to exit

If exec fails in a non-interactive shell, the shell exits.

```
begin test "exec of nonexistent command causes non-interactive shell to exit"
  script
    exec /invalid/does/not/exist
    echo "survived"
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "exec of nonexistent command causes non-interactive shell to exit"
```

#### Test: exec with no utility returns 0

When `exec` is invoked with no command or arguments (only
redirections or nothing), the shell continues execution and the
exit status is 0.

```
begin test "exec with no utility returns 0"
  script
    exec
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "exec with no utility returns 0"
```

#### Test: exec uses PATH to find utility

The `exec` utility shall search `PATH` for the utility when the
operand does not contain a slash.

```
begin test "exec uses PATH to find utility"
  script
    exec printf '%s\n' "found_via_path"
  expect
    stdout "found_via_path"
    stderr ""
    exit_code 0
end test "exec uses PATH to find utility"
```

#### Test: exec failure redirections persist in interactive subshell

If exec fails and the shell does not exit, any redirections that were
successfully made shall take effect. Since a non-interactive shell
exits on exec failure, this test uses a subshell to capture behavior.

```
begin test "exec failure redirections persist in interactive subshell"
  script
    (exec 3>tmp_exec_redir.txt; echo "fd3" >&3; exec 3>&-; cat tmp_exec_redir.txt)
    rm -f tmp_exec_redir.txt
  expect
    stdout "fd3"
    stderr ""
    exit_code 0
end test "exec failure redirections persist in interactive subshell"
```
