# Test Suite for 2.15 Special Built-In: times

This test suite covers the **times** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities times](#215-special-built-in-utilities-times)

## 2.15 Special Built-In Utilities times

#### NAME

> times — write process times

#### SYNOPSIS

> `times`

#### DESCRIPTION

> The [*times*](#times) utility shall write the accumulated user and system times for the shell and for all of its child processes, in the following POSIX locale format:
>
> ```
> "%dm%fs %dm%fs\n%dm%fs %dm%fs\n", <shell user minutes>,
>     <shell user seconds>, <shell system minutes>,
>     <shell system seconds>, <children user minutes>,
>     <children user seconds>, <children system minutes>,
>     <children system seconds>
> ```
>
> The four pairs of times shall correspond to the members of the [*\<sys/times.h\>*](docs/posix/md/basedefs/sys_times.h.md) **tms** structure (defined in XBD [*14. Headers*](docs/posix/md/basedefs/V1_chap14.md#14-headers)) as returned by [*times*()](docs/posix/md/functions/times.md): *tms_utime*, *tms_stime*, *tms_cutime*, and *tms_cstime*, respectively.

#### OPTIONS

> None.

#### OPERANDS

> None.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> None.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> See the DESCRIPTION.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> - 0: Successful completion.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> ```
> $
>  times
>
> 0m0.43s 0m1.11s
> 8m44.18s 1m43.23s
> ```

#### RATIONALE

> The [*times*](#times) special built-in from the Single UNIX Specification is now required for all conforming shells.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)
>
> XBD [*\<sys/times.h\>*](docs/posix/md/basedefs/sys_times.h.md)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/9 is applied, changing text in the DESCRIPTION from: "Write the accumulated user and system times for the shell and for all of its child processes ..." to: "The [*times*](#times) utility shall write the accumulated user and system times for the shell and for all of its child processes ...".

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0056 [960] is applied.

*End of informative text.*

### Tests

#### Test: times produces output with time format

The `times` utility writes accumulated user and system times.

```
begin test "times produces output with time format"
  script
    times 2>/dev/null
  expect
    stdout ".*m.*s.*\n.*m.*s.*"
    stderr ""
    exit_code 0
end test "times produces output with time format"
```
