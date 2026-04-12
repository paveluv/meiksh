# Test Suite for Intrinsic Utility: ulimit

This test suite covers the **ulimit** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: ulimit](#utility-ulimit)

## utility: ulimit

#### NAME

> ulimit — report or set resource limits

#### SYNOPSIS

> ```
> ulimit [-H|-S] -a
> [XSI] ulimit [-H|-S] [-c|-d|-f|-n|-s|-t|-v] [newlimit]
> ```

#### DESCRIPTION

> The *ulimit* utility shall report or set the resource limits in effect in the process in which it is executed.
>
> Soft limits can be changed by a process to any value that is less than or equal to the hard limit. A process can (irreversibly) lower its hard limit to any value that is greater than or equal to the soft limit. Only a process with appropriate privileges can raise a hard limit.
>
> The value **unlimited** for a resource shall be considered to be larger than any other limit value. When a resource has this limit value, the implementation shall not enforce limits on that resource. In locales other than the POSIX locale, *ulimit* may support additional non-numeric values with the same meaning as **unlimited**.
>
> The behavior when resource limits are exceeded shall be as described in the System Interfaces volume of POSIX.1-2024 for the [*setrlimit*()](docs/posix/md/functions/setrlimit.md) function.

#### OPTIONS

> The *ulimit* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), except that:
>
> - The order in which options other than **-H**, **-S**, and **-a** are specified may be significant.
> - Conforming applications shall specify each option separately; that is, grouping option letters (for example, **-fH**) need not be recognized by all implementations.
>
> The following options shall be supported:
>
> - **-H**: Report hard limit(s) or set only a hard limit.
> - **-S**: Report soft limit(s) or set only a soft limit.
> - **-a**: Report the limit value for all of the resources named below and for any implementation-specific additional resources.
> - **-c**: Report, or set if the *newlimit* operand is present, the core image size limit(s) in units of 512 bytes. [RLIMIT_CORE]
> - **-d**: Report, or set if the *newlimit* operand is present, the data segment size limit(s) in units of 1024 bytes. [RLIMIT_DATA]
> - **-f**: Report, or set if the *newlimit* operand is present, the file size limit(s) in units of 512 bytes. [RLIMIT_FSIZE]
> - **-n**: Report, or set if the *newlimit* operand is present, the limit(s) on the number of open file descriptors, given as a number one greater than the maximum value that the system assigns to a newly-created descriptor. [RLIMIT_NOFILE]
> - **-s**: Report, or set if the *newlimit* operand is present, the stack size limit(s) in units of 1024 bytes. [RLIMIT_STACK]
> - **-t**: Report, or set if the *newlimit* operand is present, the per-process CPU time limit(s) in units of seconds. [RLIMIT_CPU]
> - **-v**: Report, or set if the *newlimit* operand is present, the address space size limit(s) in units of 1024 bytes. [RLIMIT_AS]
>
> Where an option description is followed by [RLIMIT_*name*] it indicates which resource for the [*getrlimit*()](docs/posix/md/functions/getrlimit.md) and [*setrlimit*()](docs/posix/md/functions/setrlimit.md) functions, defined in the System Interfaces volume of POSIX.1-2024, the option corresponds to.
>
> If neither the **-H** nor **-S** option is specified:
>
> - If the *newlimit* operand is present, it shall be used as the new value for both the hard and soft limits.
> - If the *newlimit* operand is not present, **-S** shall be the default.
>
> If no options other than **-H** or **-S** are specified, the behavior shall be as if the **-f** option was (also) specified.
>
> If any option other than **-H** or **-S** is repeated, the behavior is unspecified.

#### OPERANDS

> The following operand shall be supported:
>
> - *newlimit*: Either an integer value to use as the new limit(s) for the specified resource, in the units specified in OPTIONS, or a non-numeric string indicating no limit, as described in the DESCRIPTION section. Numerals in the range 0 to the maximum limit value supported by the implementation for any resource shall be syntactically recognized as numeric values.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *ulimit*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The standard output shall be used when no *newlimit* operand is present.
>
> If the **-a** option is specified, the output written for each resource shall consist of one line that includes:
>
> - A short phrase identifying the resource (for example "file size").
> - An indication of the units used for the resource, if the corresponding option description in OPTIONS specifies the units to be used.
> - The *ulimit* option used to specify the resource.
> - The limit value.
>
> The format used within each line is unspecified, except that the format used for the limit value shall be as described below for the case where a single limit value is written.
>
> If a single limit value is to be written; that is, the **-a** option is not specified and at most one option other than **-H** or **-S** is specified:
>
> - If the resource being reported has a numeric limit, the limit value shall be written in the following format: where \<*limit value*\> is the value of the limit in the units specified in OPTIONS.
>   ```
>   "%1d\n", <limit value>
>   ```
> - If the resource being reported does not have a numeric limit, in the POSIX locale the following format shall be used:
>   ```
>   "unlimited\n"
>   ```

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
> - \>0: A request for a higher limit was rejected or an error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *ulimit* affects the current shell execution environment, it is always provided as a shell regular built-in. If it is called with an operand in a separate utility execution environment, such as one of the following:
>
> ```
> nohup ulimit -f 10000
> env ulimit -S -c 10000
> ```
>
> it does not affect the limit(s) in the caller's environment.
>
> See also the APPLICATION USAGE for [*getrlimit*()](docs/posix/md/functions/getrlimit.md).

