# Test Suite for Intrinsic Utility: kill

This test suite covers the **kill** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: kill](#utility-kill)

## utility: kill

#### NAME

> kill — terminate or signal processes

#### SYNOPSIS

> ```
> kill [-s signal_name] pid...
> kill -l [exit_status]
> ```

#### DESCRIPTION

> The *kill* utility shall send a signal to the process or processes specified by each *pid* operand.
>
> For each *pid* operand, the *kill* utility shall perform actions equivalent to the [*kill*()](docs/posix/md/functions/kill.md) function defined in the System Interfaces volume of POSIX.1-2024 called with the following arguments:
>
> - The value of the *pid* operand shall be used as the *pid* argument.
> - The *sig* argument is the value specified by the **-s** option, **-***signal_number* option, or the **-***signal_name* option, or by SIGTERM, if none of these options is specified.

#### OPTIONS

> The *kill* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), except that in the last two SYNOPSIS forms, the **-***signal_number* and **-***signal_name* options are usually more than a single character.
>
> The following options shall be supported:
>
> - **-l**: (The letter ell.) Write all values of *signal_name* supported by the implementation, if no operand is given. If an *exit_status* operand is given and it is a value of the `'?'` shell special parameter (see [*2.5.2 Special Parameters*](docs/posix/md/utilities/V3_chap02.md#252-special-parameters) and [*wait*](docs/posix/md/utilities/wait.md)) corresponding to a process that was terminated or stopped by a signal, the *signal_name* corresponding to the signal that terminated or stopped the process shall be written. If an *exit_status* operand is given and it is the unsigned decimal integer value of a signal number, the *signal_name* (the symbolic constant name without the **SIG** prefix defined in the Base Definitions volume of POSIX.1-2024) corresponding to that signal shall be written. Otherwise, the results are unspecified.
> - **-s***signal_name*: Specify the signal to send, using one of the symbolic names defined in the [*\<signal.h\>*](docs/posix/md/basedefs/signal.h.md) header. Values of *signal_name* shall be recognized in a case-independent fashion, without the **SIG** prefix. In addition, the symbolic name 0 shall be recognized, representing the signal value zero. The corresponding signal shall be sent instead of SIGTERM.
> - **-***signal_name*: Equivalent to **-s** *signal_name* .
> - **-***signal_number*: Specify a non-negative decimal integer, *signal_number* , representing the signal to be used instead of SIGTERM, as the *sig* argument in the effective call to [*kill*()](docs/posix/md/functions/kill.md) . The correspondence between integer values and the *sig* value used is shown in the following list. The effects of specifying any *signal_number* other than those listed below are undefined.
>
>     - 0: 0
>     - 1: SIGHUP
>     - 2: SIGINT
>     - 3: SIGQUIT
>     - 6: SIGABRT
>     - 9: SIGKILL
>     - 14: SIGALRM
>     - 15: SIGTERM
>
>   If the first argument is a negative integer, it shall be interpreted as a **-***signal_number* option, not as a negative *pid* operand specifying a process group.

#### OPERANDS

> The following operands shall be supported:
>
> - *pid*: One of the following:
>
>     1. A decimal integer specifying a process or process group to be signaled. The process or processes selected by positive, negative, and zero values of the *pid* operand shall be as described for the [*kill*()](docs/posix/md/functions/kill.md) function. If process number 0 is specified, all processes in the current process group shall be signaled. For the effects of negative *pid* numbers, see the [*kill*()](docs/posix/md/functions/kill.md) function defined in the System Interfaces volume of POSIX.1-2024. If the first *pid* operand is negative, it should be preceded by `"--"` to keep it from being interpreted as an option.
>     2. A job ID (see XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id)) that identifies a process group in the case of a job-control background job, or a process ID in the case of a non-job-control background job (if supported), to be signaled. The job ID notation is applicable only for invocations of *kill* in the current shell execution environment; see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment).
>       **Note:** The job ID type of *pid* is only available on systems supporting the User Portability Utilities option or supporting non-job-control background jobs.
> - *exit_status*: A decimal integer specifying a signal number or the exit status of a process terminated by a signal.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *kill*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> When the **-l** option is not specified, the standard output shall not be used.
>
> When the **-l** option is specified, the symbolic name of each signal shall be written in the following format:
>
> ```
> "%s%c", <signal_name>, <separator>
> ```
>
> where the \<*signal_name*\> is in uppercase, without the **SIG** prefix, and the \<*separator*\> shall be either a `<newline>` or a `<space>`. For the last signal written, \<*separator*\> shall be a `<newline>`.
>
> When both the **-l** option and *exit_status* operand are specified, the symbolic name of the corresponding signal shall be written in the following format:
>
> ```
> "%s\n", <signal_name>
> ```

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: The **-l** option was specified and the output specified in STDOUT was successfully written to standard output; or, the **-l** option was not specified, at least one matching process was found for each *pid* operand, and the specified signal was successfully processed for at least one matching process.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Process numbers can be found by using [*ps*](docs/posix/md/utilities/ps.md).
>
> The use of job ID notation is not dependent on job control being enabled. When job control has been disabled using [*set*](docs/posix/md/utilities/set.md) **+m**, *kill* can still be used to signal the process group associated with a job-control background job, or the process ID associated with a non-control background job (if supported), using
>
> ```
> kill %<background job number>
> ```
>
> See also the RATIONALE for [*jobs*](docs/posix/md/utilities/jobs.md) and [*wait*](docs/posix/md/utilities/wait.md).
>
> The job ID notation is not required to work as expected when *kill* is operating in its own utility execution environment. In either of the following examples:
>
> ```
> nohup kill %1 &
> system("kill %1");
> ```
>
> the *kill* operates in a different environment and does not share the shell's understanding of job numbers.

