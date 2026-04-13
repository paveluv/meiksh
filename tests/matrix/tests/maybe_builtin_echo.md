# Test Suite for Maybe-Builtin Utility: echo

This test suite covers the **echo** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: echo](#utility-echo)

## utility: echo

#### NAME

> echo — write arguments to standard output

#### SYNOPSIS

> `echo [string...]`

#### DESCRIPTION

> The *echo* utility writes its arguments to standard output, followed by a `<newline>`. If there are no arguments, only the `<newline>` is written.

#### OPTIONS

> The *echo* utility shall not recognize the `"--"` argument in the manner specified by Guideline 10 of XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines); `"--"` shall be recognized as a string operand.
>
> Implementations shall not support any options.

#### OPERANDS

> The following operands shall be supported:
>
> - *string*: A string to be written to standard output. If the first operand consists of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`}, or if any of the operands contain a `<backslash>` character, the results are implementation-defined. On XSI-conformant systems, if the first operand consists of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`}, it shall be treated as a string to be written. The following character sequences shall be recognized on XSI-conformant systems within any of the arguments:
>
>     - `\a`: Write an `<alert>`.
>     - `\b`: Write a `<backspace>`.
>     - `\c`: Suppress the `<newline>` that otherwise follows the final argument in the output. All characters following the `'\c'` in the arguments shall be ignored.
>     - `\f`: Write a `<form-feed>`.
>     - `\n`: Write a `<newline>`.
>     - `\r`: Write a `<carriage-return>`.
>     - `\t`: Write a `<tab>`.
>     - `\v`: Write a `<vertical-tab>`.
>     - `\\`: Write a `<backslash>` character.
>     - `\0`*num*: Write an 8-bit value that is the zero, one, two, or three-digit octal number *num*.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *echo*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The *echo* utility arguments shall be separated by single `<space>` characters and a `<newline>` character shall follow the last argument. Output transformations shall occur based on the escape sequences in the input. See the OPERANDS section.

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

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> It is not possible to use *echo* portably across all POSIX systems unless escape sequences are omitted, and the first argument does not consist of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`}.
>
> The [*printf*](docs/posix/md/utilities/printf.md) utility can be used portably to emulate any of the traditional behaviors of the *echo* utility as follows (assuming that *IFS* has its standard value or is unset):
>
> - The historic System V *echo* and the requirements on XSI implementations in this volume of POSIX.1-2024 are equivalent to:
>   ```
>   printf "%b\n" "$*"
>   ```
> - The BSD *echo* is equivalent to:
>   ```
>   if [ "X$1" = "X-n" ]
>   then
>       shift
>       printf "%s" "$*"
>   else
>       printf "%s\n" "$*"
>   fi
>   ```
>
> New applications are encouraged to use [*printf*](docs/posix/md/utilities/printf.md) instead of *echo*.

#### EXAMPLES

> None.

#### RATIONALE

> The *echo* utility has not been made obsolescent because of its extremely widespread use in historical applications. Conforming applications that wish to do prompting without `<newline>` characters or that could possibly be expecting to echo a string consisting of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`} should use the [*printf*](docs/posix/md/utilities/printf.md) utility.
>
> At the time that the IEEE Std 1003.2-1992 standard was being developed, the two different historical versions of *echo* that were considered for standardization varied in incompatible ways.
>
> The BSD *echo* checked the first argument for the string **-n** which caused it to suppress the `<newline>` that would otherwise follow the final argument in the output.
>
> The System V *echo* treated all arguments as strings to be written, but allowed escape sequences within them, as described for XSI implementations in the OPERANDS section, including `\c` to suppress a trailing `<newline>`.
>
> Thus the IEEE Std 1003.2-1992 standard said that the behavior was implementation-defined if the first operand is **-n** or if any of the operands contain a `<backslash>` character. It also specified that the *echo* utility does not support Utility Syntax Guideline 10 because historical applications depended on *echo* to echo *all* of its arguments, except for the **-n** first argument in the BSD version.
>
> The Single UNIX Specification, Version 1 required the System V behavior, and this became the XSI requirement when Version 2 and POSIX.2 were merged with POSIX.1 to form the joint IEEE Std 1003.1-2001 / Single UNIX Specification, Version 3 standard.
>
> This standard now treats a first operand of **-e** or **-E** the same as **-n** in recognition that support for them has become more widespread in non-XSI implementations. Where supported, **-e** enables processing of escape sequences in the remaining operands (in situations where it is disabled by default), and **-E** disables it (in situations where it is enabled by default). A first operand containing a combination of these three letters, in the same manner as option grouping, also results in implementation-defined behavior.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*printf*](docs/posix/md/utilities/printf.md#tag_20_96)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 5

> In the OPTIONS section, the last sentence is changed to indicate that implementations "do not" support any options; in the previous issue this said "need not".

#### Issue 6

> The following new requirements on POSIX implementations derive from alignment with the Single UNIX Specification:
>
> - A set of character sequences is defined as *string* operands.
> - *LC_CTYPE* is added to the list of environment variables affecting *echo*.
> - In the OPTIONS section, implementations shall not support any options.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/21 is applied, so that the *echo* utility can accommodate historical BSD behavior.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1222 is applied, making the results implementation-defined, on systems that are not XSI-conformant, if the first operand consists of a `'-'` followed by one or more characters from the set {`'e'`, `'E'`, `'n'`}.

*End of informative text.*

### Tests

#### Test: echo hello

Verifies that `echo` writes its argument to standard output followed by a newline (POSIX: "The echo utility writes its arguments to standard output, followed by a newline").

```
begin test "echo hello"
  script
    echo hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "echo hello"
```

#### Test: echo with no arguments produces empty line

Verifies that when called with no arguments, `echo` writes only a newline (POSIX: "If there are no arguments, only the newline is written").

```
begin test "echo with no arguments produces empty line"
  script
    echo
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo with no arguments produces empty line"
```

#### Test: echo hello world joins with space

Verifies that arguments are separated by single space characters in the output (POSIX: "arguments shall be separated by single space characters").

```
begin test "echo hello world joins with space"
  script
    echo hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "echo hello world joins with space"
```

#### Test: echo many arguments

Verifies that `echo` correctly joins many operands with single space characters (POSIX: "arguments shall be separated by single space characters").

```
begin test "echo many arguments"
  script
    echo 1 2 3 4 5
  expect
    stdout "1 2 3 4 5"
    stderr ""
    exit_code 0
end test "echo many arguments"
```

#### Test: echo -- prints double dash

Verifies that `echo` does not recognize `--` as an end-of-options marker; it is treated as a string operand (POSIX: "echo shall not recognize the `--` argument in the manner specified by Guideline 10").

```
begin test "echo -- prints double dash"
  script
    echo --
  expect
    stdout "--"
    stderr ""
    exit_code 0
end test "echo -- prints double dash"
```

#### Test: echo -- hello treats -- as operand

Verifies that `--` is recognized as a string operand and printed alongside other arguments (POSIX: "`--` shall be recognized as a string operand").

```
begin test "echo -- hello treats -- as operand"
  script
    echo -- hello
  expect
    stdout "-- hello"
    stderr ""
    exit_code 0
end test "echo -- hello treats -- as operand"
```

#### Test: echo -x prints -x

Verifies that `echo` does not support options. A first operand like `-x` (which is not from the implementation-defined set `{e, E, n}`) must be written literally (POSIX: "Implementations shall not support any options").

```
begin test "echo -x prints -x"
  script
    echo -x
  expect
    stdout "-x"
    stderr ""
    exit_code 0
end test "echo -x prints -x"
```

#### Test: echo -abc prints -abc

Verifies that `echo` does not support options. A first operand like `-abc` (containing characters outside `{e, E, n}`) must be written literally (POSIX: "Implementations shall not support any options").

```
begin test "echo -abc prints -abc"
  script
    echo -abc
  expect
    stdout "-abc"
    stderr ""
    exit_code 0
end test "echo -abc prints -abc"
```

#### Test: echo --help prints --help

Verifies that `echo` does not support `--help` as an option. Since `echo` does not recognize `--` or any options, the operand `--help` must be written literally (POSIX: "Implementations shall not support any options").

```
begin test "echo --help prints --help"
  script
    echo --help
  expect
    stdout "--help"
    stderr ""
    exit_code 0
end test "echo --help prints --help"
```

#### Test: echo hello produces 6 bytes (with trailing newline)

Verifies that the output includes a mandatory trailing newline by checking byte count: `hello` (5 bytes) plus newline (1 byte) equals 6 (POSIX: "a newline character shall follow the last argument").

```
begin test "echo hello produces 6 bytes (with trailing newline)"
  script
    echo hello 2>/dev/null | wc -c | tr -d ' '
  expect
    stdout "6"
    stderr ""
    exit_code 0
end test "echo hello produces 6 bytes (with trailing newline)"
```

#### Test: echo with no args produces 1 byte (newline only)

Verifies at the byte level that `echo` with no arguments produces exactly 1 byte (the newline), confirming both statement 2 and statement 9 (POSIX: "If there are no arguments, only the newline is written").

```
begin test "echo with no args produces 1 byte (newline only)"
  script
    echo 2>/dev/null | wc -c | tr -d ' '
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "echo with no args produces 1 byte (newline only)"
```