#### EXAMPLES

> Set the hard and soft file size limits to 51200 bytes:
>
> ```
> ulimit -f 100
> ```
>
> Save and restore a soft resource limit (where *X* is an option letter specifying a resource):
>
> ```
> saved=$(ulimit -X)
>
> ...
>
> ulimit -X -S "$saved"
> ```
>
> Execute a utility with a CPU limit of 5 minutes (using an asynchronous subshell to ensure the limit is set in a child process):
>
> ```
> (ulimit -t 300; exec utility_name </dev/null) &
> wait $!
> ```

#### RATIONALE

> The *ulimit* utility has no equivalent of the special values RLIM_SAVED_MAX and RLIM_SAVED_CUR returned by [*getrlimit*()](docs/posix/md/functions/getrlimit.md), as *ulimit* is required to be able to output, and accept as input, all numeric limit values supported by the system.
>
> Implementations differ in their behavior when the **-a** option is not specified and more than one option other than **-H** or **-S** is specified. Some write output for all of the specified resources in the same format as for **-a**; others write only the value for the last specified option. Both behaviors are allowed by the standard, since the SYNOPSIS lists the options as mutually exclusive.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*getrlimit*()](docs/posix/md/functions/getrlimit.md)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1418 is applied, adding the **-H**, **-S**, **-a**, **-c**, **-d**, **-n**, **-s**, **-t**, and **-v** options, and relating the **-f** option to the RLIMIT_FSIZE resource for [*setrlimit*()](docs/posix/md/functions/setrlimit.md).
>
> Austin Group Defect 1669 is applied, moving the *ulimit* utility, excluding the **-t** option, from the XSI option to the Base.

*End of informative text.*

### Tests

#### Test: ulimit -f produces output

`ulimit -f` reports the file size limit.

```
begin test "ulimit -f produces output"
  script
    ulimit -f
  expect
    stdout ".+"
    stderr ""
    exit_code 0
end test "ulimit -f produces output"
```

#### Test: ulimit -a lists multiple limits

`ulimit -a` reports all current limits.

```
begin test "ulimit -a lists multiple limits"
  script
    lines=$(ulimit -a | wc -l)
    test "$lines" -gt 1 && echo "multiple"
  expect
    stdout "multiple"
    stderr ""
    exit_code 0
end test "ulimit -a lists multiple limits"
```

#### Test: ulimit -f unlimited succeeds

Setting the file size limit to `unlimited` succeeds.

```
begin test "ulimit -f unlimited succeeds"
  script
    ulimit -f unlimited
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "ulimit -f unlimited succeeds"
```

#### Test: ulimit -S -f prints soft file-size limit

When `-S` is combined with `-f`, ulimit reports the soft file-size limit. The command should succeed and produce output without error.

```
begin test "ulimit -S -f prints soft file-size limit"
  script
    ulimit -S -f >/dev/null
    echo "pass"
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "ulimit -S -f prints soft file-size limit"
```

#### Test: ulimit -a succeeds

`ulimit -a` reports all resource limits and must exit with status 0.

```
begin test "ulimit -a succeeds"
  script
    ulimit -a >/dev/null
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "ulimit -a succeeds"
```

#### Test: ulimit options specified separately

POSIX requires that conforming applications specify each ulimit option separately (e.g. `-S -f` rather than `-Sf`). This verifies that separately specified options work correctly.

```
begin test "ulimit options specified separately"
  script
    ulimit -S -f >/dev/null && echo pass
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "ulimit options specified separately"
```

#### Test: ulimit with invalid value produces diagnostic

Setting a resource limit to a non-numeric, non-`unlimited` string is invalid. The shell must write a diagnostic message to standard error in this case.