#### EXAMPLES

> Any of the commands:
>
> ```
> kill -9 100 -165
> kill -s kill 100 -165
> kill -s KILL 100 -165
> ```
>
> sends the SIGKILL signal to the process whose process ID is 100 and to all processes whose process group ID is 165, assuming the sending process has permission to send that signal to the specified processes, and that they exist.
>
> The System Interfaces volume of POSIX.1-2024 and this volume of POSIX.1-2024 do not require specific signal numbers for any *signal_names*. Even the **-***signal_number* option provides symbolic (although numeric) names for signals. If a process is terminated by a signal, its exit status indicates the signal that killed it, but the exact values are not specified. The *kill* **-l** option, however, can be used to map decimal signal numbers and exit status values into the name of a signal. The following example reports the status of a terminated job:
>
> ```
> job
> stat=$?
> if [ $stat -eq 0 ]
> then
>     echo job completed successfully.
> elif [ $stat -gt 128 ]
> then
>     echo job terminated by signal SIG$(kill -l $stat).
> else
>     echo job terminated with error code $stat.
> fi
> ```
>
> To send the default signal to a process group (say 123), an application should use a command similar to one of the following:
>
> ```
> kill -s TERM -- -123
> kill -- -123
> ```

#### RATIONALE

> The **-l** option originated from the C shell, and is also implemented in the KornShell. The C shell output can consist of multiple output lines because the signal names do not always fit on a single line on some terminal screens. The KornShell output also included the implementation-defined signal numbers and was considered by the standard developers to be too difficult for scripts to parse conveniently. The specified output format is intended not only to accommodate the historical C shell output, but also to permit an entirely vertical or entirely horizontal listing on systems for which this is appropriate.
>
> An early proposal invented the name SIGNULL as a *signal_name* for signal 0 (used by the System Interfaces volume of POSIX.1-2024 to test for the existence of a process without sending it a signal). Since the *signal_name* 0 can be used in this case unambiguously, SIGNULL has been removed.
>
> An early proposal also required symbolic *signal_name*s to be recognized with or without the **SIG** prefix. Historical versions of *kill* have not written the **SIG** prefix for the **-l** option and have not recognized the **SIG** prefix on *signal_name*s. Since neither applications portability nor ease-of-use would be improved by requiring this extension, it is no longer required.
>
> To avoid an ambiguity of an initial negative number argument specifying either a signal number or a process group, POSIX.1-2024 mandates that it is always considered the former by implementations that support the XSI option. It also requires that conforming applications always use the `"--"` options terminator argument when specifying a process group.
>
> The **-s** option was added in response to international interest in providing some form of *kill* that meets the Utility Syntax Guidelines.
>
> The job ID notation is not required to work as expected when *kill* is operating in its own utility execution environment. In either of the following examples:
>
> ```
> nohup kill %1 &
> system("kill %1");
> ```
>
> the *kill* operates in a different environment and does not understand how the shell has managed its job numbers.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language), [*ps*](docs/posix/md/utilities/ps.md), [*wait*](docs/posix/md/utilities/wait.md#tag_20_147)
>
> XBD [*3.182 Job ID*](docs/posix/md/basedefs/V1_chap03.md#3182-job-id), [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), [*\<signal.h\>*](docs/posix/md/basedefs/signal.h.md)
>
> XSH [*kill*()](docs/posix/md/functions/kill.md#tag_17_296)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> The obsolescent versions of the SYNOPSIS are turned into non-obsolescent features of the XSI option, corresponding to a similar change in the [*trap*](docs/posix/md/utilities/V3_chap02.md#trap) special built-in.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1254 is applied, clarifying the **-l** option with regard to an *exit_status* operand corresponding to a stopped process, changing "job control job ID" to "job ID", and adding a paragraph to the RATIONALE section.
>
> Austin Group Defect 1260 is applied, changing the SYNOPSIS and EXAMPLES sections in relation to the **-s** option, and the RATIONALE section in relation to the use of `"--"` when specifying a process group.
>
> Austin Group Defect 1504 is applied, changing the EXIT STATUS section.

*End of informative text.*

### Tests

#### Test: kill with default signal sends SIGTERM

`kill` with no signal option sends SIGTERM to the process. The resulting
wait status can be decoded with `kill -l` and should identify `TERM`.

```
begin test "kill with default signal sends SIGTERM"
  script
    sleep 60 &
    pid=$!
    kill $pid
    wait $pid 2>/dev/null
    kill -l $?
  expect
    stdout ".*TERM.*"
    stderr ""
    exit_code 0
end test "kill with default signal sends SIGTERM"
```

#### Test: kill -l lists standard POSIX signals

`kill -l` writes all signal names supported by the implementation.

```
begin test "kill -l lists standard POSIX signals"
  script
    kill -l | tr ' ' '\n' | grep -c "HUP\|INT\|QUIT\|KILL\|TERM"
  expect
    stdout "[5-9].*"
    stderr ""
    exit_code 0
end test "kill -l lists standard POSIX signals"
```

#### Test: kill -0 own PID succeeds

`kill -0` tests whether the process exists without sending a signal.

```
begin test "kill -0 own PID succeeds"
  script
    kill -0 $$
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "kill -0 own PID succeeds"
```

#### Test: kill nonexistent PID exits non-zero

Sending a signal to a nonexistent PID fails.

```
begin test "kill nonexistent PID exits non-zero"
  script
    kill 99999999 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill nonexistent PID exits non-zero"
```

#### Test: kill -s TERM sends SIGTERM

Using `kill -s TERM` should send SIGTERM to the target process via the symbolic signal-name form of the `-s` option. The background process should terminate and `wait` should complete.

```
begin test "kill -s TERM sends SIGTERM"
  script
    sleep 60 & p=$!
    kill -s TERM $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -s TERM sends SIGTERM"
```

#### Test: kill -s KILL sends SIGKILL

Using `kill -s KILL` should send the uncatchable SIGKILL signal to the target process, ensuring it is immediately terminated.

```
begin test "kill -s KILL sends SIGKILL"
  script
    sleep 60 & p=$!
    kill -s KILL $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -s KILL sends SIGKILL"
```

#### Test: kill -TERM abbreviated form

The `-signal_name` shorthand (here `-TERM`) is equivalent to `-s TERM`. This verifies that the abbreviated form correctly terminates the target process.

```
begin test "kill -TERM abbreviated form"
  script
    sleep 60 & p=$!
    kill -TERM $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -TERM abbreviated form"
```

#### Test: kill -KILL abbreviated form

The `-signal_name` shorthand (here `-KILL`) is equivalent to `-s KILL`. This verifies that the abbreviated form sends SIGKILL and terminates the process.

```
begin test "kill -KILL abbreviated form"
  script
    sleep 60 & p=$!
    kill -KILL $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -KILL abbreviated form"
```

#### Test: kill -HUP abbreviated form

The `-signal_name` shorthand (here `-HUP`) is equivalent to `-s HUP`. This verifies that the abbreviated form sends SIGHUP and terminates the process.

```
begin test "kill -HUP abbreviated form"
  script
    sleep 60 & p=$!
    kill -HUP $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -HUP abbreviated form"
```

#### Test: kill -15 sends SIGTERM by number

POSIX maps signal number 15 to SIGTERM. Using `kill -15` should terminate the target process just as `-s TERM` would.

```
begin test "kill -15 sends SIGTERM by number"
  script
    sleep 60 & p=$!
    kill -15 $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -15 sends SIGTERM by number"
```

#### Test: kill -9 sends SIGKILL by number

POSIX maps signal number 9 to SIGKILL. Using `kill -9` should unconditionally kill the target process.

```
begin test "kill -9 sends SIGKILL by number"
  script
    sleep 60 & p=$!
    kill -9 $p
    wait $p 2>/dev/null
    echo done
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "kill -9 sends SIGKILL by number"
```

#### Test: kill -l 9 outputs KILL

When given a signal number, `kill -l` should print the corresponding symbolic name without the SIG prefix. For signal 9 the output should contain `KILL`.

```
begin test "kill -l 9 outputs KILL"
  script
    kill -l 9
  expect
    stdout ".*KILL.*"
    stderr ""
    exit_code 0
end test "kill -l 9 outputs KILL"
```

#### Test: kill -l 137 outputs KILL

Exit status 137 corresponds to a process killed by signal 9 (128+9). `kill -l 137` should map this exit status back to the signal name `KILL`.

```
begin test "kill -l 137 outputs KILL"
  script
    kill -l 137
  expect
    stdout ".*KILL.*"
    stderr ""
    exit_code 0
end test "kill -l 137 outputs KILL"
```

#### Test: kill -l 15 outputs TERM

When given signal number 15, `kill -l` should output the symbolic name `TERM` (SIGTERM without the SIG prefix).

```
begin test "kill -l 15 outputs TERM"
  script
    kill -l 15
  expect
    stdout ".*TERM.*"
    stderr ""
    exit_code 0
end test "kill -l 15 outputs TERM"
```

#### Test: kill -l 143 outputs TERM

Exit status 143 corresponds to a process terminated by signal 15 (128+15). `kill -l 143` should map this exit status back to the signal name `TERM`.

```
begin test "kill -l 143 outputs TERM"
  script
    kill -l 143
  expect
    stdout ".*TERM.*"
    stderr ""
    exit_code 0
end test "kill -l 143 outputs TERM"
```

#### Test: kill exits 0 on success with default signal

POSIX requires `kill` to exit 0 when the signal was successfully sent. This verifies that `kill` (with the default SIGTERM) prints `0` as its exit status after signalling a valid process.

```
begin test "kill exits 0 on success with default signal"
  script
    sleep 60 & p=$!
    kill $p
    rc=$?
    wait $p 2>/dev/null || :
    echo $rc
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "kill exits 0 on success with default signal"
```

#### Test: kill -s KILL exits 0 on success

POSIX requires `kill` to exit 0 when the signal was successfully delivered. This verifies that `kill -s KILL` reports a zero exit status after successfully signalling a process.

```
begin test "kill -s KILL exits 0 on success"
  script
    sleep 60 & p=$!
    kill -s KILL $p
    rc=$?
    wait $p 2>/dev/null || :
    echo $rc
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "kill -s KILL exits 0 on success"
```

#### Test: kill without -l produces no stdout

POSIX states that when `-l` is not specified, the standard output shall not be used. Sending a signal without `-l` should produce no stdout output.

```
begin test "kill without -l produces no stdout"
  script
    sleep 60 & p=$!
    out=$(kill -s TERM "$p")
    rc=$?
    wait "$p" 2>/dev/null || :
    if [ "$rc" -eq 0 ] && [ -z "$out" ]; then
      echo pass
    else
      echo fail
    fi
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "kill without -l produces no stdout"
```

#### Test: kill -s NONEXISTENT exits non-zero

Specifying an invalid signal name with `-s` should cause `kill` to fail and exit with a non-zero status.

```
begin test "kill -s NONEXISTENT exits non-zero"
  script
    kill -s NONEXISTENT $$ 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill -s NONEXISTENT exits non-zero"
```

#### Test: kill -NONEXISTENT exits non-zero

Using the abbreviated `-signal_name` form with a non-existent signal name should cause `kill` to fail and exit with a non-zero status.

```
begin test "kill -NONEXISTENT exits non-zero"
  script
    kill -NONEXISTENT $$ 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill -NONEXISTENT exits non-zero"
```

#### Test: kill -l output contains HUP

`kill -l` should list all supported signal names. SIGHUP is a standard POSIX signal, so the output must contain `HUP`.

```
begin test "kill -l output contains HUP"
  script
    kill -l
  expect
    stdout ".*HUP.*"
    stderr ""
    exit_code 0
end test "kill -l output contains HUP"
```

#### Test: kill -99999 invalid signal number exits non-zero

Specifying a signal number that is not defined by the implementation should cause `kill` to fail and exit with a non-zero status.

```
begin test "kill -99999 invalid signal number exits non-zero"
  script
    kill -99999 $$ 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill -99999 invalid signal number exits non-zero"
```

#### Test: kill -s 0 0 checks current process group

Signal 0 tests whether the target exists without sending a real signal. PID 0 means the current process group. This should succeed with exit status 0.

```
begin test "kill -s 0 0 checks current process group"
  script
    kill -s 0 0
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "kill -s 0 0 checks current process group"
```

#### Test: kill -0 nonexistent PID fails

Sending signal 0 to a nonexistent PID should fail with a non-zero exit status, since the target process does not exist.

```
begin test "kill -0 nonexistent PID fails"
  script
    kill -0 99999999 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill -0 nonexistent PID fails"
```

#### Test: kill -s TERM as separate option-argument

Verifies that `-s TERM` works when the signal name is passed as a separate argument (i.e., `-s` and `TERM` as two tokens), conforming to the Utility Syntax Guidelines.

```
begin test "kill -s TERM as separate option-argument"
  script
    sleep 0 & _pid=$!
    kill -s TERM $_pid 2>/dev/null
    wait $_pid 2>/dev/null
    true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "kill -s TERM as separate option-argument"
```

#### Test: kill invalid pid fails

Sending a signal to a PID that does not correspond to any running process should fail and `kill` should exit with a non-zero status.

```
begin test "kill invalid pid fails"
  script
    kill 999999 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "kill invalid pid fails"
```

#### Test: kill with job ID %1

Verifies that `kill` accepts a job ID operand (`%1`) to signal a background job by its job number, as POSIX requires for job-control environments.

```
begin interactive test "kill with job ID %1"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill %1"
  expect "$ "
  send "wait 2>/dev/null; echo killed_ok"
  expect "killed_ok"
  sendeof
  wait
end interactive test "kill with job ID %1"
```

#### Test: kill -s KILL with job ID %1

Verifies that `kill -s KILL %1` sends SIGKILL to the background job identified by job ID `%1`, combining the `-s signal_name` option with job ID notation as POSIX permits.

```
begin interactive test "kill -s KILL with job ID %1"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill -s KILL %1"
  expect "$ "
  send "wait 2>/dev/null; echo killed_ok"
  expect "killed_ok"
  sendeof
  wait
end interactive test "kill -s KILL with job ID %1"
```

#### Test: kill -s 0 with job ID checks existence

Verifies that `kill -s 0 %1` tests whether the background job exists without actually sending a signal, and exits 0 when the job is alive. Signal 0 is the POSIX-specified way to probe process existence.

```
begin interactive test "kill -s 0 with job ID checks existence"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill -s 0 %1; echo check_$?"
  expect "check_0"
  send "kill %1; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "kill -s 0 with job ID checks existence"
```

#### Test: kill %% targets current job

Verifies that `kill %%` sends the default signal (SIGTERM) to the current job. POSIX defines `%%` as equivalent to `%+`, identifying the job that would be used as the default for `fg` or `bg`.

```
begin interactive test "kill %% targets current job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill %%; echo kill_ok_$?"
  expect "kill_ok_0"
  send "wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "kill %% targets current job"
```

#### Test: kill %+ targets current job

Verifies that `kill %+` sends the default signal (SIGTERM) to the current job. POSIX defines `%+` as the job ID that identifies the current (most recent) background job.

```
begin interactive test "kill %+ targets current job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "kill %+; echo kill_ok_$?"
  expect "kill_ok_0"
  send "wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "kill %+ targets current job"
```

#### Test: kill %- targets previous job

Verifies that `kill %-` sends the default signal to the previous job (the one that would become current if the current job were to exit). POSIX defines `%-` as the job ID for the previous job.

```
begin interactive test "kill %- targets previous job"
  spawn -i
  expect "$ "
  send "set -m"
  expect "$ "
  send "sleep 60 &"
  expect "$ "
  send "sleep 61 &"
  expect "$ "
  send "kill %-; echo kill_ok_$?"
  expect "kill_ok_0"
  send "kill %+; wait 2>/dev/null"
  expect "$ "
  sendeof
  wait
end interactive test "kill %- targets previous job"
```
