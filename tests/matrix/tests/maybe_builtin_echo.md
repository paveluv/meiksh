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

`echo` writes its string operands to standard output.

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

#### Test: echo hello world joins with space

Arguments are separated by single space characters and a newline
follows the last argument.

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

#### Test: echo with no arguments produces empty line

With no operands, `echo` writes only a newline.

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

#### Test: echo -- prints double dash

`echo` does not recognize `--` as end of options; it is treated as
a string operand.

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

`echo` shall not recognize `--` as an end-of-options marker; it is
treated as a regular string operand and written to standard output.

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

#### Test: echo -- hello prints both

Since `echo` does not recognize `--` as an end-of-options marker, both `--` and `hello` are treated as ordinary string operands and written to standard output separated by a space.

```
begin test "echo -- hello prints both"
  script
    echo -- hello
  expect
    stdout "-- hello"
    stderr ""
    exit_code 0
end test "echo -- hello prints both"
```

#### Test: echo -- -- prints both double dashes

Because `echo` does not support the `--` end-of-options convention, multiple `--` operands are each printed as literal strings, separated by spaces.

```
begin test "echo -- -- prints both double dashes"
  script
    echo -- --
  expect
    stdout "-- --"
    stderr ""
    exit_code 0
end test "echo -- -- prints both double dashes"
```

#### Test: echo one two three joins with spaces

Multiple string operands passed to `echo` shall be separated by single space characters in the output.

```
begin test "echo one two three joins with spaces"
  script
    echo one two three
  expect
    stdout "one two three"
    stderr ""
    exit_code 0
end test "echo one two three joins with spaces"
```

#### Test: echo single character

Verifies that `echo` correctly writes a single-character operand to standard output followed by a newline.

```
begin test "echo single character"
  script
    echo x
  expect
    stdout "x"
    stderr ""
    exit_code 0
end test "echo single character"
```

#### Test: echo -n does not crash

When the first operand is `-n`, the behavior is implementation-defined. This test verifies the shell does not crash or error out when encountering this form.

```
begin test "echo -n does not crash"
  script
    echo -n test >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo -n does not crash"
```

#### Test: echo -e does not crash

When the first operand is `-e`, the behavior is implementation-defined. This test verifies the shell does not crash or error out when encountering this form.

```
begin test "echo -e does not crash"
  script
    echo -e test >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo -e does not crash"
```

#### Test: echo -E does not crash

When the first operand is `-E`, the behavior is implementation-defined. This test verifies the shell does not crash or error out when encountering this form.

```
begin test "echo -E does not crash"
  script
    echo -E test >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo -E does not crash"
```

#### Test: echo -nee does not crash

When the first operand is a `-` followed by a combination of `n`, `e`, and `E` characters, the behavior is implementation-defined. This test verifies the shell handles `-nee` gracefully without crashing.

```
begin test "echo -nee does not crash"
  script
    echo -nee test >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo -nee does not crash"
```

#### Test: echo -neE does not crash

When the first operand is a `-` followed by a combination of `n`, `e`, and `E` characters, the behavior is implementation-defined. This test verifies the shell handles `-neE` gracefully without crashing.

```
begin test "echo -neE does not crash"
  script
    echo -neE test >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "echo -neE does not crash"
```

#### Test: echo \a produces BEL (0x07)

The POSIX standard specifies that `\a` in an `echo` operand shall write an alert (BEL) character, which is byte value 0x07.

```
begin test "echo \\a produces BEL (0x07)"
  script
    echo "\a" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*07.*"
    stderr ""
    exit_code 0
end test "echo \\a produces BEL (0x07)"
```

#### Test: echo \b produces BS (0x08)

The POSIX standard specifies that `\b` in an `echo` operand shall write a backspace character, which is byte value 0x08.

```
begin test "echo \\b produces BS (0x08)"
  script
    echo "\b" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*08.*"
    stderr ""
    exit_code 0
end test "echo \\b produces BS (0x08)"
```

#### Test: echo \f produces FF (0x0c)

The POSIX standard specifies that `\f` in an `echo` operand shall write a form-feed character, which is byte value 0x0c.

```
begin test "echo \\f produces FF (0x0c)"
  script
    echo "\f" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*0c.*"
    stderr ""
    exit_code 0
end test "echo \\f produces FF (0x0c)"
```

#### Test: echo \n produces two newlines

The POSIX standard specifies that `\n` in an `echo` operand shall write a newline character. Combined with the trailing newline that `echo` always appends, this produces two consecutive newline bytes (0x0a 0x0a).

```
begin test "echo \\n produces two newlines"
  script
    echo "\n" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*0a0a.*"
    stderr ""
    exit_code 0
end test "echo \\n produces two newlines"
```

