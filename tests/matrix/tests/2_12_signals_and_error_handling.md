# Test Suite for 2.12 Signals and Error Handling

This test suite covers **Section 2.12 Signals and Error Handling** of the
POSIX.1-2024 Shell Command Language specification. This section describes how
signals are inherited by commands and how traps interact with foreground and
background command execution.

## Table of contents

- [2.12 Signals and Error Handling](#212-signals-and-error-handling)

## 2.12 Signals and Error Handling

If job control is disabled (see the description of [*set*](#set) **-m**) when the shell executes an asynchronous AND-OR list, the commands in the list shall inherit from the shell a signal action of ignored (SIG_IGN) for the SIGINT and SIGQUIT signals. In all other cases, commands executed by the shell shall inherit the same signal actions as those inherited by the shell from its parent unless a signal action is modified by the [*trap*](#trap) special built-in (see [trap](#tag_19_29))

When a signal for which a trap has been set is received while the shell is waiting for the completion of a utility executing a foreground command, the trap associated with that signal shall not be executed until after the foreground command has completed. When the shell is waiting, by means of the [*wait*](docs/posix/md/utilities/wait.md) utility, for asynchronous commands to complete, the reception of a signal for which a trap has been set shall cause the [*wait*](docs/posix/md/utilities/wait.md) utility to return immediately with an exit status \>128, immediately after which the trap associated with that signal shall be taken.

If multiple signals are pending for the shell for which there are associated trap actions, the order of execution of trap actions is unspecified.

### Tests

#### Test: async list inherits ignored SIGINT when job control disabled

When job control is disabled, an asynchronous AND-OR list shall inherit
SIG_IGN for SIGINT. A child process sends itself SIGINT; because the
signal is ignored, it survives and prints confirmation.

```
begin test "async list inherits ignored SIGINT when job control disabled"
  script
    set +m
    sh -c 'kill -INT $$; echo survived' &
    wait
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "async list inherits ignored SIGINT when job control disabled"
```

#### Test: commands inherit ignored signal from parent

Commands executed by the shell shall inherit the same signal actions as
those inherited by the shell from its parent. A signal set to SIG_IGN
via `trap '' USR1` is inherited by a child command, which survives
sending itself that signal.

```
begin test "commands inherit ignored signal from parent"
  script
    trap '' USR1
    sh -c 'kill -USR1 $$; echo survived'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "commands inherit ignored signal from parent"
```

#### Test: trap deferred during foreground command

When a trapped signal arrives during a foreground command, the trap shall
not execute until after the foreground command completes. The trap fires
after `sleep` finishes but before the next command runs, so TRAP_FIRED
appears before AFTER_SLEEP.

```
begin test "trap deferred during foreground command"
  script
    trap 'echo TRAP_FIRED' USR1
    (sleep 0.1; kill -USR1 $$) &
    sleep 0.3
    echo AFTER_SLEEP
  expect
    stdout "TRAP_FIRED\nAFTER_SLEEP"
    stderr ""
    exit_code 0
end test "trap deferred during foreground command"
```

#### Test: wait interrupted by trapped signal

When `wait` is used to wait for asynchronous commands, reception of a
trapped signal causes `wait` to return immediately with exit status >128,
and then the trap action is executed.

```
begin test "wait interrupted by trapped signal"
  script
    trap 'echo GOT_USR1' USR1
    sleep 60 & bgpid=$!
    (sleep 0.1; kill -USR1 $$) & wait $bgpid
    rc=$?
    kill $bgpid 2>/dev/null
    wait $bgpid 2>/dev/null
    echo wait_rc=$rc
  expect
    stdout "GOT_USR1\nwait_rc=(129|1[3-9][0-9]|2[0-4][0-9]|25[0-5])"
    stderr ""
    exit_code 0
end test "wait interrupted by trapped signal"
```

#### Test: async list inherits ignored SIGQUIT when job control disabled

When job control is disabled, an asynchronous AND-OR list shall inherit
SIG_IGN for SIGQUIT in addition to SIGINT. A child process sends itself
SIGQUIT; because the signal is ignored, it survives.

```
begin test "async list inherits ignored SIGQUIT when job control disabled"
  script
    set +m
    sh -c 'kill -QUIT $$; echo survived' &
    wait
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "async list inherits ignored SIGQUIT when job control disabled"
```

#### Test: caught trap does not propagate to child command

A caught trap (as opposed to an ignored one) does not propagate through
exec to child processes; the signal is reset to its default action. The
child is terminated by the signal, confirming the caught action was not
inherited.

```
begin test "caught trap does not propagate to child command"
  script
    trap 'echo CAUGHT' USR1
    bash -c 'kill -USR1 $$' 2>/dev/null
    [ $? -gt 128 ] && echo child_terminated
  expect
    stdout "child_terminated"
    stderr ""
    exit_code 0
end test "caught trap does not propagate to child command"
```

#### Test: async list with job control does not inherit ignored SIGINT

When job control is enabled, the SIG_IGN override for SIGINT does not
apply to asynchronous lists. The background process retains the default
SIGINT action and is terminated when sent the signal.

```
begin interactive test "async list with job control does not inherit ignored SIGINT"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -INT %1"
  expect "$ "
  send "wait %1 2>/dev/null; [ $? -gt 128 ] && echo killed_by_sigint"
  expect "killed_by_sigint"
  expect "$ "
  send "exit"
  wait
end interactive test "async list with job control does not inherit ignored SIGINT"
```

#### Test: multiple pending trapped signals all fire

When multiple signals with associated trap actions are pending (their
order of execution is unspecified), each trap shall still be taken. Both
USR1 and USR2 arrive during a foreground command and both traps fire
after it completes.

```
begin test "multiple pending trapped signals all fire"
  script
    trap 'echo GOT_USR1' USR1
    trap 'echo GOT_USR2' USR2
    (sleep 0.1; kill -USR1 $$; kill -USR2 $$) &
    sleep 0.3
    echo DONE
  expect
    stdout "(GOT_USR1\nGOT_USR2|GOT_USR2\nGOT_USR1)\nDONE"
    stderr ""
    exit_code 0
end test "multiple pending trapped signals all fire"
```