```
begin test "ulimit with invalid value produces diagnostic"
  script
    err=$(ulimit -f not_a_number 2>&1)
    [ -n "$err" ] && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "ulimit with invalid value produces diagnostic"
```

#### Test: ulimit -f reports unlimited after setting unlimited

After setting the file-size limit to `unlimited`, querying it with `ulimit -f` must print `unlimited` to confirm the value took effect.

```
begin test "ulimit -f reports unlimited after setting unlimited"
  script
    ulimit -f unlimited
    ulimit -f
  expect
    stdout "unlimited"
    stderr ""
    exit_code 0
end test "ulimit -f reports unlimited after setting unlimited"
```

#### Test: setting soft limit to its current value succeeds

Re-setting the soft file-size limit to its current value is always valid (it is at most equal to the hard limit) and must succeed with exit status 0.

```
begin test "setting soft limit to its current value succeeds"
  script
    ulimit -Sf $(ulimit -Sf)
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "setting soft limit to its current value succeeds"
```

#### Test: setting soft limit to unlimited succeeds when hard is unlimited

A soft limit can be set to any value up to and including the hard limit. When the hard limit is `unlimited`, setting the soft limit to `unlimited` must succeed.

```
begin test "setting soft limit to unlimited succeeds when hard is unlimited"
  script
    hard=$(ulimit -Hf)
    if [ "$hard" = "unlimited" ]; then
      ulimit -Sf unlimited && echo pass
    else
      echo pass
    fi
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "setting soft limit to unlimited succeeds when hard is unlimited"
```

#### Test: setting limit to non-numeric string fails

The newlimit operand must be either an integer or the special string meaning no limit (e.g. `unlimited`). Any other non-numeric string is invalid and the command must exit with a non-zero status.

```
begin test "setting limit to non-numeric string fails"
  script
    ulimit -f notanumber
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "setting limit to non-numeric string fails"
```

#### Test: ulimit -f returns numeric or unlimited

When no newlimit operand is given, `ulimit -f` must print either a decimal integer (the limit in 512-byte units) or `unlimited`. No other output format is valid.

```
begin test "ulimit -f returns numeric or unlimited"
  script
    val=$(ulimit -f)
    case "$val" in unlimited) echo pass ;; *[!0-9]*) echo fail ;; *) echo pass ;; esac
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "ulimit -f returns numeric or unlimited"
```

#### Test: unlimited hard limit allows setting arbitrary soft limit

When the hard limit is `unlimited`, the soft limit can be freely changed to any numeric value and back to `unlimited`, because a soft limit may be set to any value up to the hard limit.

```
begin test "unlimited hard limit allows setting arbitrary soft limit"
  script
    hard=$(ulimit -Hf)
    if [ "$hard" = "unlimited" ]; then
      ulimit -Sf 12345 && ulimit -Sf unlimited && echo pass
    else
      echo pass
    fi
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "unlimited hard limit allows setting arbitrary soft limit"
```

#### Test: 0 is a valid limit

Numerals in the range 0 to the maximum supported limit value are valid. Setting the soft file-size limit to 0 must succeed.

```
begin test "0 is a valid limit"
  script
    ulimit -Sf 0
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "0 is a valid limit"
```

#### Test: setting soft limit to 0 takes effect

After setting the soft file-size limit to 0, querying it must report `0`, confirming the new limit value actually took effect.

```
begin test "setting soft limit to 0 takes effect"
  script
    ulimit -Sf 0
    ulimit -Sf
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "setting soft limit to 0 takes effect"
```

#### Test: current soft limit is a valid numeric value to re-set

The current soft limit value, obtained via command substitution, must itself be a valid newlimit operand that can be passed back to `ulimit -Sf` without error.

```
begin test "current soft limit is a valid numeric value to re-set"
  script
    cur=$(ulimit -Sf)
    if [ "$cur" != "unlimited" ]; then
      ulimit -Sf $cur && echo pass
    else
      echo pass
    fi
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "current soft limit is a valid numeric value to re-set"
```

#### Test: ulimit -a lines each contain a limit value

Every non-empty line produced by `ulimit -a` must include a limit value -- either a number or the string `unlimited` -- as required by the POSIX output specification for the `-a` option.

```
begin test "ulimit -a lines each contain a limit value"
  script
    bad=$(ulimit -a | while IFS= read -r line; do case "$line" in *unlimited*|*[0-9]*) ;; "") ;; *) echo "bad: $line" ;; esac
    done)
    [ -z "$bad" ] && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "ulimit -a lines each contain a limit value"
```
