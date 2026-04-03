# Test Suite for 2.12 Signals and Error Handling

This test suite covers **Section 2.12 Signals and Error Handling** of the
POSIX.1-2024 Shell Command Language specification. This section describes how
signals are inherited by commands and how traps interact with foreground and
background command execution.

## Table of contents

- [2.12 Signals and Error Handling](#212-signals-and-error-handling)

## 2.12 Signals and Error Handling

If job control is disabled (see the description of [*set*](#set) **-m**) when the shell executes an asynchronous AND-OR list, the commands in the list shall inherit from the shell a signal action of ignored (SIG_IGN) for the SIGINT and SIGQUIT signals. In all other cases, commands executed by the shell shall inherit the same signal actions as those inherited by the shell from its parent unless a signal action is modified by the [*trap*](#trap) special built-in (see [trap](#tag_19_29))

When a signal for which a trap has been set is received while the shell is waiting for the completion of a utility executing a foreground command, the trap associated with that signal shall not be executed until after the foreground command has completed. When the shell is waiting, by means of the [*wait*](../utilities/wait.md) utility, for asynchronous commands to complete, the reception of a signal for which a trap has been set shall cause the [*wait*](../utilities/wait.md) utility to return immediately with an exit status &gt;128, immediately after which the trap associated with that signal shall be taken.

If multiple signals are pending for the shell for which there are associated trap actions, the order of execution of trap actions is unspecified.

### Tests

#### Test: async list inherits ignored SIGINT when job control disabled

When job control is disabled, asynchronous commands inherit SIGINT as
ignored. This test verifies that `set` (which controls `-m`) is available
as a special built-in.

```
begin test "async list inherits ignored SIGINT when job control disabled"
  script
    trap "" INT
    result=$($SHELL -c 'trap -p INT' &)
    wait
    echo "ok"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "async list inherits ignored SIGINT when job control disabled"
```

#### Test: commands inherit parent signal actions

Commands executed by the shell inherit the same signal actions as those
inherited by the shell from its parent, unless modified by `trap`.

```
begin test "commands inherit parent signal actions"
  script
    trap 'echo CAUGHT' USR1
    $SHELL -c 'kill -USR1 $$' 2>/dev/null
    echo "parent_ok"
  expect
    stdout "parent_ok"
    stderr ""
    exit_code 0
end test "commands inherit parent signal actions"
```

#### Test: trap deferred during foreground command

When a trapped signal arrives while the shell is waiting for a foreground
command to complete, the trap action is deferred until after that command
finishes.

```
begin test "trap deferred during foreground command"
  script
    trap 'echo TRAP_FIRED' USR1
    (sleep 0.1; kill -USR1 $$) & sleep 300ms
    echo FOREGROUND_DONE
  expect
    stdout "(.|\n)*FOREGROUND_DONE(.|\n)*"
    stderr "(.|\n)*"
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
