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

An asynchronous AND-OR list (`sleep 30 &`) creates a background job with a
job number and process ID. The `jobs` built-in lists the background job.

```
begin interactive test "background job and jobs listing"
  spawn -i
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "jobs"
  expect "\[1\].*sleep 30"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "background job and jobs listing"
```

#### Test: background job pid is known and matches jobs -p

Each background job shall have an associated process ID that is known in the
current shell execution environment. In an interactive shell, `$!` and
`jobs -p %%` should refer to the same background job process ID.

```
begin interactive test "background job pid is known and matches jobs -p"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "JPID=$(jobs -p %%); [ $JPID = $! ] && echo pid_match"
  expect "pid_match"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "background job pid is known and matches jobs -p"
```

#### Test: fg removes background job from jobs list

When a background job is brought into the foreground by `fg`, its associated
job number shall be removed from the shell's background jobs list after the
job completes.

```
begin interactive test "fg removes background job from jobs list"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 0.1 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "fg %1"
  expect "$ "
  send "jobs | grep . >/dev/null && echo still_listed || echo removed"
  expect "removed"
  expect "$ "
  send "exit"
  wait
end interactive test "fg removes background job from jobs list"
```

#### Test: bg resumes stopped background job with SIGCONT

When a suspended job is continued by `bg`, the shell shall send SIGCONT to
the process group whose stopped wait status caused the shell to suspend the
job.

```
begin interactive test "bg resumes stopped background job with SIGCONT"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "rm -f tmp_bg_cont.txt"
  expect "$ "
  send "contjob() { trap 'echo continued > tmp_bg_cont.txt' CONT; while :; do sleep 1; done; }"
  expect "$ "
  send "contjob &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -STOP %1"
  sleep 500ms
  expect "$ "
  send "bg %1"
  expect "$ "
  sleep 500ms
  send "cat tmp_bg_cont.txt"
  expect "continued"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "bg resumes stopped background job with SIGCONT"
```

#### Test: stopped background job becomes suspended job

When a process associated with a background job is stopped by `SIGSTOP`, the
shell shall convert that background job into a suspended job and, for an
interactive shell, write a suspended-job status message.

```
begin interactive test "stopped background job becomes suspended job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -STOP %1"
  sleep 500ms
  send "jobs"
  expect "(Stopped|Suspended).*sleep 30"
  expect "$ "
  send "kill -CONT %1; kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "stopped background job becomes suspended job"
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

#### Test: background job process is a process group leader

When a single asynchronous AND-OR list creates a background job, its
associated process ID shall be that of a child process that is made a
process group leader (its PGID equals its PID).

```
begin interactive test "background job process is a process group leader"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "[ $(ps -o pgid= -p $!) -eq $! ] && echo pgrp_leader"
  expect "pgrp_leader"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "background job process is a process group leader"
```

#### Test: background pipeline processes share process group

When a background job is a pipeline, all processes created to execute
the AND-OR list shall initially share the same process group ID as the
process group leader.

```
begin interactive test "background pipeline processes share process group"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "(echo $BASHPID > tmp_pgtest_a.txt; sleep 30) | (echo $BASHPID > tmp_pgtest_b.txt; sleep 30) &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  sleep 500ms
  send "PG_A=$(ps -o pgid= -p $(cat tmp_pgtest_a.txt) | tr -d ' '); PG_B=$(ps -o pgid= -p $(cat tmp_pgtest_b.txt) | tr -d ' '); [ $PG_A -eq $PG_B ] && echo same_pgrp"
  expect "same_pgrp"
  expect "$ "
  send "kill %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  send "rm -f tmp_pgtest_a.txt tmp_pgtest_b.txt"
  expect "$ "
  send "exit"
  wait
