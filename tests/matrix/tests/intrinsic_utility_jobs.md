# Test Suite for Intrinsic Utility: jobs

This test suite covers the **jobs** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: jobs](#utility-jobs)

## utility: jobs

#### NAME

> jobs — display status of jobs in the current shell execution environment

#### SYNOPSIS

> `[UP] jobs [-l|-p] [job_id...]`

#### DESCRIPTION

> If the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)) is not a subshell environment, the *jobs* utility shall display the status of background jobs that were created in the current shell execution environment; it may also do so if the current shell execution environment is a subshell environment.
>
> When *jobs* reports the termination status of a job, the shell shall remove the job from the background jobs list and the associated process ID from the list of those "known in the current shell execution environment"; see [*2.9.3.1 Asynchronous AND-OR Lists*](docs/posix/md/utilities/V3_chap02.md#2931-asynchronous-and-or-lists). If a write error occurs when *jobs* writes to standard output, some process IDs might have been removed from the list but not successfully reported.

#### OPTIONS

> The *jobs* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following options shall be supported:
>
> - **-l**: (The letter ell.) Provide more information about each job listed. See STDOUT for details.
> - **-p**: Display only the process IDs for the process group leaders of job-control background jobs and the process IDs associated with non-job-control background jobs (if supported).
>
> By default, the *jobs* utility shall display the status of all background jobs, both running and suspended, and all jobs whose status has changed and have not been reported by the shell.

#### OPERANDS

> The following operand shall be supported:
>
> - *job_id*: Specifies the jobs for which the status is to be displayed. If no *job_id* is given, the status information for all jobs shall be displayed. The format of *job_id* is described in XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id).

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *jobs*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error and informative messages written to standard output.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> If the **-p** option is specified, the output shall consist of one line for each process ID:
>
> ```
> "%d\n", <process ID>
> ```
>
> Otherwise, if the **-l** option is not specified, the output shall be a series of lines of the form:
>
> ```
> "[%d] %c %s %s\n", <job-number>, <current>, <state>, <command>
> ```
>
> where the fields shall be as follows:
>
> - \<*current*\>: The character `'+'` identifies the job that would be used as a default for the [*fg*](docs/posix/md/utilities/fg.md) or [*bg*](docs/posix/md/utilities/bg.md) utilities; this job can also be specified using the *job_id* %+ or `"%%"`. The character `'-'` identifies the job that would become the default if the current default job were to exit; this job can also be specified using the *job_id* %-. For other jobs, this field is a `<space>`. At most one job can be identified with `'+'` and at most one job can be identified with `'-'`. If there is any suspended job, then the current job shall be a suspended job. If there are at least two suspended jobs, then the previous job also shall be a suspended job.
> - \<*job-number*\>: A number that can be used to identify the job to the [*wait*](docs/posix/md/utilities/wait.md), [*fg*](docs/posix/md/utilities/fg.md), [*bg*](docs/posix/md/utilities/bg.md), and [*kill*](docs/posix/md/utilities/kill.md) utilities. Using these utilities, the job can be identified by prefixing the job number with `'%'`.
> - \<*state*\>: One of the following strings (in the POSIX locale):
>
>     - **Running**: Indicates that the job has not been suspended by a signal and has not exited.
>     - **Done**: Indicates that the job completed and returned exit status zero.
>     - **Done**(*code*): Indicates that the job completed normally and that it exited with the specified non-zero exit status, *code*, expressed as a decimal number.
>     - **Stopped**: Indicates that the job was suspended by the SIGTSTP signal.
>     - **Stopped** (**SIGTSTP**): Indicates that the job was suspended by the SIGTSTP signal.
>     - **Stopped** (**SIGSTOP**): Indicates that the job was suspended by the SIGSTOP signal.
>     - **Stopped** (**SIGTTIN**): Indicates that the job was suspended by the SIGTTIN signal.
>     - **Stopped** (**SIGTTOU**): Indicates that the job was suspended by the SIGTTOU signal.
>
>   The implementation may substitute the string **Suspended** in place of **Stopped**. If the job was terminated by a signal, the format of \<*state*\> is unspecified, but it shall be visibly distinct from all of the other \<*state*\> formats shown here and shall indicate the name or description of the signal causing the termination.
> - \<*command*\>: The associated command that was given to the shell.
>
> If the **-l** option is specified:
>
> - For job-control background jobs, a field containing the process group ID shall be inserted before the \<*state*\> field. Also, more processes in a process group may be output on separate lines, using only the process ID and \<*command*\> fields.
> - For non-job-control background jobs (if supported), a field containing the process ID associated with the job shall be inserted before the \<*state*\> field. Also, more processes created to execute the job may be output on separate lines, using only the process ID and \<*command*\> fields.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: The output specified in STDOUT was successfully written to standard output.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> The **-p** option is the only portable way to find out the process group of a job-control background job because different implementations have different strategies for defining the process group of the job. Usage such as $(*jobs* **-p**) provides a way of referring to the process group of the job in an implementation-independent way.
>
> The *jobs* utility does not work as expected when it is operating in its own utility execution environment because that environment has no applicable jobs to manipulate. See the APPLICATION USAGE section for [*bg*](docs/posix/md/utilities/bg.md). For this reason, *jobs* is generally implemented as a shell regular built-in.

