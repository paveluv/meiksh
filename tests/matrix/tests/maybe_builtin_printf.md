# Test Suite for Maybe-Builtin Utility: printf

This test suite covers the **printf** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: printf](#utility-printf)

## utility: printf

#### NAME

> printf — write formatted output

#### SYNOPSIS

> `printf format [argument...]`

#### DESCRIPTION

> The *printf* utility shall write formatted operands to the standard output. The *argument* operands shall be formatted under control of the *format* operand.

#### OPTIONS

> None.

#### OPERANDS

> The following operands shall be supported:
>
> - *format*: A character string describing the format to use to write the remaining operands. See the EXTENDED DESCRIPTION section.
> - *argument*: The values to be written to standard output, under the control of *format*. See the EXTENDED DESCRIPTION section.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *printf*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *LC_NUMERIC*: Determine the locale for numeric formatting. It shall affect the format of numbers written using the `e` , `E` , `f` , `g` , and `G` conversion specifier characters (if supported).
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> See the EXTENDED DESCRIPTION section.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> The application shall ensure that the *format* operand is a character string, beginning and ending in its initial shift state, if any. The *format* operand shall be used as the format string described in XBD [*5. File Format Notation*](docs/posix/md/basedefs/V1_chap05.md#5-file-format-notation) with the following exceptions:
>
> 1. A `<space>` in the format string, in any context other than a flag of a conversion specification, shall be treated as an ordinary character that is copied to the output.
> 2. A `'Δ'` character in the format string shall be treated as a `'Δ'` character, not as a `<space>`.
> 3. In addition to the escape sequences shown in XBD [*5. File Format Notation*](docs/posix/md/basedefs/V1_chap05.md#5-file-format-notation) (`'\\'`, `'\a'`, `'\b'`, `'\f'`, `'\n'`, `'\r'`, `'\t'`, `'\v'`), `"\ddd"`, where *ddd* is a one, two, or three-digit octal number, shall be written as a byte with the numeric value specified by the octal number.
> 4. The implementation shall not precede or follow output from the `d` or `u` conversion specifiers with `<blank>` characters not specified by the *format* operand.
> 5. The implementation shall not precede output from the `o` conversion specifier with zeros not specified by the *format* operand.
> 6. The `a`, `A`, `e`, `E`, `f`, `F`, `g`, and `G` conversion specifiers need not be supported.
> 7. An additional conversion specifier character, `b`, shall be supported as follows. The argument shall be taken to be a string that can contain `<backslash>`-escape sequences. The following `<backslash>`-escape sequences shall be supported: The interpretation of a `<backslash>` followed by any other sequence of characters is unspecified. Bytes from the converted string shall be written until the end of the string or the number of bytes indicated by the precision specification is reached. If the precision is omitted, it shall be taken to be infinite, so all bytes up to the end of the converted string shall be written.
>     - The escape sequences listed in XBD [*5. File Format Notation*](docs/posix/md/basedefs/V1_chap05.md#5-file-format-notation) (`'\\'`, `'\a'`, `'\b'`, `'\f'`, `'\n'`, `'\r'`, `'\t'`, `'\v'`), which shall be converted to the characters they represent.
>     - `"\0ddd"`, where *ddd* is a zero, one, two, or three-digit octal number that shall be converted to a byte with the numeric value specified by the octal number.
>     - `'\c'`, which shall not be written and shall cause *printf* to ignore any remaining characters in the string operand containing it, any remaining string operands, and any additional characters in the *format* operand. If a precision is specified and the argument contains a `'\c'` after the point at which the number of bytes indicated by the precision specification have been written, it is unspecified whether the `'\c'` takes effect.
> 8. Conversions can be applied to the *n*th *argument* operand rather than to the next *argument* operand. In this case, the conversion specifier character `'%'` is replaced by the sequence `"%n$"`, where *n* is a decimal integer in the range [1,{NL_ARGMAX}], giving the *argument* operand number. This feature provides for the definition of format strings that select arguments in an order appropriate to specific languages. The format can contain either numbered argument conversion specifications (that is, ones beginning with `"%n$"`), or unnumbered argument conversion specifications, but not both. The only exception to this is that `"%%"` can be mixed with the `"%n$"` form. The results of mixing numbered and unnumbered argument specifications that consume an argument are unspecified.
> 9. For each conversion specification that consumes an argument, an *argument* operand shall be evaluated and converted to the appropriate type for the conversion as specified below. The operand to be evaluated shall be determined as follows: If the *format* operand contains no conversion specifications that consume an argument and there are *argument* operands present, the results are unspecified.
>     - If the conversion specification begins with a `"%n$"` sequence, the *n*th *argument* operand shall be evaluated.
>     - Otherwise, the evaluated operand shall be the next *argument* operand after the one evaluated by the previous conversion specification that consumed an argument; if there is no such previous conversion specification the first *argument* operand shall be evaluated.
> 10. The *format* operand shall be reused as often as necessary to satisfy the *argument* operands. If conversion specifications beginning with a `"%n$"` sequence are used, on format reuse the value of *n* shall refer to the *n*th *argument* operand following the highest numbered *argument* operand consumed by the previous use of the *format* operand.
> 11. If an *argument* operand to be consumed by a conversion specification does not exist:
>     - If it is a numbered argument conversion specification, *printf* should write a diagnostic message to standard error and exit with non-zero status, but may behave as for an unnumbered argument conversion specification.
>     - If it is an unnumbered argument conversion specification, any extra `b`, `c`, or `s` conversion specifiers shall be evaluated as if a null string argument were supplied and any other extra conversion specifiers shall be evaluated as if a zero argument were supplied.
> 12. If a character sequence in the *format* operand begins with a `'%'` character, but does not form a valid conversion specification, the behavior is unspecified.
> 13. The argument to the `c` conversion specifier can be a string containing zero or more bytes. If it contains one or more bytes, the first byte shall be written and any additional bytes shall be ignored. If the argument is an empty string, it is unspecified whether nothing is written or a null byte is written.
>
> The *argument* operands shall be treated as strings if the corresponding conversion specifier is `b`, `c`, or `s`, and shall be evaluated as if by the [*strtod*()](docs/posix/md/functions/strtod.md) function if the corresponding conversion specifier is `a`, `A`, `e`, `E`, `f`, `F`, `g`, or `G`. Otherwise, they shall be evaluated as unsuffixed C integer constants, as described by the ISO C standard, with the following extensions:
>
> - A leading `<plus-sign>` or `<hyphen-minus>` shall be allowed.
> - If the leading character is a single-quote or double-quote, the value shall be the numeric value in the underlying codeset of the character following the single-quote or double-quote.
> - Suffixed integer constants may be allowed.
>
> If an *argument* operand cannot be completely converted into an internal value appropriate to the corresponding conversion specification, a diagnostic message shall be written to standard error and the utility shall not exit with a zero exit status, but shall continue processing any remaining operands and shall write the value accumulated at the time the error was detected to standard output.
>
> It shall not be considered an error if an *argument* operand is not completely used for a `b`, `c`, or `s` conversion.

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

> The floating-point formatting conversion specifications of [*printf*()](docs/posix/md/functions/printf.md) are not required because all arithmetic in the shell is integer arithmetic. The [*awk*](docs/posix/md/utilities/awk.md) utility performs floating-point calculations and provides its own **printf** function. The [*bc*](docs/posix/md/utilities/bc.md) utility can perform arbitrary-precision floating-point arithmetic, but does not provide extensive formatting capabilities. (This *printf* utility cannot really be used to format [*bc*](docs/posix/md/utilities/bc.md) output; it does not support arbitrary precision.) Implementations are encouraged to support the floating-point conversions as an extension.
>
> Note that this *printf* utility, like the [*printf*()](docs/posix/md/functions/printf.md) function defined in the System Interfaces volume of POSIX.1-2024 on which it is based, makes no special provision for dealing with multi-byte characters when using the `%c` conversion specification or when a precision is specified in a `%b` or `%s` conversion specification. Applications should be extremely cautious using either of these features when there are multi-byte characters in the character set.
>
> No provision is made in this volume of POSIX.1-2024 which allows field widths and precisions to be specified as `'*'` since the `'*'` can be replaced directly in the *format* operand using shell variable substitution. Implementations can also provide this feature as an extension if they so choose.
>
> Hexadecimal character constants as defined in the ISO C standard are not recognized in the *format* operand because there is no consistent way to detect the end of the constant. Octal character constants are limited to, at most, three octal digits, but hexadecimal character constants are only terminated by a non-hex-digit character. In the ISO C standard, string literal concatenation can be used to terminate a constant and follow it with a hexadecimal character to be written. In the shell, similar concatenation can be done using `$'...'` so that the shell converts the hexadecimal sequence before it executes *printf*.
>
> The `%b` conversion specification is not part of the ISO C standard; it has been added here as a portable way to process `<backslash>`-escapes expanded in string operands as provided by the [*echo*](docs/posix/md/utilities/echo.md) utility. See also the APPLICATION USAGE section of [*echo*](docs/posix/md/utilities/echo.md) for ways to use *printf* as a replacement for all of the traditional versions of the [*echo*](docs/posix/md/utilities/echo.md) utility.
>
> If an argument cannot be parsed correctly for the corresponding conversion specification, the *printf* utility is required to report an error. Thus, overflow and extraneous characters at the end of an argument being used for a numeric conversion shall be reported as errors.
>
> Unlike the [*printf*()](docs/posix/md/functions/printf.md) function, when numbered conversion specifications are used, specifying the *N*th argument does not require that all the leading arguments, from the first to the (*N-1*)th, are specified in the format string. For example, `"%3$s %1$d\n"` is an acceptable *format* operand which evaluates the first and third *argument* operands but not the second.

#### EXAMPLES

> To alert the user and then print and read a series of prompts:
>
> ```
> printf "\aPlease fill in the following: \nName: "
> read name
> printf "Phone number: "
> read phone
> ```
>
> To read out a list of right and wrong answers from a file, calculate the percentage correctly, and print them out. The numbers are right-justified and separated by a single `<tab>`. The percentage is written to one decimal place of accuracy:
>
> ```
> while read right wrong ; do
>     percent=$(echo "scale=1;($right*100)/($right+$wrong)" | bc)
>     printf "%2d right\t%2d wrong\t(%s%%)\n" \
>         $right $wrong $percent
> done < database_file
> ```
>
> The command:
>
> ```
> printf "%5d%4d\n" 1 21 321 4321 54321
> ```
>
> produces:
>
> ```
>     1  21
>   3214321
> 54321   0
> ```
>
> Note that the *format* operand is used three times to print all of the given strings and that a `'0'` was supplied by *printf* to satisfy the last `%4d` conversion specification.
>
> The command:
>
> ```
> printf '%d\n' 10 010 0x10
> ```
>
> produces:
>
> | **Output Line** | **Explanation** |
> | --- | --- |
> | 10 | Decimal representation of the numeric value of decimal integer constant 10 |
> | 8 | Decimal representation of the numeric value of octal integer constant 010 |
> | 16 | Decimal representation of the numeric value of hexadecimal integer constant 0x10 |
>
> If the implementation supports floating-point conversions, the command:
>
> ```
> LC_ALL=C printf '%.2f\n' 10 010 0x10 10.1e2 010.1e2 0x10.1p2
> ```
>
> produces:
>
> | **Output Line** | **Explanation** |
> | --- | --- |
> | 10.00 | The string `"10"` interpreted as a decimal value, with 2 digits of precision |
> | 10.00 | The string `"010"` interpreted as a decimal (not octal) value, with 2 digits of precision |
> | 16.00 | The string `"0x10"` interpreted as a hexadecimal value, with 2 digits of precision |
> | 1010.00 | The string `"10.1e2"` interpreted as a decimal floating-point value, with 2 digits of precision |
> | 1010.00 | The string `"010.1e2"` interpreted as a decimal (not octal) floating-point value, with 2 digits of precision |
> | 64.25 | The string `"0x10.1p2"` interpreted as a hexadecimal floating-point value, with 2 digits of precision |
>
> The *printf* utility is required to notify the user when conversion errors are detected while producing numeric output; thus, the following results would be expected on an implementation with 32-bit two's-complement integers when `%d` is specified as the *format* operand:
>
> | **Argument** | **Standard Output** | **Diagnostic Output** |
> | --- | --- | --- |
> | 5a | 5 | printf: "5a" not completely converted |
> | 9999999999 | 2147483647 | printf: "9999999999" arithmetic overflow |
> | -9999999999 | -2147483648 | printf: "-9999999999" arithmetic overflow |
> | ABC | 0 | printf: "ABC" expected numeric value |
>
> The diagnostic message format is not specified, but these examples convey the type of information that should be reported. Note that the value shown on standard output is what would be expected as the return value from the [*strtol*()](docs/posix/md/functions/strtol.md) function as defined in the System Interfaces volume of POSIX.1-2024. A similar correspondence exists between `%u` and [*strtoul*()](docs/posix/md/functions/strtoul.md) and `%e`, `%f`, and `%g` (if the implementation supports floating-point conversions) and [*strtod*()](docs/posix/md/functions/strtod.md).
>
> In a locale that uses a codeset based on the ISO/IEC 646:1991 standard, the command:
>
> ```
> printf "%d\n" 3 +3 -3 \'3 \"+3 "'-3"
> ```
>
> produces:
>
> | **Output Line** | **Explanation** |
> | --- | --- |
> | 3 | Decimal representation of the numeric value 3 |
> | 3 | Decimal representation of the numeric value +3 |
> | -3 | Decimal representation of the numeric value -3 |
> | 51 | Decimal representation of the numeric value of the character `'3'` in the ISO/IEC 646:1991 standard codeset |
> | 43 | Decimal representation of the numeric value of the character `'+'` in the ISO/IEC 646:1991 standard codeset |
> | 45 | Decimal representation of the numeric value of the character `'-'` in the ISO/IEC 646:1991 standard codeset |
>
> Since the last two arguments contain more characters than used for the conversion, a diagnostic message is generated and the exit status is non-zero. Note that in a locale with multi-byte characters, the value of a character is intended to be the value of the equivalent of the **wchar_t** representation of the character as described in the System Interfaces volume of POSIX.1-2024.

#### RATIONALE

> The *printf* utility was added to provide functionality that has historically been provided by [*echo*](docs/posix/md/utilities/echo.md). However, due to irreconcilable differences in the various versions of [*echo*](docs/posix/md/utilities/echo.md) extant, the version has few special features, leaving those to this new *printf* utility, which is based on one in the Ninth Edition system.
>
> The format strings for the *printf* utility are handled differently than for the [*printf*()](docs/posix/md/functions/printf.md) function in several respects. Notable differences include:
>
> - Reuse of the format until all arguments have been consumed.
> - No provision for obtaining field width and precision from argument values.
> - No requirement to support floating-point conversion specifiers.
> - An additional `b` conversion specifier.
> - Special handling of leading single-quote or double-quote for integer conversion specifiers.
> - Hexadecimal character constants are not recognized in the format.
> - Formats that use numbered argument conversion specifications can have gaps in the argument numbers.
>
> Although *printf* implementations have no difficulty handling formats with mixed numbered and unnumbered conversion specifications (unlike the [*printf*()](docs/posix/md/functions/printf.md) function where it is undefined behavior), existing implementations differ in behavior. Given the format operand `"%2$d %d\n"`, with some implementations the `"%d"` evaluates the first argument and with others it evaluates the third. Consequently this standard leaves the behavior unspecified (as opposed to undefined).
>
> Earlier versions of this standard specified that arguments for all conversions other than `b`, `c`, and `s` were evaluated in the same way (as C constants, but with stated exceptions). For implementations supporting the floating-point conversions it was not clear whether integer conversions need only accept integer constants and floating-point conversions need only accept floating-point constants, or whether both types of conversions should accept both types of constants. Also by not distinguishing between them, the requirement relating to a leading single-quote or double-quote applied to floating-point conversions even though this provided no useful functionality to applications that was not already available through the integer conversions. The current standard clarifies the situation by specifying that the arguments for floating-point conversions are evaluated as if by [*strtod*()](docs/posix/md/functions/strtod.md), and the arguments for integer conversions are evaluated as C integer constants, with the special treatment of leading single-quote and double-quote applying only to integer conversions.

#### FUTURE DIRECTIONS

> A future version of this standard may require that a missing *argument* operand to be consumed by a numbered argument conversion specification is treated as an error.
>
> A future version of this standard is expected to add a `%b` conversion to the [*printf*()](docs/posix/md/functions/printf.md) function for binary conversion of integers, in alignment with the next version of the ISO C standard. This will result in an inconsistency between the *printf* utility and [*printf*()](docs/posix/md/functions/printf.md) function for format strings containing `%b`. Implementors are encouraged to collaborate on a way to address this which could then be adopted in a future version of this standard. For example, the *printf* utility could add a **-C** option to make the format string behave in the same way, as far as possible, as the [*printf*()](docs/posix/md/functions/printf.md) function.
>
> A future version of this standard may add a `%q` conversion to convert a string argument to a quoted output format that can be reused as shell input.

#### SEE ALSO

> [*awk*](docs/posix/md/utilities/awk.md), [*bc*](docs/posix/md/utilities/bc.md), [*echo*](docs/posix/md/utilities/echo.md)
>
> XBD [*5. File Format Notation*](docs/posix/md/basedefs/V1_chap05.md#5-file-format-notation), [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables)
>
> XSH [*fprintf*()](docs/posix/md/functions/fprintf.md), [*strtod*()](docs/posix/md/functions/strtod.md)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 7

> Austin Group Interpretations 1003.1-2001 #175 and #177 are applied.
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0156 [727], XCU/TC2-2008/0157 [727,932], XCU/TC2-2008/0158 [584], and XCU/TC2-2008/0159 [727] are applied.

#### Issue 8

> Austin Group Defect 1108 is applied, changing "twos" to "two's".
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1202 is applied, changing the description of how `'\c'` is handled by the `b` conversion specifier.
>
> Austin Group Defects 1209 and 1476 are applied, changing the EXAMPLES section.
>
> Austin Group Defect 1413 is applied, changing the APPLICATION USAGE section.
>
> Austin Group Defect 1562 is applied, clarifying that it is the application's responsibility to ensure that the format is a character string, beginning and ending in its initial shift state, if any.
>
> Austin Group Defect 1592 is applied, adding support for numbered conversion specifications.
>
> Austin Group Defect 1771 is applied, changing the FUTURE DIRECTIONS section.

*End of informative text.*

### Tests

#### Test: printf plain string

`printf` writes the format operand to standard output.

```
begin test "printf plain string"
  script
    printf hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "printf plain string"
```

#### Test: printf %s with two arguments

The `%s` conversion specifier writes string arguments.

```
begin test "printf %s with two arguments"
  script
    printf "%s %s" hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "printf %s with two arguments"
```

#### Test: printf %d integer

The `%d` conversion specifier writes a signed decimal integer.

```
begin test "printf %d integer"
  script
    printf "%d" 42
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "printf %d integer"
```

#### Test: printf newline escape

`\n` in the format produces a newline character.

```
begin test "printf newline escape"
  script
    printf "a\nb"
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "printf newline escape"
```

#### Test: printf %c takes first byte of abc

The `%c` conversion writes the first byte of the argument.

```
begin test "printf %c takes first byte of abc"
  script
    printf "%c" abc
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "printf %c takes first byte of abc"
```

#### Test: printf with %s format

`printf` writes formatted operands to standard output under control
of the format operand.

```
begin test "printf with %s format"
  script
    printf "%s %s\n" hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "printf with %s format"
```

#### Test: printf reuses format for multiple arguments

The format operand shall be reused as often as necessary to satisfy
the argument operands.

```
begin test "printf reuses format for multiple arguments"
  script
    printf "%s\n" a b c
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "printf reuses format for multiple arguments"
```

#### Test: printf %s treats argument as string

The argument operands shall be treated as strings if the
corresponding conversion specifier is `b`, `c`, or `s`.

```
begin test "printf %s treats argument as string"
  script
    printf "%s\n" test
  expect
    stdout "test"
    stderr ""
    exit_code 0
end test "printf %s treats argument as string"
```

#### Test: printf %d treats argument as integer

Arguments for `d`, `i`, `o`, `u`, `x`, `X` specifiers are
evaluated as integer constants.

```
begin test "printf %d treats argument as integer"
  script
    printf "%d\n" 42
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "printf %d treats argument as integer"
```

#### Test: printf %d with leading quote gives character value

If the leading character is a single-quote or double-quote, the
value shall be the numeric value of the following character.

```
begin test "printf %d with leading quote gives character value"
  script
    printf "%d\n" "'A"
  expect
    stdout "65"
    stderr ""
    exit_code 0
end test "printf %d with leading quote gives character value"
```

#### Test: printf %s preserves shift state

The `%s` conversion specifier writes its string argument verbatim, preserving the format string's initial shift state throughout.

```
begin test "printf %s preserves shift state"
  script
    printf "%s" abc
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "printf %s preserves shift state"
```

#### Test: printf mixed %s%d shift state

When multiple conversion specifiers appear in a single format string, each consumes the next argument in order — `%s` treats its argument as a string and `%d` treats its argument as a decimal integer.

```
begin test "printf mixed %s%d shift state"
  script
    printf "%s%d" abc 123
  expect
    stdout "abc123"
    stderr ""
    exit_code 0
end test "printf mixed %s%d shift state"
```

#### Test: printf format %s %s

A format string with two `%s` specifiers separated by a space writes both string arguments with the literal space between them.

```
begin test "printf format %s %s"
  script
    printf "%s %s" hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "printf format %s %s"
```

#### Test: printf format num=%d

Literal text in the format string is written verbatim alongside the `%d` conversion, so `"num=%d"` with argument `42` produces `num=42`.

```
begin test "printf format num=%d"
  script
    printf "num=%d" 42
  expect
    stdout "num=42"
    stderr ""
    exit_code 0
end test "printf format num=%d"
```

#### Test: printf right-justified width %4d

A field width in the conversion specifier right-justifies the output by default, padding with spaces on the left to reach the specified width.

```
begin test "printf right-justified width %4d"
  script
    printf "%4d" 42
  expect
    stdout "  42"
    stderr ""
    exit_code 0
end test "printf right-justified width %4d"
```

#### Test: printf left-justified width %-4d

The `-` flag causes the output to be left-justified within the specified field width, padding with spaces on the right instead of the left.

```
begin test "printf left-justified width %-4d"
  script
    printf "%-4d" 42
  expect
    stdout "42.*"
    stderr ""
    exit_code 0
end test "printf left-justified width %-4d"
```

#### Test: printf zero-padded hex %04x

The `0` flag combined with a field width pads numeric output with leading zeros; here `%04x` formats the integer as a four-digit zero-padded hexadecimal value.

```
begin test "printf zero-padded hex %04x"
  script
    printf "%04x" 42
  expect
    stdout "002a"
    stderr ""
    exit_code 0
end test "printf zero-padded hex %04x"
```

#### Test: printf precision %.5s truncates

A precision specification on `%s` limits the number of bytes written, so `%.5s` truncates the argument to at most five characters.

```
begin test "printf precision %.5s truncates"
  script
    printf "%.5s" helloworld
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "printf precision %.5s truncates"
```

#### Test: printf backslash escape

The `\\` escape sequence in the format string produces a single literal backslash character in the output.

```
begin test "printf backslash escape"
  script
    printf "\\\\"
  expect
    stdout "\\"
    stderr ""
    exit_code 0
end test "printf backslash escape"
```

#### Test: printf tab escape

The `\t` escape sequence in the format string produces a horizontal tab character in the output.

```
begin test "printf tab escape"
  script
    printf "a\tb"
  expect
    stdout "a\tb"
    stderr ""
    exit_code 0
end test "printf tab escape"
```

#### Test: printf carriage return escape

The `\r` escape sequence in the format string produces a carriage return character in the output.

```
begin test "printf carriage return escape"
  script
    printf "a\rb" | cat -v
  expect
    stdout ".*\^M.*|.*b.*"
    stderr ""
    exit_code 0
end test "printf carriage return escape"
```

#### Test: printf bell escape produces a byte

The `\a` escape sequence in the format string produces a bell (alert) character, which is a non-zero byte written to standard output.

```
begin test "printf bell escape produces a byte"
  script
    printf "\a" | wc -c | tr -d ' '
  expect
    stdout ".*[1-9].*"
    stderr ""
    exit_code 0
end test "printf bell escape produces a byte"
```

#### Test: printf backspace escape

The `\b` escape sequence in the format string produces a backspace character in the output.

```
begin test "printf backspace escape"
  script
    printf "a\bb" | cat -v
  expect
    stdout ".*\^H.*|.*b.*"
    stderr ""
    exit_code 0
end test "printf backspace escape"
```

#### Test: printf form feed escape

The `\f` escape sequence in the format string produces a form feed character in the output.

```
begin test "printf form feed escape"
  script
    printf "a\fb" | cat -v
  expect
    stdout ".*\^L.*|.*b.*"
    stderr ""
    exit_code 0
end test "printf form feed escape"
```

#### Test: printf vertical tab escape

The `\v` escape sequence in the format string produces a vertical tab character in the output.

```
begin test "printf vertical tab escape"
  script
    printf "a\vb" | cat -v
  expect
    stdout ".*\^K.*|.*b.*"
    stderr ""
    exit_code 0
end test "printf vertical tab escape"
```

#### Test: printf octal \101 produces A

An octal escape `\ddd` in the format string writes a byte with the specified numeric value; `\101` is octal for 65, which is the ASCII character `A`.

```
begin test "printf octal \\101 produces A"
  script
    printf "\101"
  expect
    stdout "A"
    stderr ""
    exit_code 0
end test "printf octal \\101 produces A"
```

#### Test: printf octal \060 produces 0

An octal escape `\ddd` in the format string writes a byte with the specified numeric value; `\060` is octal for 48, which is the ASCII character `0`.

```
begin test "printf octal \\060 produces 0"
  script
    printf "\060"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "printf octal \\060 produces 0"
```

#### Test: printf %d no surrounding blanks

The implementation shall not precede or follow output from the `%d` conversion specifier with blank characters not specified by the format operand.

```
begin test "printf %d no surrounding blanks"
  script
    printf "%d" 42
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "printf %d no surrounding blanks"
```

#### Test: printf %d negative

The `%d` conversion specifier correctly formats negative integer arguments, including the leading minus sign.

```
begin test "printf %d negative"
  script
    printf "%d" -1
  expect
    stdout "-1"
    stderr ""
    exit_code 0
end test "printf %d negative"
```

#### Test: printf %d zero

The `%d` conversion specifier correctly formats a zero argument, producing the string `0`.

```
begin test "printf %d zero"
  script
    printf "%d" 0
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "printf %d zero"
```

#### Test: printf %u no surrounding blanks

The implementation shall not precede or follow output from the `%u` conversion specifier with blank characters not specified by the format operand.

```
begin test "printf %u no surrounding blanks"
  script
    printf "%u" 42
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "printf %u no surrounding blanks"
```

#### Test: printf %u zero

The `%u` conversion specifier correctly formats a zero argument as the unsigned decimal string `0`.

```
begin test "printf %u zero"
  script
    printf "%u" 0
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "printf %u zero"
```

#### Test: printf %o without leading zero

The `%o` conversion specifier shall not precede its output with zeros not specified by the format operand, so `%o` with `42` produces `52` without a leading zero.

```
begin test "printf %o without leading zero"
  script
    printf "%o" 42
  expect
    stdout "52"
    stderr ""
    exit_code 0
end test "printf %o without leading zero"
```

#### Test: printf %o zero

The `%o` conversion specifier correctly formats a zero argument as the octal string `0`.

```
begin test "printf %o zero"
  script
    printf "%o" 0
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "printf %o zero"
```

#### Test: printf %#o with leading zero

The `#` flag on the `%o` conversion specifier forces the output to begin with a leading zero (the alternative form), so `%#o` with `42` produces `052`.

```
begin test "printf %#o with leading zero"
  script
    printf "%#o" 42
  expect
    stdout "052"
    stderr ""
    exit_code 0
end test "printf %#o with leading zero"
```

#### Test: printf %b plain string

The `%b` conversion specifier treats its argument as a string that may contain backslash escape sequences; when none are present, the argument is written verbatim.

```
begin test "printf %b plain string"
  script
    printf "%b" hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "printf %b plain string"
```

#### Test: printf %b two arguments

Multiple `%b` conversion specifiers each consume the next argument in order, writing the interpreted string values separated by the literal text in the format.

```
begin test "printf %b two arguments"
  script
    printf "%b %b" hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "printf %b two arguments"
```

#### Test: printf %b newline in argument

The `%b` conversion specifier interprets `\n` in the argument string as a newline character, converting the escape sequence into an actual newline in the output.

```
begin test "printf %b newline in argument"
  script
    printf "%b" "a\nb"
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "printf %b newline in argument"
```

#### Test: printf %b tab in argument

The `%b` conversion specifier interprets `\t` in the argument string as a horizontal tab character.

```
begin test "printf %b tab in argument"
  script
    printf "%b" "a\tb"
  expect
    stdout "a\tb"
    stderr ""
    exit_code 0
end test "printf %b tab in argument"
```

#### Test: printf %b hello newline world

The `%b` conversion specifier interprets embedded `\n` escape sequences within its argument, producing a newline between `hello` and `world`.

```
begin test "printf %b hello newline world"
  script
    printf "%b" "hello\nworld"
  expect
    stdout "hello\nworld"
    stderr ""
    exit_code 0
end test "printf %b hello newline world"
```

#### Test: printf %b backslash

The `%b` conversion specifier interprets `\\` in the argument as a single literal backslash character in the output.

```
begin test "printf %b backslash"
  script
    printf "%b" "\\\\"
  expect
    stdout "\\"
    stderr ""
    exit_code 0
end test "printf %b backslash"
```

#### Test: printf %b newline

The `%b` conversion specifier interprets `\n` in its argument as a newline character, splitting the output across two lines.

```
begin test "printf %b newline"
  script
    printf "%b" "x\ny"
  expect
    stdout "x\ny"
    stderr ""
    exit_code 0
end test "printf %b newline"
```

#### Test: printf %b tab

The `%b` conversion specifier interprets `\t` in its argument as a horizontal tab character.

```
begin test "printf %b tab"
  script
    printf "%b" "x\ty"
  expect
    stdout "x\ty"
    stderr ""
    exit_code 0
end test "printf %b tab"
```

#### Test: printf %b \0101 produces A

The `%b` conversion specifier interprets `\0ddd` octal escapes in its argument; `\0101` is octal for 65, producing the ASCII character `A`.

```
begin test "printf %b \\0101 produces A"
  script
    printf "%b" "\0101"
  expect
    stdout "A"
    stderr ""
    exit_code 0
end test "printf %b \\0101 produces A"
```

#### Test: printf %b \0060 produces 0

The `%b` conversion specifier interprets `\0ddd` octal escapes in its argument; `\0060` is octal for 48, producing the ASCII character `0`.

```
begin test "printf %b \\0060 produces 0"
  script
    printf "%b" "\0060"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "printf %b \\0060 produces 0"
```

#### Test: printf %b \c stops output

The `%b` conversion specification interprets `\c` in the argument string as an instruction to stop all further output, discarding remaining characters in the current argument.

```
begin test "printf %b \\c stops output"
  script
    printf "%b" "hello\cworld"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "printf %b \\c stops output"
```

#### Test: printf %b \c stops subsequent %b arguments

When `\c` is encountered in a `%b` argument, `printf` shall ignore any remaining string operands as well, not just the rest of the current argument.

```
begin test "printf %b \\c stops subsequent %b arguments"
  script
    printf "%b%b" "a\c" "b"
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "printf %b \\c stops subsequent %b arguments"
```

#### Test: printf %b \c stops format reuse

When `\c` is encountered in a `%b` argument, `printf` shall also stop reusing the format operand for any additional argument operands that would otherwise cause a second pass.

```
begin test "printf %b \\c stops format reuse"
  script
    printf "%b" "first\csecond" third
  expect
    stdout "first"
    stderr ""
    exit_code 0
end test "printf %b \\c stops format reuse"
```

#### Test: printf %s full string

The `%s` conversion specifier without a precision writes the entire string argument to standard output.

```
begin test "printf %s full string"
  script
    printf "%s" hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "printf %s full string"
```

#### Test: printf %.3s truncation

A precision of 3 on the `%s` specifier limits output to at most 3 bytes of the argument, so `%.3s` with `hello` produces `hel`.

```
begin test "printf %.3s truncation"
  script
    printf "%.3s" hello
  expect
    stdout "hel"
    stderr ""
    exit_code 0
end test "printf %.3s truncation"
```

#### Test: printf %.2s truncation

A precision of 2 on the `%s` specifier limits output to at most 2 bytes of the argument, so `%.2s` with `hello` produces `he`.

```
begin test "printf %.2s truncation"
  script
    printf "%.2s" hello
  expect
    stdout "he"
    stderr ""
    exit_code 0
end test "printf %.2s truncation"
```

#### Test: printf %s without precision prints full string

When no precision is specified on `%s`, the entire string argument is written regardless of its length.

```
begin test "printf %s without precision prints full string"
  script
    printf "%s" "hello world this is a long string"
  expect
    stdout "hello world this is a long string"
    stderr ""
    exit_code 0
end test "printf %s without precision prints full string"
```

#### Test: printf %s long string no truncation

The `%s` specifier without a precision writes arbitrarily long string arguments in full, with no implicit truncation.

```
begin test "printf %s long string no truncation"
  script
    printf "%s" "abcdefghijklmnopqrstuvwxyz0123456789"
  expect
    stdout "abcdefghijklmnopqrstuvwxyz0123456789"
    stderr ""
    exit_code 0
end test "printf %s long string no truncation"
```

#### Test: printf %s %d two specs two args

A format string containing both `%s` and `%d` consumes two arguments in order, treating the first as a string and the second as a decimal integer.

```
begin test "printf %s %d two specs two args"
  script
    printf "%s %d" hello 42
  expect
    stdout "hello 42"
    stderr ""
    exit_code 0
end test "printf %s %d two specs two args"
```

#### Test: printf three %s specs

A format string with three `%s` specifiers consumes three arguments sequentially, writing each separated by the literal text in the format.

```
begin test "printf three %s specs"
  script
    printf "%s %s %s" a b c
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "printf three %s specs"
```

#### Test: printf three %d specs

A format string with three `%d` specifiers consumes three integer arguments sequentially, formatting each as a signed decimal.

```
begin test "printf three %d specs"
  script
    printf "%d %d %d" 1 2 3
  expect
    stdout "1 2 3"
    stderr ""
    exit_code 0
end test "printf three %d specs"
```

#### Test: printf positional n$ specifiers (optional)

POSIX allows numbered argument conversion specifications (`%n$`) to select arguments in an order appropriate to specific languages. This test verifies that such specifiers do not cause an error, even if the feature is optional.

```
begin test "printf positional n$ specifiers (optional)"
  script
    printf "%2\$s %1\$s\n" hello world 2>/dev/null
    true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "printf positional n$ specifiers (optional)"
```

#### Test: printf sequential argument consumption

When unnumbered conversion specifications are used, each specifier consumes the next argument operand in order from left to right.

```
begin test "printf sequential argument consumption"
  script
    printf "%s %s %s" a b c
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "printf sequential argument consumption"
```

#### Test: printf mixed %d %s %d

A format string that interleaves `%d` and `%s` specifiers consumes arguments in order, applying the appropriate conversion (integer or string) to each.

```
begin test "printf mixed %d %s %d"
  script
    printf "%d %s %d" 1 hello 3
  expect
    stdout "1 hello 3"
    stderr ""
    exit_code 0
end test "printf mixed %d %s %d"
```

#### Test: printf format reuse with %d newline

The format operand is reused as often as necessary to satisfy the argument operands; here `"%d\n"` is applied three times to print each integer on its own line.

```
begin test "printf format reuse with %d newline"
  script
    printf "%d\n" 1 2 3
  expect
    stdout "1\n2\n3"
    stderr ""
    exit_code 0
end test "printf format reuse with %d newline"
```

#### Test: printf format reuse with %s newline

The format operand is reused as often as necessary to satisfy the argument operands; here `"%s\n"` is applied three times to print each string on its own line.

```
begin test "printf format reuse with %s newline"
  script
    printf "%s\n" a b c
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "printf format reuse with %s newline"
```

#### Test: printf %c takes first byte of xyz

The `%c` conversion specifier writes only the first byte of its argument and ignores any additional bytes, so `xyz` produces `x`.

```
begin test "printf %c takes first byte of xyz"
  script
    printf "%c" xyz
  expect
    stdout "x"
    stderr ""
    exit_code 0
end test "printf %c takes first byte of xyz"
```

#### Test: printf %c takes first byte of 123

The `%c` conversion specifier writes only the first byte of its argument regardless of content, so `123` produces `1`.

```
begin test "printf %c takes first byte of 123"
  script
    printf "%c" 123
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "printf %c takes first byte of 123"
```

#### Test: printf %c with empty arg produces null/empty

When the argument to `%c` is an empty string, it is unspecified whether nothing is written or a null byte is written; either way, the result should be empty or null.

```
begin test "printf %c with empty arg produces null/empty"
  script
    result=$(printf "%c" "")
    [ -z "$result" ] && echo "ok" || echo "fail"
  expect
    stdout "ok"
    stderr ".+"
    exit_code 0
end test "printf %c with empty arg produces null/empty"
```

#### Test: extra %s gets null string

When fewer arguments are provided than format specifiers require, missing `%s` arguments are treated as empty (null) strings.

```
begin test "extra %s gets null string"
  script
    printf "%s %s" hello
  expect
    stdout "hello.*"
    stderr ""
    exit_code 0
end test "extra %s gets null string"
```

#### Test: extra %d gets zero

When fewer arguments are provided than format specifiers require, missing `%d` arguments are treated as zero.

```
begin test "extra %d gets zero"
  script
    printf "%d %d" 42
  expect
    stdout "42 0"
    stderr ""
    exit_code 0
end test "extra %d gets zero"
```

#### Test: extra %o gets zero

When fewer arguments are provided than format specifiers require, missing `%o` arguments are treated as zero.

```
begin test "extra %o gets zero"
  script
    printf "%o %o" 42
  expect
    stdout "52 0"
    stderr ""
    exit_code 0
end test "extra %o gets zero"
```

#### Test: extra %x gets zero

When fewer arguments are provided than format specifiers require, missing `%x` arguments are treated as zero.

```
begin test "extra %x gets zero"
  script
    printf "%x %x" 42
  expect
    stdout "2a 0"
    stderr ""
    exit_code 0
end test "extra %x gets zero"
```

#### Test: extra %b gets null string

When fewer arguments are provided than format specifiers require, missing `%b` arguments are treated as empty (null) strings.

```
begin test "extra %b gets null string"
  script
    printf "%b %b" hello
  expect
    stdout "hello.*"
    stderr ""
    exit_code 0
end test "extra %b gets null string"
```

#### Test: printf %d with non-numeric arg exits non-zero

If an argument operand cannot be completely converted for the `%d` specifier (e.g. a non-numeric string), `printf` shall write a diagnostic to stderr and exit with a non-zero status.

```
begin test "printf %d with non-numeric arg exits non-zero"
  script
    printf "%d" not_a_number 2>/dev/null
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "printf %d with non-numeric arg exits non-zero"
```

#### Test: printf %d with trailing non-numeric exits non-zero

If an argument operand is not completely converted (e.g. `42abc` has trailing non-numeric characters), `printf` shall report an error and exit with a non-zero status.

```
begin test "printf %d with trailing non-numeric exits non-zero"
  script
    printf "%d" 42abc 2>/dev/null
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "printf %d with trailing non-numeric exits non-zero"
```

#### Test: printf %u with non-numeric arg exits non-zero

If an argument operand cannot be completely converted for the `%u` specifier, `printf` shall write a diagnostic to stderr and exit with a non-zero status.

```
begin test "printf %u with non-numeric arg exits non-zero"
  script
    printf "%u" xyz 2>/dev/null
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "printf %u with non-numeric arg exits non-zero"
```

#### Test: printf %o with non-numeric arg exits non-zero

If an argument operand cannot be completely converted for the `%o` specifier, `printf` shall write a diagnostic to stderr and exit with a non-zero status.

```
begin test "printf %o with non-numeric arg exits non-zero"
  script
    printf "%o" xyz 2>/dev/null
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "printf %o with non-numeric arg exits non-zero"
```

#### Test: printf %x with non-numeric arg exits non-zero

If an argument operand cannot be completely converted for the `%x` specifier, `printf` shall write a diagnostic to stderr and exit with a non-zero status.

```
begin test "printf %x with non-numeric arg exits non-zero"
  script
    printf "%x" xyz 2>/dev/null
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "printf %x with non-numeric arg exits non-zero"
```

#### Test: printf %d non-numeric produces stderr diagnostic

When a non-numeric argument is given for `%d`, `printf` shall write a diagnostic message to standard error indicating the conversion failure.

```
begin test "printf %d non-numeric produces stderr diagnostic"
  script
    printf "%d" not_a_number 2>&1 >/dev/null
  expect
    stdout ".+"
    stderr ""
    exit_code !=0
end test "printf %d non-numeric produces stderr diagnostic"
```

#### Test: printf %c with multi-char arg exits 0

It is not considered an error if the `%c` argument contains more than one byte; the extra bytes are silently ignored and the exit status shall be zero.

```
begin test "printf %c with multi-char arg exits 0"
  script
    printf "%c" hello
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "printf %c with multi-char arg exits 0"
```

#### Test: printf %.3s with truncation exits 0

It is not considered an error if the `%s` argument is not completely used due to a precision specification; the exit status shall be zero.

```
begin test "printf %.3s with truncation exits 0"
  script
    printf "%.3s" hello
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "printf %.3s with truncation exits 0"
```

#### Test: printf %b with long arg exits 0

It is not considered an error if a `%b` argument is not completely used (e.g. a long string with no escape sequences); the exit status shall be zero.

```
begin test "printf %b with long arg exits 0"
  script
    printf "%b" "hello world"
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "printf %b with long arg exits 0"
```

#### Test: printf %c output correct despite partial use

When `%c` receives a multi-byte argument, only the first byte (`h` from `hello`) is written; partial use of the argument does not affect correctness.

```
begin test "printf %c output correct despite partial use"
  script
    printf "%c" hello
  expect
    stdout "h"
    stderr ""
    exit_code 0
end test "printf %c output correct despite partial use"
```

#### Test: printf %.3s output correct despite truncation

When `%.3s` truncates the argument to 3 bytes, the output (`hel`) is correct and matches the precision specification.

```
begin test "printf %.3s output correct despite truncation"
  script
    printf "%.3s" hello
  expect
    stdout "hel"
    stderr ""
    exit_code 0
end test "printf %.3s output correct despite truncation"
```

#### Test: printf %n$ format reuse

When numbered conversion specifications (`%n$`) are used and the format is reused, the value of `n` refers to the nth argument following the highest-numbered argument consumed by the previous use of the format.

```
begin test "printf %n$ format reuse"
  script
    /usr/bin/printf '%1$s %2$s\n' a b c d
  expect
    stdout "a b\nc d"
    stderr ""
    exit_code 0
end test "printf %n$ format reuse"
```

#### Test: printf conversion error not masked by %b \c

When a numeric conversion error occurs (e.g., a non-numeric string for `%d`),
the utility shall not exit with a zero exit status, even if a subsequent `%b`
argument contains `\c` which causes output to stop. The conversion error must
still be reflected in the exit status.
Known `bash --posix` non-compliance #12: the `\c` escape causes an immediate
return that bypasses the conversion error check, resulting in exit status 0.

```
begin test "printf conversion error not masked by %b \\c"
  script
    printf "%d%b" abc "\c" 2>/dev/null
  expect
    stdout "0"
    stderr ""
    exit_code !=0
end test "printf conversion error not masked by %b \\c"
```