end interactive test "background pipeline processes share process group"
```

#### Test: foreground pipeline processes share process group

When a foreground pipeline is executed, all processes comprising the
pipeline shall be in the same process group. Each pipeline component
records its own PGID; after completion, both PGIDs are compared.

```
begin interactive test "foreground pipeline processes share process group"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "(ps -o pgid= -p $BASHPID | tr -d ' ' > tmp_fgpg_a.txt; echo a) | (ps -o pgid= -p $BASHPID | tr -d ' ' > tmp_fgpg_b.txt; cat)"
  expect "a"
  expect "$ "
  send "PG_A=$(cat tmp_fgpg_a.txt); PG_B=$(cat tmp_fgpg_b.txt); [ $PG_A -eq $PG_B ] && echo same_pgrp"
  expect "same_pgrp"
  expect "$ "
  send "rm -f tmp_fgpg_a.txt tmp_fgpg_b.txt"
  expect "$ "
  send "exit"
  wait
end interactive test "foreground pipeline processes share process group"
```

#### Test: sequential list with trailing async creates background job

For a list of sequentially executed AND-OR lists followed by an
asynchronous AND-OR list, the sequential parts run as a foreground job.
When they complete, the asynchronous part forms a separate background job.

```
begin interactive test "sequential list with trailing async creates background job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "echo seq_done; sleep 30 &"
  expect "seq_done"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "jobs"
  expect "sleep 30"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "sequential list with trailing async creates background job"
```

#### Test: foreground job stopped by SIGTSTP becomes suspended

When a process in a foreground job is stopped by a catchable signal
(SIGTSTP), the shell shall create a suspended job, assign it a job
number, and write a suspension message to standard error.

```
begin interactive test "foreground job stopped by SIGTSTP becomes suspended"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sh -c 'sleep 0.2; kill -TSTP $$'"
  sleep 1000ms
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "foreground job stopped by SIGTSTP becomes suspended"
```

#### Test: foreground job stopped by SIGSTOP becomes suspended

When a foreground job is stopped by SIGSTOP, the behavior shall be the
same as for a catchable signal: the shell creates a suspended job,
assigns it a job number, and writes a suspension message.

```
begin interactive test "foreground job stopped by SIGSTOP becomes suspended"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sh -c 'sleep 0.2; kill -STOP $$'"
  sleep 1000ms
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "foreground job stopped by SIGSTOP becomes suspended"
```

#### Test: fg resumes suspended job as foreground

A suspended job can be continued as a foreground job by means of the fg
utility, which sends SIGCONT to the job's process group. The job runs to
completion and is then removed from the jobs list.

```
begin interactive test "fg resumes suspended job as foreground"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sh -c 'trap \"exit 0\" CONT; sleep 0.1; kill -STOP $$' &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  sleep 500ms
  send "true"
  expect "(Stopped|Suspended)"
  expect "$ "
  send "fg %1"
  expect "$ "
  send "jobs | grep . > /dev/null && echo still_listed || echo cleared"
  expect "cleared"
  expect "$ "
  send "exit"
  wait
end interactive test "fg resumes suspended job as foreground"
```

#### Test: background job terminated by signal produces notification

When a background job is terminated by a signal (not just normal
completion), an interactive shell shall write a termination notification
before the next prompt.

```
begin interactive test "background job terminated by signal produces notification"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill %1"
  expect "$ "
  send "true"
  expect "(Terminated|Kill).*sleep"
  expect "$ "
  send "exit"
  wait
end interactive test "background job terminated by signal produces notification"
```

#### Test: set -b immediate notification for signal-terminated background job

With `set -b` enabled, the shell shall write a termination notification
immediately after a background job is terminated by a signal, without
waiting for the next prompt.

```
begin interactive test "set -b immediate notification for signal-terminated background job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "set -b"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill %1"
  sleep 500ms
  expect "(Terminated|Kill).*sleep"
  send "echo ok"
  expect "ok"
  sendeof
  wait
