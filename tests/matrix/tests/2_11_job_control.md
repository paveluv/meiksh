# Test Suite for 2.11 Job Control

This test suite covers **Section 2.11 Job Control** of the POSIX.1-2024
Shell Command Language specification. Job control allows users to selectively
stop and resume processes, and is enabled by `set -m` (default in interactive
shells). This section has no subsections.

## Table of contents

- [2.11 Job Control](#211-job-control)

## 2.11 Job Control

Job control is defined (see XBD [*3.181 Job Control*](docs/posix/md/basedefs/V1_chap03.md#3181-job-control)) as a facility that allows users selectively to stop (suspend) the execution of processes and continue (resume) their execution at a later point. It is jointly supplied by the terminal I/O driver and a command interpreter. The shell is one such command interpreter and job control in the shell is enabled by [set](#tag_19_26) **-m** (which is enabled by default in interactive shells). The remainder of this section describes the job control facility provided by the shell. Requirements relating to background jobs stated in this section only apply to job-control background jobs.

If the shell has a controlling terminal and it is the controlling process for the terminal session, it shall initially set the foreground process group ID associated with the terminal to its own process group ID. Otherwise, if it has a controlling terminal, it shall initially perform the following steps if interactive and may perform them if non-interactive:

1. If its process group is the foreground process group associated with the terminal, the shell shall set its process group ID to its process ID (if they are not already equal) and set the foreground process group ID associated with the terminal to its process group ID.
2. If its process group is not the foreground process group associated with the terminal (which would result from it being started by a job-control shell as a background job), the shell shall either stop itself by sending itself a SIGTTIN signal or, if interactive, attempt to read from standard input (which generates a SIGTTIN signal if standard input is the controlling terminal). If it is stopped, then when it continues execution (after receiving a SIGCONT signal) it shall repeat these steps.

Subsequently, the shell shall change the foreground process group associated with its controlling terminal when a foreground job is running as noted in the description below.

When job control is enabled, the shell shall create one or more jobs when it executes a list (see [2.9.3 Lists](#293-lists)) that has one of the following forms:

- A single asynchronous AND-OR list
- One or more sequentially executed AND-OR lists followed by at most one asynchronous AND-OR list

For the purposes of job control, a list that includes more than one asynchronous AND-OR list shall be treated as if it were split into multiple separate lists, each ending with an asynchronous AND-OR list.

When a job consisting of a single asynchronous AND-OR list is created, it shall form a *background job* and the associated process ID shall be that of a child process that is made a process group leader, with all other processes (if any) that the shell creates to execute the AND-OR list initially having this process ID as their process group ID.

For a list consisting of one or more sequentially executed AND-OR lists followed by at most one asynchronous AND-OR list, the whole list shall form a single *foreground job* up until the sequentially executed AND-OR lists have all completed execution, at which point the asynchronous AND-OR list (if any) shall form a background job as described above.

For each pipeline in a foreground job, if the pipeline is executed while the list is still a foreground job, the set of processes comprising the pipeline, and any processes descended from it, shall all be in the same process group, unless the shell executes some of the commands in the pipeline in the current shell execution environment and others in a subshell environment; in this case the process group ID of the current shell need not change (or cannot change if it is the session leader), and consequently the process group ID that the other processes all share may differ from the process group ID of the current shell (which means that a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTOU signal sent to one of those process groups does not cause the whole pipeline to stop).

A background job that was created on execution of an asynchronous AND-OR list can be brought into the foreground by means of the [*fg*](docs/posix/md/utilities/fg.md) utility (if supported); in this case the entire job shall become a single foreground job. If a process that the shell subsequently waits for is part of this foreground job and is stopped by a signal, the entire job shall become a suspended job and the behavior shall be as if the process had been stopped while the job was running in the background.

When a foreground job is created, or a background job is brought into the foreground by the [*fg*](docs/posix/md/utilities/fg.md) utility, if the shell has a controlling terminal it shall set the foreground process group ID associated with the terminal as follows:

- If the job was originally created as a background job, the foreground process group ID shall be set to the process ID of the process that the shell made a process group leader when it executed the asynchronous AND-OR list.
- If the job was originally created as a foreground job, the foreground process group ID shall be set as follows when each pipeline in the job is executed:
    - If the shell is not itself executing, in the current shell execution environment, all of the commands in the pipeline, the foreground process group ID shall be set to the process group ID that is shared by the other processes executing the pipeline (see above).
    - If all of the commands in the pipeline are being executed by the shell itself in the current shell execution environment, the foreground process group ID shall be set to the process group ID of the shell.

When a foreground job terminates, or becomes a suspended job (see below), if the shell has a controlling terminal it shall set the foreground process group ID associated with the terminal to the process group ID of the shell.

Each background job (whether suspended or not) shall have associated with it a job number and a process ID that is known in the current shell execution environment. When a background job is brought into the foreground by means of the [*fg*](docs/posix/md/utilities/fg.md) utility, the associated job number shall be removed from the shell's background jobs list and the associated process ID shall be removed from the list of process IDs known in the current shell execution environment.

If a process that the shell is waiting for is part of a foreground job that was started as a foreground job and is stopped by a catchable signal (SIGTSTP, SIGTTIN, or SIGTTOU):

- If the currently executing AND-OR list within the list comprising the foreground job consists of a single pipeline in which all of the commands are simple commands, the shell shall either create a suspended job consisting of at least that AND-OR list and the remaining (if any) AND-OR lists in the same list, or create a suspended job consisting of just that AND-OR list and discard the remaining (if any) AND-OR lists in the same list.
- Otherwise, the shell shall create a suspended job consisting of a set of commands, from within the list comprising the foreground job, that is unspecified except that the set shall include at least the pipeline to which the stopped process belongs. Commands in the foreground job that have not already completed and are not included in the suspended job shall be discarded.

**Note:** Although only a pipeline of simple commands is guaranteed to remain intact if started in the foreground and subsequently suspended, it is possible to ensure that a complex AND-OR list will remain intact when suspended by starting it in the background and immediately bringing it into the foreground. For example:

```
command1 && command2 | { command3 || command4; } & fg
```

If a process that the shell is waiting for is part of a foreground job that was started as a foreground job and is stopped by a SIGSTOP signal, the behavior shall be as described above for a catchable signal unless the shell was executing a built-in utility in the current shell execution environment when the SIGSTOP was delivered, resulting in the shell itself being stopped by the signal, in which case if the shell subsequently receives a SIGCONT signal and has one or more child processes that remain stopped, the shell shall create a suspended job as if only those child processes had been stopped.

When a suspended job is created as a result of a foreground job being stopped, it shall be assigned a job number, and an interactive shell shall write, and a non-interactive shell may write, a message to standard error, formatted as described by the [*jobs*](docs/posix/md/utilities/jobs.md) utility (without the **-l** option) for a suspended job. The message may indicate that the commands comprising the job include commands that have already completed; in this case the completed commands shall not be repeated if execution of the job is subsequently continued. If the shell is interactive, it shall save the terminal settings before changing them to the settings it needs to read further commands.

When a process associated with a background job is stopped by a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTOU signal, the shell shall convert the (non-suspended) background job into a suspended job and an interactive shell shall write a message to standard error, formatted as described by the [*jobs*](docs/posix/md/utilities/jobs.md) utility (without the **-l** option) for a suspended job, at the following time:

- If [*set*](#set) **-b** is enabled, the message shall be written either immediately after the job became suspended or immediately prior to writing the next prompt for input.
- If [*set*](#set) **-b** is disabled, the message shall be written immediately prior to writing the next prompt for input.

Execution of a suspended job can be continued as a foreground job by means of the [*fg*](docs/posix/md/utilities/fg.md) utility (if supported), or as a (non-suspended) background job either by means of the [*bg*](docs/posix/md/utilities/bg.md) utility (if supported) or by sending the stopped processes a SIGCONT signal. The [*fg*](docs/posix/md/utilities/fg.md) and [*bg*](docs/posix/md/utilities/bg.md) utilities shall send a SIGCONT signal to the process group of the process(es) whose stopped wait status caused the shell to suspend the job. If the shell has a controlling terminal, the [*fg*](docs/posix/md/utilities/fg.md) utility shall send the SIGCONT signal after it has set the foreground process group ID associated with the terminal (see above). If the [*fg*](docs/posix/md/utilities/fg.md) utility is used from an interactive shell to bring into the foreground a suspended job that was created from a foreground job, before it sends the SIGCONT signal the [*fg*](docs/posix/md/utilities/fg.md) utility shall restore the terminal settings to the ones that the shell saved when the job was suspended.

When a background job completes or is terminated by a signal, an interactive shell shall write a message to standard error, formatted as described by the [*jobs*](docs/posix/md/utilities/jobs.md) utility (without the **-l** option) for a job that completed or was terminated by a signal, respectively, at the following time:

- If [*set*](#set) **-b** is enabled, the message shall be written immediately after the job completes or is terminated.
- If [*set*](#set) **-b** is disabled, the message shall be written immediately prior to writing the next prompt for input.

In each case above where an interactive shell writes a message immediately prior to writing the next prompt for input, the same message may also be written by a non-interactive shell, at any of the following times:

- After the next time a foreground job terminates or is suspended
- Before the shell parses further input
- Before the shell exits

### Tests

#### Test: background job and jobs listing

An asynchronous AND-OR list (`sleep 10 &`) creates a background job with a
job number and process ID. The `jobs` built-in lists the background job.

```
begin interactive test "background job and jobs listing"
  spawn -i
  expect "$ "
  send "sleep 10 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "jobs"
  expect "\[1\].*sleep 10"
  sendeof
  wait
end interactive test "background job and jobs listing"
```

#### Test: user must explicitly exit interactive shell

An interactive shell with a controlling terminal does not exit on its own;
the user must issue an explicit `exit` command.

```
begin interactive test "user must explicitly exit interactive shell"
  spawn -i
  expect "$ "
  send "echo still_here"
  expect "still_here"
  expect "$ "
  send "exit"
  wait
end interactive test "user must explicitly exit interactive shell"
```

#### Test: fg/bg send SIGCONT to stopped job

A background job stopped with SIGSTOP can be brought to the foreground
with `fg`, which sends SIGCONT. The job resumes execution and can then
be terminated.

```
begin interactive test "fg/bg send SIGCONT to stopped job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill -STOP %1"
  sleep 500ms
  expect "$ "
  send "fg %1"
  sleep 500ms
  send ""
  sleep 200ms
  send "kill %1"
  sleep 500ms
  sendeof
  wait
end interactive test "fg/bg send SIGCONT to stopped job"
```

#### Test: background job completion notification

When a background job completes and `set -b` is not enabled, the shell
writes a completion notification immediately before the next prompt.

```
begin interactive test "background job completion notification"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 0.1 &"
  expect "$ "
  sleep 500ms
  send "echo trigger_prompt"
  expect "\[[[:digit:]]+\].*Done.*sleep"
  sendeof
  wait
end interactive test "background job completion notification"
```

#### Test: signal inheritance with job control disabled

When job control is disabled, SIGINT is inherited as ignored by
asynchronous commands. The trapped signal action persists in the
subshell created for the background job.

```
begin test "signal inheritance with job control disabled"
  script
    trap "" INT
    (trap) & wait
  expect
    stdout ".*"
    stderr ""
    exit_code 0
end test "signal inheritance with job control disabled"
```

#### Test: trap inheritance in shell scripts

When the shell has a controlling terminal and is the controlling process,
it sets the foreground process group. Child shell scripts inherit the
default trap actions (caught traps are reset to default).

```
begin test "trap inheritance in shell scripts"
  script
    s=$TMPDIR/_trap_inherit_$$.sh
    printf 'trap
    ' > $s; chmod +x $s; trap 'echo caught' USR1; $SHELL $s; rc=$?; rm -f $s; exit $rc
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "trap inheritance in shell scripts"
```

#### Test: set -- positional parameters

The `set` special built-in with `--` sets positional parameters.
This verifies basic `set` functionality used in conjunction with
job control notification settings (`set -b`).

```
begin test "set -- positional parameters"
  script
    set -- a b c
    echo count=$#
  expect
    stdout "count=3"
    stderr ""
    exit_code 0
end test "set -- positional parameters"
```

#### Test: set -b immediate background job notification

With `set -b` enabled, the shell writes background job completion
notifications immediately rather than waiting for the next prompt.

```
begin interactive test "set -b immediate background job notification"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "set -b"
  expect "$ "
  send "sleep 0.1 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  sleep 1000ms
  expect "Done"
  send "echo setb_ok"
  expect "setb_ok"
  sendeof
  wait
end interactive test "set -b immediate background job notification"
```

#### Test: multiple async commands in one list

A list with multiple asynchronous AND-OR lists (`cmd1 & cmd2 & cmd3 &`)
is treated as if split into separate lists, each creating its own
background job with a distinct job number.

```
begin interactive test "multiple async commands in one list"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 1 & sleep 2 & sleep 3 &"
  expect "\[1\]"
  expect "\[2\]"
  expect "\[3\]"
  expect "$ "
  send "kill %1 %2 %3 2>/dev/null; wait"
  expect "$ "
  sendeof
  wait
end interactive test "multiple async commands in one list"
```
