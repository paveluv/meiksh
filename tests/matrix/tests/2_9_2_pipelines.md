# Test Suite for 2.9.2 Pipelines

This test suite covers **Section 2.9.2 Pipelines** of the POSIX.1-2024
Shell Command Language specification (part of 2.9 Shell Commands).

## Table of contents

- [2.9.2 Pipelines](#292-pipelines)

## 2.9.2 Pipelines

A *pipeline* is a sequence of one or more commands separated by the control operator `'|'`. For each command but the last, the shell shall connect the standard output of the command to the standard input of the next command as if by creating a pipe and passing the write end of the pipe as the standard output of the command and the read end of the pipe as the standard input of the next command.

The format for a pipeline is:

```
[!] command1 [ | command2 ...]
```

If the pipeline begins with the reserved word **!** and *command1* is a subshell command, the application shall ensure that the **(** operator at the beginning of *command1* is separated from the **!** by one or more `<blank>` characters. The behavior of the reserved word **!** immediately followed by the **(** operator is unspecified.

The standard output of *command1* shall be connected to the standard input of *command2*. The standard input, standard output, or both of a command shall be considered to be assigned by the pipeline before any redirection specified by redirection operators that are part of the command (see [2.7 Redirection](#27-redirection)).

If the pipeline is not in the background (see [2.9.3.1 Asynchronous AND-OR Lists](#2931-asynchronous-and-or-lists) and [2.11 Job Control](#211-job-control)), the shell shall wait for the last command specified in the pipeline to complete, and may also wait for all commands to complete.

##### Exit Status

The exit status of a pipeline shall depend on whether or not the *pipefail* option (see [set](#tag_19_26)) is enabled and whether or not the pipeline begins with the **!** reserved word, as described in the following table. The *pipefail* option determines which command in the pipeline the exit status is derived from; the **!** reserved word causes the exit status to be the logical NOT of the exit status of that command. The shell shall use the *pipefail* setting at the time it begins execution of the pipeline, not the setting at the time it sets the exit status of the pipeline. (For example, in `command1 | set -o pipefail` the exit status of `command1` has no effect on the exit status of the pipeline, even if the shell executes `set -o pipefail` in the current shell environment.)

| **pipefail Enabled** | **Begins with !** | **Exit Status** |
| --- | --- | --- |
| no | no | The exit status of the last (rightmost) command specified in the pipeline. |
| no | yes | Zero, if the last (rightmost) command in the pipeline returned a non-zero exit status; otherwise, 1. |
| yes | no | Zero, if all commands in the pipeline returned an exit status of 0; otherwise, the exit status of the last (rightmost) command specified in the pipeline that returned a non-zero exit status. |
| yes | yes | Zero, if any command in the pipeline returned a non-zero exit status; otherwise, 1. |

### Tests

#### Test: basic pipe stdout flows into stdin

The shell connects stdout of the first command to stdin of the next command
via a pipe.

```
begin test "basic pipe stdout flows into stdin"
  script
    echo "hello pipe" | tr "p" "t"
  expect
    stdout "hello tite"
    stderr ""
    exit_code 0
end test "basic pipe stdout flows into stdin"
```

#### Test: pipeline exit status is last command

Without pipefail, the exit status of a pipeline is the exit status of the
last command.

```
begin test "pipeline exit status is last command"
  script
    false | true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "pipeline exit status is last command"
```

#### Test: pipeline assignments happen before redirections

Pipeline assignments to stdin/stdout occur before explicit redirections
specified by redirection operators.

```
begin test "pipeline assignments happen before redirections"
  script
    echo "pipeline test" | cat > tmp_pipe.txt
    cat tmp_pipe.txt
  expect
    stdout "pipeline test"
    stderr ""
    exit_code 0
end test "pipeline assignments happen before redirections"
```

#### Test: ! ( false ) succeeds with blank separation

The `!` reserved word negates the exit status; when command1 is a subshell,
`!` and `(` must be separated by a blank.

```
begin test "! ( false ) succeeds with blank separation"
  script
    ! ( false )
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "! ( false ) succeeds with blank separation"
```

#### Test: errexit does not exit on ! pipeline

When `set -e` is active, a pipeline prefixed with `!` does not trigger
an exit, even if the underlying command fails.

```
begin test "errexit does not exit on ! pipeline"
  script
    set -e
    ! false
    echo "survived_not"
  expect
    stdout "survived_not"
    stderr ""
    exit_code 0
end test "errexit does not exit on ! pipeline"
```