end interactive test "set -b immediate notification for signal-terminated background job"
```

#### Test: set -b notification for stopped background job

With `set -b` enabled, when a background job is stopped by a signal the
shell shall write the suspension notification immediately (or at the next
prompt), rather than deferring it until the next prompt without `set -b`.

```
begin interactive test "set -b notification for stopped background job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "set -b"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -STOP %1"
  sleep 500ms
  expect "(Stopped|Suspended).*sleep"
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  send "exit"
  wait
end interactive test "set -b notification for stopped background job"
```
#### Test: non-job-control background jobs do not appear in jobs list

Requirements relating to background jobs stated in this section only apply
to job-control background jobs. If job control is disabled (`set +m`), a
background job is not added to the jobs list and does not produce completion
notifications.

```
begin interactive test "non-job-control background jobs do not appear in jobs list"
  spawn -i
  expect "$ "
  send "set +m"
  expect "$ "
  send "sleep 30 &"
  expect "$ "
  send "jobs | grep . >/dev/null && echo listed || echo not_listed"
  expect "not_listed"
  expect "$ "
  send "kill \$! 2>/dev/null; wait"
  expect "$ "
  sendeof
  wait
end interactive test "non-job-control background jobs do not appear in jobs list"
```

#### Test: interactive shell in background stops with SIGTTIN

If an interactive shell is started with its process group not equal to the
terminal's foreground process group (e.g. started as a background job), it
shall either stop itself with `SIGTTIN` or attempt to read from standard
input, which generates `SIGTTIN`.

```
begin interactive test "interactive shell in background stops with SIGTTIN"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "/usr/bin/bash --posix -i </dev/tty >/dev/null 2>&1 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  sleep 500ms
  send "jobs"
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "interactive shell in background stops with SIGTTIN"
```

#### Test: foreground job sets terminal foreground process group

When a foreground job is created, the shell sets the foreground process
group ID associated with the terminal to the job's process group ID.

```
begin interactive test "foreground job sets terminal foreground process group"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "(ps -o tpgid= -p \$BASHPID | tr -d ' ' > tmp_fgpgid.txt; ps -o pgid= -p \$BASHPID | tr -d ' ' > tmp_pgid.txt)"
  expect "$ "
  send "[ \$(cat tmp_fgpgid.txt) -eq \$(cat tmp_pgid.txt) ] && echo tpgid_match"
  expect "tpgid_match"
  expect "$ "
  send "rm -f tmp_fgpgid.txt tmp_pgid.txt"
  expect "$ "
  sendeof
  wait
end interactive test "foreground job sets terminal foreground process group"
```

#### Test: built-in foreground pipeline does not change tpgid

If all commands in the foreground pipeline are executed by the shell itself
in the current environment (e.g. a simple built-in), the foreground process
group ID is set to the shell's process group ID, so it effectively does not
change.

```
begin interactive test "built-in foreground pipeline does not change tpgid"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "TPGID1=\$(ps -o tpgid= -p \$BASHPID | tr -d ' '); :; TPGID2=\$(ps -o tpgid= -p \$BASHPID | tr -d ' '); [ \$TPGID1 -eq \$TPGID2 ] && echo unchanged"
  expect "unchanged"
  expect "$ "
  sendeof
  wait
end interactive test "built-in foreground pipeline does not change tpgid"
```

#### Test: shell restores terminal foreground process group after foreground job

When a foreground job terminates or becomes suspended, the shell sets the
foreground process group ID associated with the terminal back to its own
process group ID.

```
begin interactive test "shell restores terminal foreground process group after foreground job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "true"
  expect "$ "
  send "PGID=\$(ps -o pgid= -p \$BASHPID | tr -d ' '); TPGID=\$(ps -o tpgid= -p \$BASHPID | tr -d ' '); [ \$PGID -eq \$TPGID ] && echo restored"
  expect "restored"
  expect "$ "
  sendeof
  wait
