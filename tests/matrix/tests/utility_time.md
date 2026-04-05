# Test Suite for Utility: time

This test suite covers the **time** utility as specified by
POSIX.1-2024. The `time` utility invokes another utility and writes
timing statistics (real, user, and system CPU time) to standard error.
When the word `time` is recognized as a reserved word in the shell, the
behavior shall be as specified for the `time` utility.

## Table of contents

- [utility: time](#utility-time)

## utility: time

#### NAME

> time — time a simple command

#### SYNOPSIS

> `time [-p] utility [argument...]`

#### DESCRIPTION

> The *time* utility shall invoke the utility named by the *utility* operand with arguments supplied as the *argument* operands and write a message to standard error that lists timing statistics for the utility. The message shall include the following information:
>
> - The elapsed (real) time between invocation of *utility* and its termination.
> - The User CPU time, equivalent to the sum of the *tms_utime* and *tms_cutime* fields returned by the [*times*()](docs/posix/md/functions/times.md) function defined in the System Interfaces volume of POSIX.1-2024 for the process in which *utility* is executed.
> - The System CPU time, equivalent to the sum of the *tms_stime* and *tms_cstime* fields returned by the [*times*()](docs/posix/md/functions/times.md) function for the process in which *utility* is executed.
>
> The precision of the timing shall be no less than the granularity defined for the size of the clock tick unit on the system, but the results shall be reported in terms of standard time units (for example, 0.02 seconds, 00:00:00.02, 1m33.75s, 365.21 seconds), not numbers of clock ticks.
>
> When *time* is used in any of the following circumstances, via a simple command for which the word **time** is the command name (see [*2.9.1.1 Order of Processing*](docs/posix/md/utilities/V3_chap02.md#2911-order-of-processing)), and none of the characters in the word **time** is quoted, the results (including parsing of later words) are unspecified:
>
> - The simple command for which the word **time** is the command name includes one or more redirections (see [*2.7 Redirection*](docs/posix/md/utilities/V3_chap02.md#27-redirection)) or is (directly) part of a pipeline (see [*2.9.2 Pipelines*](docs/posix/md/utilities/V3_chap02.md#292-pipelines)).
> - The next word that follows **time** would, if the word **time** were not present, be recognized as a reserved word (see [*2.4 Reserved Words*](docs/posix/md/utilities/V3_chap02.md#24-reserved-words)) or a control operator (see XBD [*3.85 Control Operator*](docs/posix/md/basedefs/V1_chap03.md#385-control-operator)).
>
> Since these limitations only apply when *time* is executed via a simple command for which the word **time** is the command name and none of the characters in the word **time** is quoted, they can be avoided by quoting all or part of the word **time**, by arranging for the command name not to be **time** (for example, by having the command name be a word expansion), or by executing *time* via another utility such as [*command*](docs/posix/md/utilities/command.md) or [*env*](docs/posix/md/utilities/env.md).
>
> The limitations on redirections and pipelines can also be overcome by embedding the simple command within a compound command—most commonly a grouping command (see [*2.9.4.1 Grouping Commands*](docs/posix/md/utilities/V3_chap02.md#2941-grouping-commands))—and applying the redirections or piping to the compound command instead.
>
> Note that in no circumstances where the results are specified is it possible to apply different redirections to the *time* utility than are applied to the utility it invokes.
>
> The following examples (where *a* and *b* are assumed to be the names of utilities found by searching *PATH )* show unspecified usages:
>
> ```
> time a arg1 arg2 | b    # part of a pipeline
> a | time -p b           # part of a pipeline
> time a >/dev/null       # output redirection
> </dev/null time a       # input redirection
> time while anything...  # reserved word after time
> time ( cmd )            # control operator after time
> time;                   # control operator after time
> time shift              # special built-in utility
> time -p cd /            # intrinsic utility
> ```
>
> The following examples have specified results and can be used as alternatives for the first four of the above when the *time* utility as specified here is intended to be invoked:
>
> ```
> { time a arg1 arg2; } | b
> t=time; a | $t -p b
> command time a >/dev/null
> </dev/null \time a
> ```

#### OPTIONS

> The *time* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following option shall be supported:
>
> - **-p**: Write the timing output to standard error in the format shown in the STDERR section.

#### OPERANDS

> The following operands shall be supported:
>
> - *utility*: The name of a utility that is to be invoked. If the *utility* operand names a special built-in utility (see [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities)), an intrinsic utility (see [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities)), or a function (see [*2.9.5 Function Definition Command*](docs/posix/md/utilities/V3_chap02.md#295-function-definition-command)), the results are unspecified.
> - *argument*: Any string to be supplied as an argument when invoking the utility named by the *utility* operand.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *time*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic and informative messages written to standard error.
> - *LC_NUMERIC*: Determine the locale for numeric formatting.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *PATH*: Determine the search path that shall be used to locate the utility to be invoked; see XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables).

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> Not used.

#### STDERR

> If the *utility* utility is invoked, the standard error shall be used to write the timing statistics and may be used to write a diagnostic message if the utility terminates abnormally; otherwise, the standard error shall be used to write diagnostic messages and may also be used to write the timing statistics.
>
> If **-p** is specified, the following format shall be used for the timing statistics in the POSIX locale:
>
> ```
> "real %f\nuser %f\nsys %f\n", <real seconds>, <user seconds>,
>     <system seconds>
> ```
>
> where each floating-point number shall be expressed in seconds. The precision used may be less than the default six digits of `%f`, but shall be sufficiently precise to accommodate the size of the clock tick on the system (for example, if there were 60 clock ticks per second, at least two digits shall follow the radix character). The number of digits following the radix character shall be no less than one, even if this always results in a trailing zero. The implementation may append white space and additional information following the format shown here. The implementation may also prepend a single empty line before the format shown here.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> If the *utility* utility is invoked, the exit status of *time* shall be the exit status of *utility*; otherwise, the *time* utility shall exit with one of the following values:
>
> - 1-125: An error occurred in the *time* utility.
> - 126: The utility specified by *utility* was found but could not be invoked.
> - 127: The utility specified by *utility* could not be found.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> The [*command*](docs/posix/md/utilities/command.md), [*env*](docs/posix/md/utilities/env.md), [*nice*](docs/posix/md/utilities/nice.md), [*nohup*](docs/posix/md/utilities/nohup.md), *time*, [*timeout*](docs/posix/md/utilities/timeout.md), and [*xargs*](docs/posix/md/utilities/xargs.md) utilities have been specified to use exit code 127 if a utility to be invoked cannot be found, so that applications can distinguish "failure to find a utility" from "invoked utility exited with an error indication". The value 127 was chosen because it is not commonly used for other meanings; most utilities use small values for "normal error conditions" and the values above 128 can be confused with termination due to receipt of a signal. The value 126 was chosen in a similar manner to indicate that the utility could be found, but not invoked. Some scripts produce meaningful error messages differentiating the 126 and 127 cases. The distinction between exit codes 126 and 127 is based on KornShell practice that uses 127 when all attempts to *exec* the utility fail with [ENOENT], and uses 126 when any attempt to *exec* the utility fails for any other reason.

#### EXAMPLES

> It is frequently desirable to apply *time* to pipelines or lists of commands. This can be done by placing pipelines and command lists in a single file; this file can then be invoked as a utility, and the *time* applies to everything in the file.
>
> Alternatively, the following command can be used to apply *time* to a complex command:
>
> ```
> time sh -c -- 'complex-command-line'
> ```

#### RATIONALE

> When the *time* utility was originally proposed to be included in the ISO POSIX-2:1993 standard, questions were raised about its suitability for inclusion on the grounds that it was not useful for conforming applications, specifically:
>
> - The underlying CPU definitions from the System Interfaces volume of POSIX.1-2024 are vague, so the numeric output could not be compared accurately between systems or even between invocations.
> - The creation of portable benchmark programs was outside the scope this volume of POSIX.1-2024.
>
> However, *time* does fit in the scope of user portability. Human judgement can be applied to the analysis of the output, and it could be very useful in hands-on debugging of applications or in providing subjective measures of system performance. Hence it has been included in this volume of POSIX.1-2024.
>
> The default output format has been left unspecified because historical implementations differ greatly in their style of depicting this numeric output. The **-p** option was invented to provide scripts with a common means of obtaining this information.
>
> In the KornShell, *time* is a shell reserved word that can be used to time an entire pipeline, rather than just a simple command. The POSIX definition has been worded to allow this implementation. Consideration was given to invalidating this approach because of the historical model from the C shell and System V shell. However, since the System V *time* utility historically has not produced accurate results in pipeline timing (because the constituent processes are not all owned by the same parent process, as allowed by POSIX), it did not seem worthwhile to break historical KornShell usage.
>
> The term *utility* is used, rather than *command*, to highlight the fact that shell compound commands, pipelines, special built-ins, and so on, cannot be used directly. However, *utility* includes user application programs and shell scripts, not just the standard utilities.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language), [*sh*](docs/posix/md/utilities/sh.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*times*()](docs/posix/md/functions/times.md#tag_17_629)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.

#### Issue 7

> The *time* utility is moved from the User Portability Utilities option to the Base. User Portability Utilities is now an option for interactive utilities.
>
> SD5-XCU-ERN-115 is applied, updating the example in the DESCRIPTION.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0144 [266] is applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0194 [723] is applied.

#### Issue 8

> Austin Group Defect 267 is applied, allowing **time** to be a reserved word.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1530 is applied, changing "`sh -c`" to "`sh -c --`".
>
> Austin Group Defect 1586 is applied, adding the [*timeout*](docs/posix/md/utilities/timeout.md) utility.
>
> Austin Group Defect 1594 is applied, changing the APPLICATION USAGE section.

*End of informative text.*

### Tests

#### Test: time invokes utility and passes through stdout

The `time` utility shall invoke the named utility, and the timed
utility's standard output shall still be written to stdout.

```
begin test "time invokes utility and passes through stdout"
  script
    time echo "measured"
  expect
    stdout "measured"
    stderr "(.|\n)*.+"
    exit_code 0
end test "time invokes utility and passes through stdout"
```

#### Test: time writes timing statistics to stderr

When the timed utility itself is silent, `time` shall still write its
timing statistics to standard error and not to standard output.

```
begin test "time writes timing statistics to stderr"
  script
    time true
  expect
    stdout ""
    stderr "(.|\n)*.+"
    exit_code 0
end test "time writes timing statistics to stderr"
```

#### Test: time passes arguments to utility

The *argument* operands shall be passed through unchanged to the utility
that `time` invokes.

```
begin test "time passes arguments to utility"
  script
    time sh -c 'printf "<%s><%s><%s>\n" "$1" "$2" "$3"' sh "arg 1" "*" ""
  expect
    stdout "<arg 1><\*><>"
    stderr "(.|\n)*.+"
    exit_code 0
end test "time passes arguments to utility"
```

#### Test: time uses PATH to locate utility

The `PATH` environment variable shall be used to locate the utility named
by the *utility* operand.

```
begin test "time uses PATH to locate utility"
  script
    tmpdir=$(mktemp -d)
    printf '#!/bin/sh\nprintf '\''from-path\\n'\''\n' >"$tmpdir/timed-probe"
    chmod +x "$tmpdir/timed-probe"
    PATH="$tmpdir:$PATH"
    time timed-probe
    status=$?
    rm -rf "$tmpdir"
    exit "$status"
  expect
    stdout "from-path"
    stderr "(.|\n)*.+"
    exit_code 0
end test "time uses PATH to locate utility"
```

#### Test: time preserves utility stderr on stderr

The timed utility and the `time` utility share standard error; output
written by the timed utility to stderr shall still appear there.

```
begin test "time preserves utility stderr on stderr"
  script
    time sh -c 'printf "utility-stderr\n" >&2'
  expect
    stdout ""
    stderr "(.|\n)*utility-stderr(.|\n)*"
    exit_code 0
end test "time preserves utility stderr on stderr"
```

#### Test: time -p produces POSIX format on stderr

With `-p`, the timing statistics shall be written to stderr in the POSIX
locale format using `real`, `user`, and `sys` lines with seconds-valued
floating-point numbers. This test asserts the POSIX requirement directly,
even though `bash --posix` currently deviates here.

```
begin test "time -p produces POSIX format on stderr"
  setenv "LC_ALL" "C"
  script
    time -p true
  expect
    stdout ""
    stderr "\n?real [0-9]+\.[0-9]+\nuser [0-9]+\.[0-9]+\nsys [0-9]+\.[0-9]+(.|\n)*"
    exit_code 0
end test "time -p produces POSIX format on stderr"
```

#### Test: time exit status equals invoked utility exit status

If the utility is invoked, `time` shall exit with the same status as the
timed utility, both for success and for non-zero exits.

```
begin test "time exit status equals invoked utility exit status"
  script
    time true; echo "true:$?"
    time sh -c 'exit 42'; echo "forty-two:$?"
  expect
    stdout "true:0\nforty-two:42"
    stderr "(.|\n)*"
    exit_code 0
end test "time exit status equals invoked utility exit status"
```

#### Test: time exit status 127 for utility not found

When the utility specified by the operand could not be found, `time`
shall exit with status 127.

```
begin test "time exit status 127 for utility not found"
  script
    time no_such_utility_xyzzy_$$
  expect
    stdout ""
    stderr "(.|\n)*.+"
    exit_code 127
end test "time exit status 127 for utility not found"
```

#### Test: time exit status 126 for utility not executable

When the utility specified by the operand was found but could not be
invoked (e.g. a file exists but is not executable), `time` shall exit
with status 126.

```
begin test "time exit status 126 for utility not executable"
  script
    tmp=$(mktemp)
    chmod 644 "$tmp"
    time "$tmp"
    status=$?
    rm -f "$tmp"
    exit "$status"
  expect
    stdout ""
    stderr "(.|\n)*.+"
    exit_code 126
end test "time exit status 126 for utility not executable"
```

#### Test: quoted time avoids reserved word limitations

Quoting the word `time` avoids the reserved-word limitations and causes
the shell to locate a utility named `time` through `PATH` instead.

```
begin test "quoted time avoids reserved word limitations"
  script
    tmpdir=$(mktemp -d)
    printf '#!/bin/sh\nprintf '\''quoted:%%s\\n'\'' "$*"\n' >"$tmpdir/time"
    chmod +x "$tmpdir/time"
    PATH="$tmpdir:$PATH"
    "time" echo hello
    status=$?
    rm -rf "$tmpdir"
    exit "$status"
  expect
    stdout "quoted:echo hello"
    stderr ""
    exit_code 0
end test "quoted time avoids reserved word limitations"
```

#### Test: expanded command name avoids reserved word limitations

Expanding the command name from a variable also avoids the reserved-word
limitations, because the command name is no longer the literal token
`time` during parsing.

```
begin test "expanded command name avoids reserved word limitations"
  script
    tmpdir=$(mktemp -d)
    printf '#!/bin/sh\nprintf '\''expanded:%%s\\n'\'' "$*"\n' >"$tmpdir/time"
    chmod +x "$tmpdir/time"
    PATH="$tmpdir:$PATH"
    t=time
    $t echo hello
    status=$?
    rm -rf "$tmpdir"
    exit "$status"
  expect
    stdout "expanded:echo hello"
    stderr ""
    exit_code 0
end test "expanded command name avoids reserved word limitations"
```

#### Test: grouping command allows redirection around time

The standard says the simple-command redirection limitation can be
avoided by timing a compound command and applying the redirection to that
compound command instead.

```
begin test "grouping command allows redirection around time"
  script
    tmp=$(mktemp)
    { time echo grouped-output; } 2>"$tmp"
    if [ -s "$tmp" ]; then
      echo "timing-captured"
    else
      echo "timing-missing"
    fi
    rm -f "$tmp"
  expect
    stdout "grouped-output\ntiming-captured"
    stderr ""
    exit_code 0
end test "grouping command allows redirection around time"
```

#### Test: grouping command allows pipeline around time

The standard also says the pipeline limitation can be avoided by timing a
grouping command and then piping that compound command.

```
begin test "grouping command allows pipeline around time"
  script
    lines=$({ time echo hello; } 2>&1 | wc -l)
    [ "$lines" -ge 2 ] && echo "ok" || echo "bad:$lines"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "grouping command allows pipeline around time"
```