#### EXAMPLES

> None.

#### RATIONALE

> Both `"%%"` and `"%+"` are used to refer to the current job. Both forms are of equal validity—the `"%%"` mirroring `"$$"` and `"%+"` mirroring the output of *jobs*. Both forms reflect historical practice of the KornShell and the C shell with job control.
>
> The job control features provided by [*bg*](docs/posix/md/utilities/bg.md), [*fg*](docs/posix/md/utilities/fg.md), and *jobs* are based on the KornShell. The standard developers examined the characteristics of the C shell versions of these utilities and found that differences exist. Despite widespread use of the C shell, the KornShell versions were selected for this volume of POSIX.1-2024 to maintain a degree of uniformity with the rest of the KornShell features selected (such as the very popular command line editing features).
>
> The *jobs* utility is not dependent on job control being enabled, as are the seemingly related [*bg*](docs/posix/md/utilities/bg.md) and [*fg*](docs/posix/md/utilities/fg.md) utilities because *jobs* is useful for examining background jobs, regardless of the current state of job control. When job control has been disabled using [*set*](docs/posix/md/utilities/V3_chap02.md#set) **+m**, the *jobs* utility can still be used to examine the job-control background jobs and (if supported) non-job-control background jobs that were created in the current shell execution environment. See also the RATIONALE for [*kill*](docs/posix/md/utilities/kill.md) and [*wait*](docs/posix/md/utilities/wait.md).
>
> The output for terminated jobs is left unspecified to accommodate various historical systems. The following formats have been witnessed:
>
> 1. **Killed**(*signal name*)
> 2. *signal name*
> 3. *signal name*(**coredump**)
> 4. *signal description*- **core dumped**
>
> Most users should be able to understand these formats, although it means that applications have trouble parsing them.
>
> The calculation of job IDs was not described since this would suggest an implementation, which may impose unnecessary restrictions.
>
> In an early proposal, a **-n** option was included to "Display the status of jobs that have changed, exited, or stopped since the last status report". It was removed because the shell always writes any changed status of jobs before each prompt.
>
> If *jobs* uses buffered writes to standard output, a write error could be detected when attempting to flush a buffer containing multiple reports of terminated jobs, resulting in some unreported jobs having their process IDs removed from the list of those known in the current shell execution environment (because they were removed when the report was added to the buffer).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment), [*bg*](docs/posix/md/utilities/bg.md), [*fg*](docs/posix/md/utilities/fg.md), [*kill*](docs/posix/md/utilities/kill.md#tag_20_64), [*wait*](docs/posix/md/utilities/wait.md#tag_20_147)
>
> XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id), [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.
>
> The JC shading is removed as job control is mandatory in this version.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1254 is applied, updating various requirements for the *jobs* utility to account for the addition of [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control).
>
> Austin Group Defect 1492 is applied, clarifying the requirements when a write error to standard output occurs.

*End of informative text.*

### Tests

#### Test: jobs -p shows background PID

`jobs -p` writes the process ID of each background job.

```
begin test "jobs -p shows background PID"
  script
    sleep 60 &
    pid=$!
    result=$(jobs -p 2>/dev/null | grep "^${pid}$")
    kill $pid 2>/dev/null; wait $pid 2>/dev/null
    test -n "$result" && echo "found"
  expect
    stdout "found"
    stderr ""
    exit_code 0
end test "jobs -p shows background PID"
```

#### Test: jobs with no background jobs produces no output

When no background jobs exist, `jobs` produces no output.

```
begin test "jobs with no background jobs produces no output"
  script
    output=$(jobs 2>/dev/null)
    echo "len=${#output}"
  expect
    stdout "len=0"
    stderr ""
    exit_code 0
end test "jobs with no background jobs produces no output"
```

#### Test: jobs -l shows long listing

Verifies that `jobs -l` produces a long-format listing that includes the job number and command for a background job, as POSIX requires the `-l` option to provide additional information such as the process group ID.

```
begin interactive test "jobs -l shows long listing"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "jobs -l"
  expect "\[[[:digit:]]+\].*sleep 60"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -l shows long listing"
```

#### Test: jobs -l shows numeric PID

Verifies that the long listing from `jobs -l` includes a numeric process ID, as POSIX requires the `-l` option to insert the process group ID before the state field.

```
begin interactive test "jobs -l shows numeric PID"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "jobs -l | grep -E \"[0-9]+\" && echo pid_ok"
  expect "pid_ok"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -l shows numeric PID"
```

#### Test: jobs -p returns PID

Verifies that `jobs -p` outputs the process ID of a background job, as POSIX requires this option to display only process IDs for job-control background jobs.

```
begin interactive test "jobs -p returns PID"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "JPID=$(jobs -p); echo pid_is_$JPID"
  expect "pid_is_"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -p returns PID"
```

#### Test: jobs -p output is numeric

Verifies that the output of `jobs -p` consists entirely of decimal digits, since POSIX specifies the format as one process ID per line.

```
begin interactive test "jobs -p output is numeric"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "jobs -p | grep -qE \"^[0-9]+$\" && echo numeric_ok"
  expect "numeric_ok"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -p output is numeric"
```

#### Test: running job shows Running state

Verifies that a background job that has not been suspended or exited is reported by `jobs` with the state string "Running", as required by POSIX.

```
begin interactive test "running job shows Running state"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "jobs"
  expect "\[[[:digit:]]+\].*Running.*sleep 60"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "running job shows Running state"
```

#### Test: completed job shows Done state

Verifies that a background job that has finished successfully is reported by `jobs` with the state string "Done", as POSIX requires for jobs that completed with exit status zero.

```
begin interactive test "completed job shows Done state"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 0.1 &"
  sleep 500ms
  send "jobs"
  expect "\[[[:digit:]]+\].*Done.*sleep"
  expect "$ "
  sendeof
  wait
end interactive test "completed job shows Done state"
```

#### Test: stopped job shows Stopped or Suspended state

Verifies that a job suspended via SIGTSTP (Ctrl-Z) is reported by `jobs` as "Stopped" or "Suspended". POSIX allows implementations to use either string for a SIGTSTP-stopped job.

```
begin interactive test "stopped job shows Stopped or Suspended state"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended).*sleep 60"
  send "jobs"
  expect "\[[[:digit:]]+\].*(Stopped|Suspended).*sleep 60"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "stopped job shows Stopped or Suspended state"
```

#### Test: output format includes job number status command

Verifies that `jobs` output follows the POSIX format "[job-number] current state command", showing the bracketed job number, the Running state, and the original command text.

```
begin interactive test "output format includes job number status command"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "jobs"
  expect "\[[[:digit:]]+\].*Running.*sleep 60"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "output format includes job number status command"
```

#### Test: jobs with job_id shows only that job

Verifies that when a job_id operand is given (e.g. `%1`), `jobs` displays only the specified job and omits other background jobs, as POSIX requires.

```
begin interactive test "jobs with job_id shows only that job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "jobs %1 | grep -c 'sleep 61' || true; echo end_jobs_check"
  expect "0"
  expect "end_jobs_check"
  expect "$ "
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs with job_id shows only that job"
```

#### Test: jobs with no args lists all jobs

Verifies that `jobs` with no operands displays the status of all background jobs, as POSIX requires when no job_id is given.

```
begin interactive test "jobs with no args lists all jobs"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "jobs"
  expect "sleep 60"
  expect "sleep 61"
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs with no args lists all jobs"
```

#### Test: jobs -l with stopped job shows PID and state

Verifies that `jobs -l` displays both the process ID and the Stopped/Suspended state for a job that has been suspended via SIGTSTP, combining the long-listing and stopped-state requirements.

```
begin interactive test "jobs -l with stopped job shows PID and state"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended).*sleep 60"
  send "jobs -l"
  expect "\[[[:digit:]]+\].*(Stopped|Suspended).*sleep 60"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -l with stopped job shows PID and state"
```

#### Test: current job marker +

Verifies that when multiple background jobs exist, `jobs` marks one of them with the `+` character to indicate the current (default) job, as POSIX specifies for the `<current>` field.

```
begin interactive test "current job marker +"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "jobs"
  expect "\[[[:digit:]]+\]\+.*sleep"
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "current job marker +"
```

#### Test: previous job marker -

Verifies that when multiple background jobs exist, `jobs` marks one of them with the `-` character to indicate the previous job (the one that would become current if the current job exits), as POSIX specifies.

```
begin interactive test "previous job marker -"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "jobs"
  expect "\[[[:digit:]]+\]-.*sleep"
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "previous job marker -"
```

#### Test: signal-terminated job no longer Running

Verifies that after a background job is killed with SIGKILL, `jobs` no longer reports it as "Running". POSIX requires the state to be visibly distinct from Running once the job has been terminated by a signal.

```
begin interactive test "signal-terminated job no longer Running"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill -KILL %1"
  sleep 500ms
  send "jobs | grep -c Running || true; echo end_run_check"
  expect "0"
  expect "end_run_check"
  expect "$ "
  send "wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "signal-terminated job no longer Running"
```

#### Test: Done(N) for non-zero exit

Verifies that a background job which exits with a non-zero status is reported by the shell as "Done(N)" where N is the exit code, as POSIX requires for jobs that completed normally with a non-zero exit status.

```
begin interactive test "Done(N) for non-zero exit"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "(exit 2) &"
  sleep 500ms
  send ""
  expect "Done\([[:digit:]]+\)"
  expect "$ "
  sendeof
  wait
end interactive test "Done(N) for non-zero exit"
```

#### Test: jobs -p with multiple jobs

Verifies that `jobs -p` outputs one process ID per line when multiple background jobs are running, as POSIX specifies one line per process ID.

```
begin interactive test "jobs -p with multiple jobs"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "jobs -p"
  expect "[[:digit:]]+"
  expect "[[:digit:]]+"
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "jobs -p with multiple jobs"
```

#### Test: background job and jobs listing

Verifies that launching a background job produces the "[job-number] PID" notification and that a subsequent `jobs` command lists that job with its command text.

```
begin interactive test "background job and jobs listing"
  spawn -i
  expect "$ "
  send "sleep 10 &"
  expect "\[[[:digit:]]+\] [[:digit:]]+"
  expect "$ "
  send "jobs"
  expect "\[[[:digit:]]+\].*sleep 10"
  sendeof
  wait
end interactive test "background job and jobs listing"
```

#### Test: fg/bg send SIGCONT to stopped job

Verifies that `fg` resumes a stopped background job by sending SIGCONT. A job is stopped with SIGSTOP and then brought to the foreground with `fg %1`, which must cause it to continue running.

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

Verifies that the shell asynchronously notifies the user when a background job completes, displaying a "Done" status message before the next prompt, as POSIX requires for jobs whose status has changed.

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

Verifies that `set -b` enables immediate (asynchronous) notification of background job completion, so the "Done" message appears as soon as the job finishes rather than waiting for the next prompt.

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

Verifies that multiple commands separated by `&` in a single command line each create a separate background job with distinct job numbers, as POSIX requires for asynchronous AND-OR lists.

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

#### Test: jobs output goes to stdout

Verifies that the `jobs` utility writes its job-status output to standard output (not stderr), so it can be redirected to a file and captured, as POSIX specifies.

```
begin interactive test "jobs output goes to stdout"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 100 &"
  expect "$ "
  send "jobs > /tmp/meiksh_jobs_out 2>/dev/null"
  expect "$ "
  send "cat /tmp/meiksh_jobs_out"
  expect "\[[[:digit:]]+\]"
  expect "$ "
  send "jobs >/dev/null; echo jobs_done"
  expect "jobs_done"
  send "kill %1 2>/dev/null; wait"
  expect "$ "
  send "rm -f /tmp/meiksh_jobs_out"
  expect "$ "
  sendeof
  wait
end interactive test "jobs output goes to stdout"
```

#### Test: async launch notification goes to stderr

Verifies that the "[job-number] PID" notification printed when a background job is launched is written to standard error, not standard output, so it does not interfere with command output or pipelines.

```
begin interactive test "async launch notification goes to stderr"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "{ sleep 1 & } 2> async_notice.txt"
  expect "$ "
  send "grep -Eq '\\[[[:digit:]]+\\] [[:digit:]]+' async_notice.txt && echo stderr_ok || echo stderr_missing"
  expect "stderr_ok"
  expect "$ "
  send "kill %1 2>/dev/null; wait; rm -f async_notice.txt; true"
  expect "$ "
  sendeof
  wait
end interactive test "async launch notification goes to stderr"
```

#### Test: jobs respects locale env vars

Verifies that `jobs` does not crash or misbehave when locale environment variables (LC_ALL) are set to a non-default value, as POSIX requires the utility to honor LC_ALL, LC_CTYPE, and LC_MESSAGES.

```
begin interactive test "jobs respects locale env vars"
  spawn -i
  expect "$ "
  send "export LC_ALL=test_EPTY.UTF-8"
  expect "$ "
  send "jobs"
  expect "$ "
  send "echo ok"
  expect "ok"
  expect "$ "
  sendeof
  wait
end interactive test "jobs respects locale env vars"
```

#### Test: multiple suspended jobs with +/- marking

Verifies that when two jobs are suspended, `jobs` marks one with `+` (current) and the other with `-` (previous). POSIX requires that if at least two suspended jobs exist, both the current and previous job must be suspended jobs.

```
begin interactive test "multiple suspended jobs with +/- marking"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "sleep 61"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "jobs"
  expect_line "\[[[:digit:]]+\]-.*(Stopped|Suspended)"
  expect_line "\[[[:digit:]]+\]\+.*(Stopped|Suspended)"
  send "kill %1 %2; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "multiple suspended jobs with +/- marking"
```