end interactive test "shell restores terminal foreground process group after foreground job"
```

#### Test: background job brought to foreground and stopped becomes suspended job

If a background job is brought to the foreground with `fg` and is
subsequently stopped by a signal, the entire job shall become a suspended
job as if it had been stopped while running in the background.

```
begin interactive test "background job brought to foreground and stopped becomes suspended job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "sh -c \"sleep 0.2; kill -STOP \$!\" &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "fg %1 >/dev/null"
  sleep 500ms
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 %2 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "background job brought to foreground and stopped becomes suspended job"
```

#### Test: fg removes process ID from known list

When a background job is brought into the foreground by `fg`, its associated
process ID shall be removed from the list of process IDs known in the
current shell execution environment.

```
begin interactive test "fg removes process ID from known list"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 0.1 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "PID=\$!"
  expect "$ "
  send "fg %1 >/dev/null"
  expect "$ "
  send "wait \$PID 2>/dev/null; echo \$?"
  expect "127"
  expect "$ "
  sendeof
  wait
end interactive test "fg removes process ID from known list"
```

#### Test: foreground job stopped by SIGTTIN becomes suspended

When a foreground job is stopped by the catchable `SIGTTIN` signal, the shell
creates a suspended job and writes a status message.

```
begin interactive test "foreground job stopped by SIGTTIN becomes suspended"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sh -c 'sleep 0.2; kill -TTIN $$'"
  sleep 1000ms
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "foreground job stopped by SIGTTIN becomes suspended"
```

#### Test: foreground job stopped by SIGTTOU becomes suspended

When a foreground job is stopped by the catchable `SIGTTOU` signal, the shell
creates a suspended job and writes a status message.

```
begin interactive test "foreground job stopped by SIGTTOU becomes suspended"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sh -c 'sleep 0.2; kill -TTOU $$'"
  sleep 1000ms
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "foreground job stopped by SIGTTOU becomes suspended"
```

#### Test: background job stopped by SIGTSTP becomes suspended job

When a background job is stopped by `SIGTSTP`, the shell converts it into a
suspended job.

```
begin interactive test "background job stopped by SIGTSTP becomes suspended job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -TSTP %1"
  sleep 500ms
  send "jobs"
  expect "(Stopped|Suspended).*sleep"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "background job stopped by SIGTSTP becomes suspended job"
```

#### Test: background job stopped by SIGTTIN becomes suspended job

When a background job is stopped by `SIGTTIN`, the shell converts it into a
suspended job.

```
begin interactive test "background job stopped by SIGTTIN becomes suspended job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -TTIN %1"
  sleep 500ms
  send "jobs"
  expect "(Stopped|Suspended).*sleep"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "background job stopped by SIGTTIN becomes suspended job"
```

#### Test: background job stopped by SIGTTOU becomes suspended job

When a background job is stopped by `SIGTTOU`, the shell converts it into a
suspended job.

```
begin interactive test "background job stopped by SIGTTOU becomes suspended job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -TTOU %1"
  sleep 500ms
  send "jobs"
  expect "(Stopped|Suspended).*sleep"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "background job stopped by SIGTTOU becomes suspended job"
```

#### Test: stopped background job produces notification at next prompt

When a process associated with a background job is stopped by a signal and
`set -b` is disabled, an interactive shell writes the suspended-job message
immediately prior to writing the next prompt for input.

```
begin interactive test "stopped background job produces notification at next prompt"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -STOP %1"
  expect "$ "
  sleep 500ms
  send "echo trigger_prompt"
  expect "(Stopped|Suspended)"
  expect "$ "
  send "kill -9 %1 2>/dev/null; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "stopped background job produces notification at next prompt"
```

#### Test: SIGCONT to stopped background job continues it in background

Execution of a suspended job can be continued as a non-suspended background
job by sending the stopped processes a `SIGCONT` signal.

```
begin interactive test "SIGCONT to stopped background job continues it in background"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 30 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "kill -STOP %1"
  sleep 500ms
  expect "$ "
  send "kill -CONT %1"
  expect "$ "
  sleep 500ms
  send "jobs"
  expect "(Running|Running).*sleep 30"
  expect "$ "
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "SIGCONT to stopped background job continues it in background"
```
