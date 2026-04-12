# Test Suite for Intrinsic Utility: bg

This test suite covers the **bg** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: bg](#utility-bg)

## utility: bg

#### NAME

> bg — run jobs in the background

#### SYNOPSIS

> `[UP] bg [job_id...]`

#### DESCRIPTION

> If job control is enabled (see the description of [*set*](docs/posix/md/utilities/V3_chap02.md#set) **-m**), the shell is interactive, and the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)) is not a subshell environment, the *bg* utility shall resume suspended jobs from the current shell execution environment by running them as background jobs, as described in [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control); it may also do so if the shell is non-interactive or the current shell execution environment is a subshell environment. If the job specified by *job_id* is already a running background job, the *bg* utility shall have no effect and shall exit successfully.

#### OPTIONS

> None.

#### OPERANDS

> The following operand shall be supported:
>
> - *job_id*: Specify the job to be resumed as a background job. If no *job_id* operand is given, the most recently suspended job shall be used. The format of *job_id* is described in XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id) .

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *bg*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The output of *bg* shall consist of a line in the format:
>
> ```
> "[%d] %s\n", <job-number>, <command>
> ```
>
> where the fields are as follows:
>
> - \<*job-number*\>: A number that can be used to identify the job to the [*wait*](docs/posix/md/utilities/wait.md), [*fg*](docs/posix/md/utilities/fg.md), and [*kill*](docs/posix/md/utilities/kill.md) utilities. Using these utilities, the job can be identified by prefixing the job number with `'%'`.
> - \<*command*\>: The associated command that was given to the shell.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: Successful completion.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> If job control is disabled, the *bg* utility shall exit with an error and no job shall be placed in the background.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> A job is generally suspended by typing the SUSP character (`<control>`-Z on most systems); see XBD [*11. General Terminal Interface*](docs/posix/md/basedefs/V1_chap11.md#11-general-terminal-interface). At that point, *bg* can put the job into the background. This is most effective when the job is expecting no terminal input and its output has been redirected to non-terminal files. A background job can be forced to stop when it has terminal output by issuing the command:
>
> ```
> stty tostop
> ```
>
> A background job can be stopped with the command:
>
> ```
> kill -s stop job ID
> ```
>
> The *bg* utility does not work as expected when it is operating in its own utility execution environment because that environment has no suspended jobs. In the following examples:
>
> ```
> ... | xargs bg
> (bg)
> ```
>
> each *bg* operates in a different environment and does not share its parent shell's understanding of jobs. For this reason, *bg* is generally implemented as a shell regular built-in.

#### EXAMPLES

> None.

#### RATIONALE

> The extensions to the shell specified in this volume of POSIX.1-2024 have mostly been based on features provided by the KornShell. The job control features provided by *bg*, [*fg*](docs/posix/md/utilities/fg.md), and [*jobs*](docs/posix/md/utilities/jobs.md) are also based on the KornShell. The standard developers examined the characteristics of the C shell versions of these utilities and found that differences exist. Despite widespread use of the C shell, the KornShell versions were selected for this volume of POSIX.1-2024 to maintain a degree of uniformity with the rest of the KornShell features selected (such as the very popular command line editing features).
>
> The *bg* utility is expected to wrap its output if the output exceeds the number of display columns.
>
> The *bg* and [*fg*](docs/posix/md/utilities/fg.md) utilities are not symmetric as regards the list of process IDs known in the current shell execution environment. Whereas [*fg*](docs/posix/md/utilities/fg.md) removes a process ID from this list, *bg* has no need to add one to this list when it resumes execution of a suspended job in the background, because this has already been done by the shell when the suspended background job was created (see [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control)).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.9.3.1 Asynchronous AND-OR Lists*](docs/posix/md/utilities/V3_chap02.md#2931-asynchronous-and-or-lists), [*fg*](docs/posix/md/utilities/fg.md), [*kill*](docs/posix/md/utilities/kill.md#tag_20_64), [*jobs*](docs/posix/md/utilities/jobs.md), [*wait*](docs/posix/md/utilities/wait.md#tag_20_147)
>
> XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id), [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*11. General Terminal Interface*](docs/posix/md/basedefs/V1_chap11.md#11-general-terminal-interface)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.
>
> The JC margin marker on the SYNOPSIS is removed since support for Job Control is mandatory in this version. This is a FIPS requirement.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1254 is applied, updating the DESCRIPTION to account for the addition of [*2.11 Job Control*](docs/posix/md/utilities/V3_chap02.md#211-job-control) and adding a paragraph to RATIONALE.

*End of informative text.*

### Tests

#### Test: bg: error when job control is disabled

`bg` produces an error when job control is not enabled.

```
begin test "bg: error when job control is disabled"
  script
    bg
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "bg: error when job control is disabled"
```

#### Test: bg: job already running in background

Verifies that `bg` exits successfully and has no effect when applied to a job that is already running in the background, as required by POSIX.

```
begin interactive test "bg: job already running in background"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "bg %1; echo bg_exit_$?"
  expect "bg_exit_0"
  send "kill %1 2>/dev/null; wait"
  expect "$ "
  sendeof
  wait
end interactive test "bg: job already running in background"
```

#### Test: bg: with explicit job_id operand

Verifies that `bg` accepts an explicit `%N` job ID operand and resumes a stopped job in the background. The job is suspended with Ctrl-Z and then resumed via `bg %1`.

```
begin interactive test "bg: with explicit job_id operand"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "bg %1"
  expect "sleep 60"
  expect "$ "
  send "kill %1 2>/dev/null; wait; true"
  expect "$ "
  sendeof
  wait
end interactive test "bg: with explicit job_id operand"
```

#### Test: bg: output format POSIX [%d] %s

Verifies that `bg` produces output in the POSIX-required format `[%d] %s` (job number followed by the command) when resuming a suspended job in the background.

```
begin interactive test "bg: output format POSIX [%d] %s"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "bg"
  expect "\[[[:digit:]]+\] .*sleep 60"
  expect "$ "
  send "kill %1 2>/dev/null; wait; true"
  expect "$ "
  sendeof
  wait
end interactive test "bg: output format POSIX [%d] %s"
```

#### Test: bg output goes to stdout

Verifies that `bg` writes its job status line to standard output (not stderr) by redirecting stdout to a file and confirming the output appears there.

```
begin interactive test "bg output goes to stdout"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 100"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "bg > /tmp/meiksh_bg_out 2>/dev/null"
  expect "$ "
  send "cat /tmp/meiksh_bg_out"
  expect "\[[[:digit:]]+\]"
  expect "$ "
  send "kill %1 2>/dev/null; wait"
  expect "$ "
  send "rm -f /tmp/meiksh_bg_out"
  expect "$ "
  sendeof
  wait
end interactive test "bg output goes to stdout"
```

#### Test: suspend -> bg -> fg cycle

Verifies the full POSIX job control lifecycle: a foreground job is suspended with Ctrl-Z, resumed in the background with `bg`, and then brought back to the foreground with `fg`, confirming that all three transitions work correctly.

```
begin interactive test "suspend -> bg -> fg cycle"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60"
  sleep 500ms
  sendraw 1a
  expect "(Stopped|Suspended)"
  send "bg"
  expect "sleep 60"
  expect "$ "
  send "fg"
  expect "sleep 60"
  sleep 200ms
  sendraw 03
  expect "$ "
  sendeof
  wait
end interactive test "suspend -> bg -> fg cycle"
```

#### Test: bg/fg: explicit %N job IDs with multiple jobs

Verifies that `bg` and `fg` correctly handle explicit `%N` job ID operands when multiple stopped jobs exist. Two jobs are suspended, then `bg %1` resumes the first in the background and `fg %2` brings the second to the foreground.

```
begin interactive test "bg/fg: explicit %N job IDs with multiple jobs"
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
  send "bg %1"
  expect "sleep 60"
  expect "$ "
  send "fg %2"
  expect "sleep 61"
  sleep 200ms
  sendraw 03
  expect "$ "
  send "kill %1 2>/dev/null; wait; true"
  expect "$ "
  sendeof
  wait
end interactive test "bg/fg: explicit %N job IDs with multiple jobs"
```