#### Test: echo \r produces CR (0x0d)

The POSIX standard specifies that `\r` in an `echo` operand shall write a carriage-return character, which is byte value 0x0d.

```
begin test "echo \\r produces CR (0x0d)"
  script
    echo "\r" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*0d.*"
    stderr ""
    exit_code 0
end test "echo \\r produces CR (0x0d)"
```

#### Test: echo \t produces HT (0x09)

The POSIX standard specifies that `\t` in an `echo` operand shall write a tab character, which is byte value 0x09.

```
begin test "echo \\t produces HT (0x09)"
  script
    echo "\t" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*09.*"
    stderr ""
    exit_code 0
end test "echo \\t produces HT (0x09)"
```

#### Test: echo \v produces VT (0x0b)

The POSIX standard specifies that `\v` in an `echo` operand shall write a vertical-tab character, which is byte value 0x0b.

```
begin test "echo \\v produces VT (0x0b)"
  script
    echo "\v" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout ".*0b.*"
    stderr ""
    exit_code 0
end test "echo \\v produces VT (0x0b)"
```

#### Test: echo \\ produces literal backslash

The POSIX standard specifies that `\\` in an `echo` operand shall write a single literal backslash character.

```
begin test "echo \\\\ produces literal backslash"
  script
    echo "\\\\"
  expect
    stdout "\\"
    stderr ""
    exit_code 0
end test "echo \\\\ produces literal backslash"
```

#### Test: echo \065 produces ASCII 5

The POSIX standard specifies that `\0num` in an `echo` operand shall write the 8-bit value of the octal number. Octal 065 corresponds to ASCII character `5`.

```
begin test "echo \\065 produces ASCII 5"
  script
    echo "\065" 2>/dev/null
  expect
    stdout ".*5.*"
    stderr ""
    exit_code 0
end test "echo \\065 produces ASCII 5"
```

#### Test: echo \0101 produces ASCII A

The POSIX standard specifies that `\0num` in an `echo` operand shall write the 8-bit value of the octal number. Octal 0101 corresponds to ASCII character `A`.

```
begin test "echo \\0101 produces ASCII A"
  script
    echo "\0101" 2>/dev/null
  expect
    stdout ".*A.*"
    stderr ""
    exit_code 0
end test "echo \\0101 produces ASCII A"
```

#### Test: echo \c suppresses text after it

The POSIX standard specifies that `\c` suppresses the trailing newline and causes all characters following it in the arguments to be ignored. Only the text before `\c` should appear in the output.

```
begin test "echo \\c suppresses text after it"
  script
    echo "hello\c world" 2>/dev/null | od -An -tx1 | tr -d ' \n'
  expect
    stdout "68656c6c6f"
    stderr ""
    exit_code 0
end test "echo \\c suppresses text after it"
```

#### Test: echo AB\c produces exactly 2 bytes

Because `\c` suppresses the trailing newline and all characters after it, `echo "AB\c"` should produce exactly two bytes (`A` and `B`) with no newline.

```
begin test "echo AB\\c produces exactly 2 bytes"
  script
    echo "AB\c" 2>/dev/null | wc -c | tr -d ' '
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "echo AB\\c produces exactly 2 bytes"
```

#### Test: echo a b c joins with single spaces

The `echo` utility shall separate its arguments with single space characters in the output, regardless of how many spaces separated them on the command line.

```
begin test "echo a b c joins with single spaces"
  script
    echo a b c
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "echo a b c joins with single spaces"
```

#### Test: echo hello produces 6 bytes (with trailing newline)

Verifies that `echo hello` outputs exactly 6 bytes: the 5-character string `hello` plus the mandatory trailing newline.

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

When called with no arguments, `echo` writes only a newline, so the output should be exactly 1 byte.

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

#### Test: echo many arguments

Verifies that `echo` correctly joins many operands with single spaces and writes them all to standard output.

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

#### Test: echo -x prints -x

Since `echo` does not support any options, a first operand like `-x` (which is not from the implementation-defined set `{e, E, n}`) must be written literally to standard output.

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

Since `echo` does not support any options, a first operand like `-abc` (which contains characters outside the implementation-defined set `{e, E, n}`) must be written literally to standard output.

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

Since `echo` does not support any options and does not recognize `--`, the operand `--help` must be written literally to standard output.

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

#### Test: echo --version prints --version

Since `echo` does not support any options and does not recognize `--`, the operand `--version` must be written literally to standard output.

```
begin test "echo --version prints --version"
  script
    echo --version
  expect
    stdout "--version"
    stderr ""
    exit_code 0
end test "echo --version prints --version"
```
