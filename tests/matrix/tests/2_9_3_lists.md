# Test Suite for 2.9.3 Lists

This test suite covers **Section 2.9.3 Lists** of the POSIX.1-2024
Shell Command Language specification (part of 2.9 Shell Commands), including
asynchronous AND-OR lists, sequential AND-OR lists, AND lists, and OR lists.

## Table of contents

- [2.9.3 Lists](#293-lists)
- [2.9.3.1 Asynchronous AND-OR Lists](#2931-asynchronous-and-or-lists)
- [2.9.3.2 Sequential AND-OR Lists](#2932-sequential-and-or-lists)
- [2.9.3.3 AND Lists](#2933-and-lists)
- [2.9.3.4 OR Lists](#2934-or-lists)

## 2.9.3 Lists

An *AND-OR list* is a sequence of one or more pipelines separated by the operators `"&&"` and `"||"`.

A *list* is a sequence of one or more AND-OR lists separated by the operators `';'` and `'&'`.

The operators `"&&"` and `"||"` shall have equal precedence and shall be evaluated with left associativity. For example, both of the following commands write solely **bar** to standard output:

```
false && echo foo || echo bar
true || echo foo && echo bar
```

A `';'` separator or a `';'` or `<newline>` terminator shall cause the preceding AND-OR list to be executed sequentially; an `'&'` separator or terminator shall cause asynchronous execution of the preceding AND-OR list.

The term "compound-list" is derived from the grammar in [2.10 Shell Grammar](#210-shell-grammar); it is equivalent to a sequence of *lists*, separated by `<newline>` characters, that can be preceded or followed by an arbitrary number of `<newline>` characters.

---

*The following sections are informative.*

##### Examples

The following is an example that illustrates `<newline>` characters in compound-lists:

```
while
    # a couple of <newline>s

    # a list
    date && who || ls; cat file
    # a couple of <newline>s

    # another list
    wc file > output & true

do
    # 2 lists
    ls
    cat file
done
```

*End of informative text.*

---

### Tests

#### Test: left associativity: false && echo foo || echo bar

The `&&` and `||` operators have equal precedence and left associativity.
`false && echo foo` fails, so `|| echo bar` executes.

```
begin test "left associativity: false && echo foo || echo bar"
  script
    false && echo foo || echo bar
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "left associativity: false && echo foo || echo bar"
```

#### Test: left associativity: true || echo foo && echo bar

With left associativity, `true || echo foo` succeeds (skipping `echo foo`),
then `&& echo bar` executes because the left side succeeded.

```
begin test "left associativity: true || echo foo && echo bar"
  script
    true || echo foo && echo bar
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "left associativity: true || echo foo && echo bar"
```

#### Test: sequential AND-OR lists write solely bar

Both forms of left-associative evaluation produce only `bar` on stdout.

```
begin test "sequential AND-OR lists write solely bar"
  script
    false && echo foo || echo bar
    true || echo foo && echo bar
  expect
    stdout "bar\nbar"
    stderr ""
    exit_code 0
end test "sequential AND-OR lists write solely bar"
```

#### Test: semicolon separator executes lists sequentially

A `';'` separator shall cause the preceding AND-OR list to be executed
sequentially. Commands separated by `;` on a single line execute in order.

```
begin test "semicolon separator executes lists sequentially"
  script
    x=1; x=2; echo "$x"
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "semicolon separator executes lists sequentially"
```

#### Test: asynchronous list isolation

An `&` terminator causes asynchronous execution in a subshell; variable
changes in the subshell do not affect the parent.

```
begin test "asynchronous list isolation"
  script
    var="parent"
    { var="child"; echo "$var" > tmp_sub.txt; } & wait
    echo "$var"
    cat tmp_sub.txt
  expect
    stdout "parent\nchild"
    stderr ""
    exit_code 0
end test "asynchronous list isolation"
```

#### Test: async AND-OR list exit status is zero

The exit status of an asynchronous AND-OR list (terminated by `&`) shall be
zero, regardless of the exit status of the commands within it. The `$?`
immediately after `false &` confirms the async list itself returned zero.

```
begin test "async AND-OR list exit status is zero"
  script
    false &
    echo "$?"
    wait
    true
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "async AND-OR list exit status is zero"
```

## 2.9.3.1 Asynchronous AND-OR Lists

If an AND-OR list is terminated by the control operator `<ampersand>` (`'&'`), the shell shall execute the AND-OR list asynchronously in a subshell environment. This subshell shall execute in the background; that is, the shell shall not wait for the subshell to terminate before executing the next command (if any); if there are no further commands to execute, the shell shall not wait for the subshell to terminate before exiting.

If job control is enabled (see [set](#tag_19_26), **-m**), the AND-OR list shall become a job-control background job and a job number shall be assigned to it. If job control is disabled, the AND-OR list may become a non-job-control background job, in which case a job number shall be assigned to it; if no job number is assigned it shall become a background command but not a background job.

A job-control background job can be controlled as described in [2.11 Job Control](#211-job-control).

The process ID associated with the asynchronous AND-OR list shall become known in the current shell execution environment; see [2.13 Shell Execution Environment](#213-shell-execution-environment). This process ID shall remain known until any one of the following occurs (and, unless otherwise specified, may continue to remain known after it occurs).

- The process terminates and the application waits for the process ID or the corresponding job ID (see [*wait*](docs/posix/md/utilities/wait.md#tag_20_147)).
- If the asynchronous AND-OR list did not become a background job: another asynchronous AND-OR list is invoked before `"$!"` (corresponding to the previous asynchronous AND-OR list) is expanded in the current shell execution environment.
- If the asynchronous AND-OR list became a background job: the [*jobs*](docs/posix/md/utilities/jobs.md) utility reports the termination status of that job.
- If the shell is interactive and the asynchronous AND-OR list became a background job: a message indicating completion of the corresponding job is written to standard error. If [*set*](#set) **-b** is enabled, it is unspecified whether the process ID is removed from the list of known process IDs when the message is written or immediately prior to when the shell writes the next prompt for input.

The implementation need not retain more than the {CHILD_MAX} most recent entries in its list of known process IDs in the current shell execution environment.

If, and only if, job control is disabled, the standard input for the subshell in which an asynchronous AND-OR list is executed shall initially be assigned to an open file description that behaves as if **/dev/null** had been opened for reading only. This initial assignment shall be overridden by any explicit redirection of standard input within the AND-OR list.

If the shell is interactive and the asynchronous AND-OR list became a background job, the job number and the process ID associated with the job shall be written to standard error using the format:

```
"[%d] %d\n", <job-number>, <process-id>
```

If the shell is interactive and the asynchronous AND-OR list did not become a background job, the process ID associated with the asynchronous AND-OR list shall be written to standard error in an unspecified format.

##### Exit Status

The exit status of an asynchronous AND-OR list shall be zero.

The exit status of the subshell in which the AND-OR list is asynchronously executed can be obtained using the [*wait*](docs/posix/md/utilities/wait.md) utility.

### Tests

#### Test: background job PID and wait

An asynchronous AND-OR list runs in a subshell; `$!` captures its PID
and `wait` waits for its completion.

```
begin test "background job PID and wait"
  script
    echo "bg_test" > tmp_bg.txt &
    bg_pid=$!
    wait $bg_pid
    cat tmp_bg.txt
  expect
    stdout "bg_test"
    stderr ""
    exit_code 0
end test "background job PID and wait"
```

#### Test: background job runs in subshell

Variable changes in the asynchronous subshell do not affect the parent shell
environment.

```
begin test "background job runs in subshell"
  script
    my_var="parent"
    my_var="child" &
    wait $!
    echo "$my_var"
  expect
    stdout "parent"
    stderr ""
    exit_code 0
end test "background job runs in subshell"
```

#### Test: background stdin from /dev/null

When job control is disabled, the standard input of an async AND-OR list
is initially assigned as if /dev/null were opened for reading.

```
begin test "background stdin from /dev/null"
  script
    cat &
    wait $!
    echo "done"
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "background stdin from /dev/null"
```

#### Test: job-control background job prints job number and PID

If the shell is interactive and the asynchronous AND-OR list became a
background job, the job number and process ID shall be written to stderr.

```
begin interactive test "job-control background job prints job number and PID"
  spawn -i
  expect "[$] "
  send "sleep 0.1 &"
  expect "\[1\] [0-9]+"
  sendeof
  wait
end interactive test "job-control background job prints job number and PID"
```

#### Test: explicit redirect overrides /dev/null

An explicit redirection of standard input overrides the default /dev/null
assignment for async lists.

```
begin test "explicit redirect overrides /dev/null"
  script
    echo "redirected" > tmp_in.txt
    cat < tmp_in.txt &
    wait $!
  expect
    stdout "redirected"
    stderr ""
    exit_code 0
end test "explicit redirect overrides /dev/null"
```

#### Test: wait returns background command exit status

The exit status of the subshell in which the AND-OR list is asynchronously
executed can be obtained using the wait utility. Here `wait $!` returns the
exit status of the `(exit 42)` background command.

```
begin test "wait returns background command exit status"
  script
    (exit 42) &
    wait $!
    echo "$?"
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "wait returns background command exit status"
```

## 2.9.3.2 Sequential AND-OR Lists

AND-OR lists that are separated by a `<semicolon>` (`';'`) shall be executed sequentially. The format for executing AND-OR lists sequentially shall be:

```
aolist1 [; aolist2] ...
```

Each AND-OR list shall be expanded and executed in the order specified.

If job control is enabled, the AND-OR lists shall form all or part of a foreground job that can be controlled as described in [2.11 Job Control](#211-job-control).

##### Exit Status

The exit status of a sequential AND-OR list shall be the exit status of the last pipeline in the AND-OR list that is executed.

### Tests

#### Test: sequential list

AND-OR lists separated by newlines are executed sequentially.

```
begin test "sequential list"
  script
    echo a
    echo b
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "sequential list"
```

#### Test: sequential list exit status is last pipeline

The exit status of a sequential list is determined by the exit status
of the last pipeline that was executed.

```
begin test "sequential list exit status is last pipeline"
  script
    echo a
    echo b
    false
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 1
end test "sequential list exit status is last pipeline"
```

## 2.9.3.3 AND Lists

The control operator `"&&"` denotes an AND list. The format shall be:

```
command1 [ && command2] ...
```

First *command1* shall be executed. If its exit status is zero, *command2* shall be executed, and so on, until a command has a non-zero exit status or there are no more commands left to execute. The commands are expanded only if they are executed.

##### Exit Status

The exit status of an AND list shall be the exit status of the last command that is executed in the list.

### Tests

#### Test: AND list executes second command on success

When the first command succeeds, the second command in the AND list is
executed.

```
begin test "AND list executes second command on success"
  script
    true && echo "and success"
  expect
    stdout "and success"
    stderr ""
    exit_code 0
end test "AND list executes second command on success"
```

#### Test: AND list skips second command on failure

When the first command fails, subsequent commands in the AND list are
skipped.

```
begin test "AND list skips second command on failure"
  script
    false && echo "should not print"
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "AND list skips second command on failure"
```

#### Test: AND list exit status is last executed command

The exit status of an AND list is the exit status of the last command
that was actually executed.

```
begin test "AND list exit status is last executed command"
  script
    true && false && echo no
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "AND list exit status is last executed command"
```

#### Test: AND list skipped commands are not expanded

The commands in an AND list are expanded only if they are executed. When a
command fails, subsequent commands are not expanded — side effects of expansion
(such as command substitution) do not occur.

```
begin test "AND list skipped commands are not expanded"
  script
    rm -f tmp_and_expand.txt
    false && echo $(touch tmp_and_expand.txt)
    test -f tmp_and_expand.txt && echo "expanded" || echo "not_expanded"
  expect
    stdout "not_expanded"
    stderr ""
    exit_code 0
end test "AND list skipped commands are not expanded"
```

## 2.9.3.4 OR Lists

The control operator `"||"` denotes an OR List. The format shall be:

```
command1 [ || command2] ...
```

First, *command1* shall be executed. If its exit status is non-zero, *command2* shall be executed, and so on, until a command has a zero exit status or there are no more commands left to execute.

##### Exit Status

The exit status of an OR list shall be the exit status of the last command that is executed in the list.

### Tests

#### Test: OR list executes second command on failure

When the first command has a non-zero exit status, the next command in the
OR list shall be executed.

```
begin test "OR list executes second command on failure"
  script
    false || echo "or_executed"
  expect
    stdout "or_executed"
    stderr ""
    exit_code 0
end test "OR list executes second command on failure"
```

#### Test: OR list skips second command on success

When the first command succeeds, subsequent commands in the OR list are
skipped.

```
begin test "OR list skips second command on success"
  script
    true || echo "should not print"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "OR list skips second command on success"
```

#### Test: OR list skipped commands are not expanded

When a command in an OR list succeeds, subsequent commands are not executed and
their expansions do not occur — side effects like command substitution are
suppressed.

```
begin test "OR list skipped commands are not expanded"
  script
    rm -f tmp_or_expand.txt
    true || echo $(touch tmp_or_expand.txt)
    test -f tmp_or_expand.txt && echo "expanded" || echo "not_expanded"
  expect
    stdout "not_expanded"
    stderr ""
    exit_code 0
end test "OR list skipped commands are not expanded"
```

#### Test: OR list exit status when all commands fail

When no command in the OR list has a zero exit status and there are no more
commands left to execute, the exit status is that of the last command executed.

```
begin test "OR list exit status when all commands fail"
  script
    false || false
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "OR list exit status when all commands fail"
```

#### Test: OR list exit status is last executed command

The exit status of an OR list is the exit status of the last command
that was actually executed.

```
begin test "OR list exit status is last executed command"
  script
    false || false || true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "OR list exit status is last executed command"
```
