# Test Suite for Intrinsic Utility: fg

This test suite covers the **fg** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: fg](#utility-fg)

## utility: fg

#### NAME

> fg — run jobs in the foreground

#### SYNOPSIS

> `[UP] fg [job_id]`

#### DESCRIPTION

> If job control is enabled (see the description of [*set*](docs/posix/md/utilities/V3_chap02.md#set) **-m**), the shell is interactive, and the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)) is not a subshell environment, the *fg* utility shall move a background job in the current execution environment into the foreground, as described in [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control); it may also do so if the shell is non-interactive or the current shell execution environment is a subshell environment.
>
> Using *fg* to place a job into the foreground shall remove its process ID from the list of those "known in the current shell execution environment"; see [*2.9.3.1 Asynchronous AND-OR Lists*](docs/posix/md/utilities/V3_chap02.md#2931-asynchronous-and-or-lists).

#### OPTIONS

> None.

#### OPERANDS

> The following operand shall be supported:
>
> - *job_id*: Specify the job to be run as a foreground job. If no *job_id* operand is given, the *job_id* for the job that was most recently suspended, placed in the background, or run as a background job shall be used. The format of *job_id* is described in XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id).

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *fg*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The *fg* utility shall write the command line of the job to standard output in the following format:
>
> ```
> "%s\n", <command>
> ```

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> If the *fg* utility succeeds, it does not return an exit status. Instead, the shell waits for the job that *fg* moved into the foreground.
>
> If *fg* does not move a job into the foreground, the following exit value shall be returned:
>
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> If job control is disabled, the *fg* utility shall exit with an error and no job shall be placed in the foreground.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> The *fg* utility does not work as expected when it is operating in its own utility execution environment because that environment has no applicable jobs to manipulate. See the APPLICATION USAGE section for [*bg*](docs/posix/md/utilities/bg.md). For this reason, *fg* is generally implemented as a shell regular built-in.

#### EXAMPLES

> None.

#### RATIONALE

> The extensions to the shell specified in this volume of POSIX.1-2024 have mostly been based on features provided by the KornShell. The job control features provided by [*bg*](docs/posix/md/utilities/bg.md), *fg*, and [*jobs*](docs/posix/md/utilities/jobs.md) are also based on the KornShell. The standard developers examined the characteristics of the C shell versions of these utilities and found that differences exist. Despite widespread use of the C shell, the KornShell versions were selected for this volume of POSIX.1-2024 to maintain a degree of uniformity with the rest of the KornShell features selected (such as the very popular command line editing features).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.9.3.1 Asynchronous AND-OR Lists*](docs/posix/md/utilities/V3_chap02.md#2931-asynchronous-and-or-lists), [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment), [*bg*](docs/posix/md/utilities/bg.md) , [*kill*](docs/posix/md/utilities/kill.md#tag_20_64), [*jobs*](docs/posix/md/utilities/jobs.md), [*wait*](docs/posix/md/utilities/wait.md#tag_20_147)
>
> XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id), [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.
>
> The APPLICATION USAGE section is added.
>
> The JC marking is removed from the SYNOPSIS since job control is mandatory is this version.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1254 is applied, updating the DESCRIPTION to account for the addition of [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control) and changing the EXIT STATUS section.

*End of informative text.*

### Tests

#### Test: fg: error when job control is disabled

`fg` produces an error when job control is not enabled.

```
begin test "fg: error when job control is disabled"
  script
    fg
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "fg: error when job control is disabled"
```

#### Test: fg: bring background job to foreground

Verifies that `fg` moves a background job into the foreground and prints the job's command line to stdout, as required by POSIX.

```
begin interactive test "fg: bring background job to foreground"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "fg"
  expect "sleep 60"
  sleep 200ms
  sendraw 03
  expect "$ "
  sendeof
  wait
end interactive test "fg: bring background job to foreground"
```

#### Test: fg: removes job from known process list

Verifies that after `fg` brings a job to the foreground and it completes, the job is removed from the shell's known process list, so `jobs` no longer reports it.

```
begin interactive test "fg: removes job from known process list"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "fg"
  expect "sleep 60"
  sleep 200ms
  sendraw 03
  expect "$ "
  send "jobs | wc -l"
  expect "0"
  expect "$ "
  sendeof
  wait
end interactive test "fg: removes job from known process list"
```

#### Test: fg output goes to stdout

Verifies that `fg` writes the resumed job's command line to standard output. The job is suspended, backgrounded, and then foregrounded while stderr is redirected away to confirm the output goes to stdout.

```
begin interactive test "fg output goes to stdout"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 100"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "bg"
  expect "$ "
  send "fg 2>/dev/null"
  expect "sleep"
  sleep 200ms
  sendraw 03
  expect "$ "
  send "kill %1 2>/dev/null; wait; true"
  expect "$ "
  sendeof
  wait
end interactive test "fg output goes to stdout"
```

#### Test: fg produces no stderr with valid job

Verifies that `fg` produces no diagnostic output on stderr when given a valid job. Stderr is redirected to a file and confirmed to be empty afterward.

```
begin interactive test "fg produces no stderr with valid job"
  spawn -im
  expect "$ "
  send "sleep 0.1 &"
  expect "$ "
  send "fg 2>/tmp/fg_stderr_test"
  expect "$ "
  send "[ ! -s /tmp/fg_stderr_test ] && echo fg_stderr_empty || echo fg_stderr_nonempty; rm -f /tmp/fg_stderr_test"
  expect "fg_stderr_empty"
  expect "$ "
  sendeof
  wait
end interactive test "fg produces no stderr with valid job"
```

#### Test: fg/bg send SIGCONT to stopped job

Verifies that `fg` sends SIGCONT to a job that was explicitly stopped with `kill -STOP`, resuming it in the foreground. This confirms the shell delivers SIGCONT as part of POSIX job control when moving a stopped job to the foreground.

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
  sendraw 03
  sleep 500ms
  sendeof
  wait
end interactive test "fg/bg send SIGCONT to stopped job"
```
