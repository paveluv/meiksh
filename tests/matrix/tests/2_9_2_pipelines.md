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

#### Test: multi-stage pipeline connects all commands

The shell shall connect stdout of each command to stdin of the next, supporting
pipelines with three or more stages.

```
begin test "multi-stage pipeline connects all commands"
  script
    echo abc | tr a x | tr c z
  expect
    stdout "xbz"
    stderr ""
    exit_code 0
end test "multi-stage pipeline connects all commands"
```

#### Test: pipeline only connects stdout not stderr

The shell shall connect the standard output of the command to the standard
input of the next command. Standard error is not part of this connection and
passes through to the overall stderr stream.

```
begin test "pipeline only connects stdout not stderr"
  script
    (echo "stdout_data"; echo "stderr_data" >&2) | cat
  expect
    stdout "stdout_data"
    stderr "stderr_data"
    exit_code 0
end test "pipeline only connects stdout not stderr"
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

#### Test: pipeline exit status follows failing last command

Without pipefail, the exit status of a pipeline is the exit status of the
last command even when an earlier command succeeds.

```
begin test "pipeline exit status follows failing last command"
  script
    true | false
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "pipeline exit status follows failing last command"
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

#### Test: pipeline stdin redirection overrides pipe assignment

The standard input of a command shall be considered to be assigned by the
pipeline before any redirection specified by redirection operators. A later
input redirection on the receiving command overrides the pipe.

```
begin test "pipeline stdin redirection overrides pipe assignment"
  script
    echo "from_redirect" > tmp_redir_override.txt
    echo "from_pipe" | cat < tmp_redir_override.txt
  expect
    stdout "from_redirect"
    stderr ""
    exit_code 0
end test "pipeline stdin redirection overrides pipe assignment"
```

#### Test: pipeline sender stdout redirect overrides pipe

The standard output of a command shall be assigned by the pipeline before any
redirection operators. A `>` redirect on the sending command overrides the
pipe, so the receiver gets no data through the pipe.

```
begin test "pipeline sender stdout redirect overrides pipe"
  script
    echo "to_file" > tmp_stdout_override.txt | cat
    cat tmp_stdout_override.txt
  expect
    stdout "to_file"
    stderr ""
    exit_code 0
end test "pipeline sender stdout redirect overrides pipe"
```

#### Test: middle pipeline command redirections override both pipe ends

If a middle pipeline command redirects both its standard input and standard
output, those redirections override both pipeline assignments after the pipe
connections are set up.

```
begin test "middle pipeline command redirections override both pipe ends"
  script
    printf 'from_file\n' > tmp_middle_pipe.txt
    printf 'from_left\n' | cat < tmp_middle_pipe.txt > tmp_middle_out.txt | wc -c
    printf 'out:'
    cat tmp_middle_out.txt
  expect
    stdout "0\nout:from_file"
    stderr ""
    exit_code 0
end test "middle pipeline command redirections override both pipe ends"
```

#### Test: foreground pipeline waits for last command to complete

For a foreground pipeline, the shell shall wait for the last command in the
pipeline to complete before executing the next command.

```
begin test "foreground pipeline waits for last command to complete"
  script
    rm -f tmp_pipeline_done.txt
    printf x | { sleep 0.2; cat >/dev/null; echo done > tmp_pipeline_done.txt; }
    if test -f tmp_pipeline_done.txt; then
      echo waited
    else
      echo early
    fi
  expect
    stdout "waited"
    stderr ""
    exit_code 0
end test "foreground pipeline waits for last command to complete"
```

#### Test: background pipeline does not wait

If the pipeline is in the background, the shell does not wait for it to
complete before continuing to the next command.

```
begin test "background pipeline does not wait"
  script
    rm -f tmp_bg_pipeline_done.txt
    { sleep 0.2; echo done > tmp_bg_pipeline_done.txt; } &
    if test -f tmp_bg_pipeline_done.txt; then echo waited; else echo early; fi
  expect
    stdout "early"
    stderr ""
    exit_code 0
end test "background pipeline does not wait"
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

#### Test: bang negation returns 1 when pipeline succeeds

When the pipeline begins with `!`, the exit status is the logical NOT of the
last command's exit status. A succeeding pipeline yields exit status 1.

```
begin test "bang negation returns 1 when pipeline succeeds"
  script
    ! true
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "bang negation returns 1 when pipeline succeeds"
```

#### Test: bang negation applies to entire multi-command pipeline

The `!` reserved word negates the exit status of the whole pipeline, not just
the first command. Without pipefail, the pipeline exit status is the last
command's; `!` then inverts that.

```
begin test "bang negation applies to entire multi-command pipeline"
  script
    ! true | false
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "bang negation applies to entire multi-command pipeline"
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

#### Test: pipefail reports rightmost non-zero exit status

With pipefail enabled and no `!` prefix, the exit status is the exit status of
the last (rightmost) command that returned a non-zero exit status.

```
begin test "pipefail reports rightmost non-zero exit status"
  script
    set -o pipefail
    (exit 2) | true | (exit 3)
  expect
    stdout ""
    stderr ""
    exit_code 3
end test "pipefail reports rightmost non-zero exit status"
```

#### Test: pipefail returns zero when all commands succeed

With pipefail enabled, the exit status is zero if all commands in the pipeline
returned an exit status of 0.

```
begin test "pipefail returns zero when all commands succeed"
  script
    set -o pipefail
    true | true | true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "pipefail returns zero when all commands succeed"
```

#### Test: pipefail picks rightmost non-zero from earlier command

With pipefail enabled, the exit status is from the rightmost non-zero command,
even when that command is not the last in the pipeline.

```
begin test "pipefail picks rightmost non-zero from earlier command"
  script
    set -o pipefail
    (exit 5) | true
  expect
    stdout ""
    stderr ""
    exit_code 5
end test "pipefail picks rightmost non-zero from earlier command"
```

#### Test: pipefail with bang negation gives zero on failure

With pipefail enabled and `!` prefix, the exit status is zero if any command in
the pipeline returned a non-zero exit status.

```
begin test "pipefail with bang negation gives zero on failure"
  script
    set -o pipefail
    ! false | true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "pipefail with bang negation gives zero on failure"
```

#### Test: pipefail with bang negation gives one on all-zero pipeline

With pipefail enabled and `!` prefix, the exit status is 1 if all commands in
the pipeline returned exit status 0.

```
begin test "pipefail with bang negation gives one on all-zero pipeline"
  script
    set -o pipefail
    ! true | true
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "pipefail with bang negation gives one on all-zero pipeline"
```

#### Test: pipefail uses setting at pipeline start

The shell shall use the pipefail setting at the time it begins execution of the
pipeline, not the setting at the time it sets the exit status. Enabling pipefail
within the pipeline does not retroactively affect the exit status.

```
begin test "pipefail uses setting at pipeline start"
  script
    (exit 42) | set -o pipefail
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "pipefail uses setting at pipeline start"
```

#### Test: pipefail start setting is not cleared by later command

The shell shall use the pipefail setting from when pipeline execution begins.
Disabling pipefail in a later command does not change which command determines
the pipeline exit status.

```
begin test "pipefail start setting is not cleared by later command"
  script
    set -o pipefail
    false | set +o pipefail
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "pipefail start setting is not cleared by later command"
```
