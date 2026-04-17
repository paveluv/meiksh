# Test Suite for Maybe-Builtin Utility: test

This test suite covers the **test** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: test](#utility-test)

## utility: test

#### NAME

> test — evaluate expression

#### SYNOPSIS

> ```
> test [expression]
> [ [expression] ]
> ```

#### DESCRIPTION

> The *test* utility shall evaluate the *expression* and indicate the result of the evaluation by its exit status. An exit status of zero indicates that the expression evaluated as true and an exit status of 1 indicates that the expression evaluated as false.
>
> In the second form of the utility, where the utility name used is *[* rather than *test*, the application shall ensure that the closing square bracket is a separate argument. The *test* and *[* utilities may be implemented as a single linked utility which examines the basename of the zeroth command line argument to determine whether to behave as the *test* or *[* variant. Applications using the *exec* family of functions to execute these utilities shall ensure that the argument passed in *arg0* or *argv*[0] is `'['` when executing the *[* utility and has a basename of `"test"` when executing the *test* utility.

#### OPTIONS

> The *test* utility shall not recognize the `"--"` argument in the manner specified by Guideline 10 in XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines). In addition, when the utility name used is *[* the utility does not conform to Guidelines 1 and 2.
>
> No options shall be supported.

#### OPERANDS

> The application shall ensure that all operators and elements of primaries are presented as separate arguments to the *test* utility.
>
> The following primaries can be used to construct *expression*:
>
> - **-b***pathname*: True if *pathname* resolves to an existing directory entry for a block special file. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a block special file.
> - **-c***pathname*: True if *pathname* resolves to an existing directory entry for a character special file. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a character special file.
> - **-d***pathname*: True if *pathname* resolves to an existing directory entry for a directory. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a directory.
> - **-e***pathname*: True if *pathname* resolves to an existing directory entry. False if *pathname* cannot be resolved.
> - *pathname1***-ef***pathname2*: True if *pathname1* and *pathname2* resolve to existing directory entries for the same file; otherwise, false.
> - **-f***pathname*: True if *pathname* resolves to an existing directory entry for a regular file. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a regular file.
> - **-g***pathname*: True if *pathname* resolves to an existing directory entry for a file that has its set-group-ID flag set. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that does not have its set-group-ID flag set.
> - **-h***pathname*: True if *pathname* resolves to an existing directory entry for a symbolic link. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a symbolic link. If the final component of *pathname* is a symbolic link, that symbolic link is not followed.
> - **-L***pathname*: True if *pathname* resolves to an existing directory entry for a symbolic link. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a symbolic link. If the final component of *pathname* is a symbolic link, that symbolic link is not followed.
> - **-n***string*: True if the length of *string* is non-zero; otherwise, false.
> - *pathname1***-nt***pathname2*: True if *pathname1* resolves to an existing file and *pathname2* cannot be resolved, or if both resolve to existing files and *pathname1* is newer than *pathname2* according to their last data modification timestamps; otherwise, false.
> - *pathname1***-ot***pathname2*: True if *pathname2* resolves to an existing file and *pathname1* cannot be resolved, or if both resolve to existing files and *pathname1* is older than *pathname2* according to their last data modification timestamps; otherwise, false.
> - **-p***pathname*: True if *pathname* resolves to an existing directory entry for a FIFO. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a FIFO.
> - **-r***pathname*: True if *pathname* resolves to an existing directory entry for a file for which permission to read from the file is granted, as defined in [*1.1.1.4 File Read, Write, and Creation*](docs/posix/md/utilities/V3_chap01.md#1114-file-read-write-and-creation). False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file for which permission to read from the file is not granted.
> - **-S***pathname*: True if *pathname* resolves to an existing directory entry for a socket. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that is not a socket.
> - **-s***pathname*: True if *pathname* resolves to an existing directory entry for a file that has a size greater than zero. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that does not have a size greater than zero.
> - **-t***file_descriptor*: True if file descriptor number *file_descriptor* is open and is associated with a terminal. False if *file_descriptor* is not a valid file descriptor number, or if file descriptor number *file_descriptor* is not open, or if it is open but is not associated with a terminal.
> - **-u***pathname*: True if *pathname* resolves to an existing directory entry for a file that has its set-user-ID flag set. False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file that does not have its set-user-ID flag set.
> - **-w***pathname*: True if *pathname* resolves to an existing directory entry for a file for which permission to write to the file is granted, as defined in [*1.1.1.4 File Read, Write, and Creation*](docs/posix/md/utilities/V3_chap01.md#1114-file-read-write-and-creation). False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file for which permission to write to the file is not granted.
> - **-x***pathname*: True if *pathname* resolves to an existing directory entry for a file for which permission to execute the file (or search it, if it is a directory) is granted, as defined in [*1.1.1.4 File Read, Write, and Creation*](docs/posix/md/utilities/V3_chap01.md#1114-file-read-write-and-creation). False if *pathname* cannot be resolved, or if *pathname* resolves to an existing directory entry for a file for which permission to execute (or search) the file is not granted.
> - **-z***string*: True if the length of string *string* is zero; otherwise, false.
> - *string*: True if the string *string* is not the null string; otherwise, false.
> - *s1***=***s2*: True if the strings *s1* and *s2* are identical; otherwise, false.
> - *s1***!=***s2*: True if the strings *s1* and *s2* are not identical; otherwise, false.
> - *s1***\>***s2*: True if *s1* collates after *s2* in the current locale; otherwise, false.
> - *s1***\<***s2*: True if *s1* collates before *s2* in the current locale; otherwise, false.
> - *n1***-eq***n2*: True if the integers *n1* and *n2* are algebraically equal; otherwise, false.
> - *n1***-ne***n2*: True if the integers *n1* and *n2* are not algebraically equal; otherwise, false.
> - *n1***-gt***n2*: True if the integer *n1* is algebraically greater than the integer *n2*; otherwise, false.
> - *n1***-ge***n2*: True if the integer *n1* is algebraically greater than or equal to the integer *n2*; otherwise, false.
> - *n1***-lt***n2*: True if the integer *n1* is algebraically less than the integer *n2*; otherwise, false.
> - *n1***-le***n2*: True if the integer *n1* is algebraically less than or equal to the integer *n2*; otherwise, false.
>
> With the exception of the **-h** *pathname* and **-L** *pathname* primaries, if a *pathname*, *pathname1*, or *pathname2* argument is a symbolic link, *test* shall evaluate the expression by resolving the symbolic link and using the file referenced by the link.
>
> These primaries can be combined with the following operator:
>
> - **!***expression*: True if *expression* is false. False if *expression* is true.
>
> The primaries with two elements of the form:
>
> ```
> -primary_operator primary_operand
> ```
>
> are known as *unary primaries*. The primaries with three elements in either of the two forms:
>
> ```
> primary_operand -primary_operator primary_operand
>
>
> primary_operand primary_operator primary_operand
> ```
>
> are known as *binary primaries*. Additional implementation-defined operators and *primary_operator*s may be provided by implementations. They shall be of the form -*operator* where the first character of *operator* is not a digit.
>
> The algorithm for determining the precedence of the operators and the return value that shall be generated is based on the number of arguments presented to *test*. (However, when using the `"[...]"` form, the `<right-square-bracket>` final argument shall not be counted in this algorithm.)
>
> In the following list, $1, $2, $3, and $4 represent the arguments presented to *test*:
>
> - 0 arguments: Exit false (1).
> - 1 argument: Exit true (0) if $1 is not null; otherwise, exit false.
> - 2 arguments:
>
>     - If $1 is `'!'`, exit true if $2 is null, false if $2 is not null.
>     - If $1 is a unary primary, exit true if the unary test is true, false if the unary test is false.
>     - Otherwise, produce unspecified results.
> - 3 arguments:
>
>     - If $2 is a binary primary, perform the binary test of $1 and $3.
>     - If $1 is `'!'`, negate the two-argument test of $2 and $3.
>     - Otherwise, produce unspecified results.
> - 4 arguments:
>
>     - If $1 is `'!'`, negate the three-argument test of $2, $3, and $4.
>     - Otherwise, the results are unspecified.
> - \>4 arguments: The results are unspecified.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *test*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_COLLATE*: Determine the locale for the behavior of the **\>** and **\<** string comparison operators.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> Not used.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: *expression* evaluated to true.
> - 1: *expression* evaluated to false or *expression* was missing.
> - \>1: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> Since `'>'` and `'<'` are operators in the shell language, applications need to quote them when passing them as arguments to *test* from a shell.
>
> The **-a** and **-o** binary primaries and the `'('` and `')'` operators have been removed. (Many expressions using them were ambiguously defined by the grammar depending on the specific expressions being evaluated.) Scripts using these expressions should be converted to the forms given below. Even though many implementations will continue to support these forms, scripts should be extremely careful when dealing with user-supplied input that could be confused with these and other primaries and operators. Unless the application developer knows all the cases that produce input to the script, invocations like:
>
> ```
> test "$1" -a "$2"
> ```
>
> should be written as:
>
> ```
> test "$1" && test "$2"
> ```
>
> to avoid problems if a user supplied values such as $1 set to `'!'` and $2 set to the null string. That is, replace:
>
> ```
> test expr1 -a expr2
> ```
>
> with:
>
> ```
> test expr1 && test expr2
> ```
>
> and replace:
>
> ```
> test expr1 -o expr2
> ```
>
> with:
>
> ```
> test expr1 || test expr2
> ```
>
> but note that, in *test*, **-a** was specified as having higher precedence than **-o** while `"&&"` and `"||"` have equal precedence in the shell.
>
> Parentheses or braces can be used in the shell command language to effect grouping.
>
> The two commands:
>
> ```
> test "$1"
> test ! "$1"
> ```
>
> could not be used reliably on some historical systems. Unexpected results would occur if such a *string* expression were used and $1 expanded to `'!'`, `'('`, or a known unary primary. Better constructs are:
>
> ```
> test -n "$1"
> test -z "$1"
> ```
>
> respectively.
>
> Historical systems have also been unreliable given the common construct:
>
> ```
> test "$response" = "expected string"
> ```
>
> One of the following is a more reliable form:
>
> ```
> test "X$response" = "Xexpected string"
> test "expected string" = "$response"
> ```
>
> Note that the second form assumes that *expected string* could not be confused with any unary primary. If *expected string* starts with `'-'`, `'('`, `'!'`, or even `'='`, the first form should be used instead. Using the preceding rules, any of the three comparison forms is reliable, given any input. (However, note that the strings are quoted in all cases.)
>
> Historically, the string comparison binary primaries, `'='` and `"!="`, had a higher precedence than any unary primary in the greater than 4 argument case, and consequently unexpected results could occur if arguments were not properly prepared. For example, in:
>
> ```
> test -d "$1" -o -d "$2"
> ```
>
> If $1 evaluates to a possible directory name of `'='`, the first three arguments are considered a string comparison, which causes a syntax error when the second **-d** is encountered. The following form prevents this:
>
> ```
> test -d "$1" || test -d "$2"
> ```
>
> Also in the greater than 4 argument case:
>
> ```
> test "$1" = "bat" -a "$2" = "ball"
> ```
>
> syntax errors would occur if $1 evaluates to `'('` or `'!'`. One of the following forms prevents this; the second is preferred:
>
> ```
> test "$1" = "bat" && test "$2" = "ball"
> test "X$1" = "Xbat" && test "X$2" = "Xball"
> ```
>
> Note that none of the following examples are permitted by the syntax described:
>
> ```
> [-f file]
> [-f file ]
> [ -f file]
> [ -f file
> test -f file ]
> ```
>
> In the first two cases, if a utility named *[-f* exists, that utility would be invoked, and not *test*. In the remaining cases, the brackets are mismatched, and the behavior is unspecified. However:
>
> ```
> test ! ]
> ```
>
> does have a defined meaning, and must exit with status 1. Similarly:
>
> ```
> test ]
> ```
>
> must exit with status 0.

#### EXAMPLES

> ```
> fi
> case "$1" in
> pear|grape|apple)
> ```

#### RATIONALE

> The KornShell-derived conditional command (double bracket **[[]]**) was removed from the shell command language description in an early proposal. Objections were raised that the real problem is misuse of the *test* command (**[**), and putting it into the shell is the wrong way to fix the problem. Instead, proper documentation and a new shell reserved word (**!**) are sufficient. A later proposal to add **[[]]** in Issue 8 was also rejected because existing implementations of it were found to be error-prone in a similar way to historical versions of *test*, and there was also too much variation in behavior between shells that support it.
>
> Tests that require multiple *test* operations can be done at the shell level using individual invocations of the *test* command and shell logicals, rather than using the error-prone historical **-a** and **-o** operators of *test*.
>
> The BSD and System V versions of **-f** were not the same. The BSD definition was:
>
> - **-f***file*: True if *file* exists and is not a directory.
>
> The SVID version (true if the file exists and is a regular file) was chosen for this volume of POSIX.1-2024 because its use is consistent with the **-b**, **-c**, **-d**, and **-p** operands (*file* exists and is a specific file type).
>
> The **-e** primary, possessing similar functionality to that provided by the C shell, was added because it provides the only way for a shell script to find out if a file exists without trying to open the file. Since implementations are allowed to add additional file types, a portable script cannot use:
>
> ```
> test -b foo || test -c foo || test -d foo || test -f foo || test -p foo
> ```
>
> to find out if **foo** is an existing file. On historical BSD systems, the existence of a file could be determined by:
>
> ```
> test -f foo || test -d foo
> ```
>
> but there was no easy way to determine that an existing file was a regular file. An early proposal used the KornShell **-a** primary (with the same meaning), but this was changed to **-e** because there were concerns about the high probability of humans confusing the **-a** primary with the historical **-a** binary operator.
>
> The following options were not included in this volume of POSIX.1-2024, although they are provided by some implementations. These operands should not be used by new implementations for other purposes:
>
> - **-k***file*: True if *file* exists and its sticky bit is set.
> - **-C***file*: True if *file* is a contiguous file.
> - **-V***file*: True if *file* is a version file.
>
> The following option was not included because it was undocumented in most implementations, has been removed from some implementations (including System V), and the functionality is provided by the shell (see [*2.6.2 Parameter Expansion*](docs/posix/md/utilities/V3_chap02.md#262-parameter-expansion).
>
> - **-l***string*: The length of the string *string*.
>
> The **-b**, **-c**, **-g**, **-p**, **-u**, and **-x** operands are derived from the SVID; historical BSD does not provide them. The **-k** operand is derived from System V; historical BSD does not provide it.
>
> On historical BSD systems, *test* **-w** *directory* always returned false because *test* tried to open the directory for writing, which always fails.
>
> Some additional primaries newly invented or from the KornShell appeared in an early proposal as part of the conditional command (**[[]]**): *s1* **\>** *s2*, *s1* **\<** *s2*, *f1* **-nt** *f2*, *f1* **-ot** *f2*, and *f1* **-ef** *f2*. They were not carried forward into the *test* utility when the conditional command was removed from the shell because they had not been included in the *test* utility built into historical implementations of the [*sh*](docs/posix/md/utilities/sh.md) utility. However, they were later added to this standard once support for them became widespread.
>
> The **-t** *file_descriptor* primary is shown with a mandatory argument because the grammar is ambiguous if it can be omitted. Historical implementations have allowed it to be omitted, providing a default of 1.
>
> It is noted that `'['` is not part of the portable filename character set; however, since it is required to be encoded by a single byte, and is part of the portable character set, the name of this utility forms a character string across all supported locales.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*1.1.1.4 File Read, Write, and Creation*](docs/posix/md/utilities/V3_chap01.md#1114-file-read-write-and-creation), [*find*](docs/posix/md/utilities/find.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 5

> The FUTURE DIRECTIONS section is added.

#### Issue 6

> The **-h** operand is added for symbolic links, and access permission requirements are clarified for the **-r**, **-w**, and **-x** operands to align with the IEEE P1003.2b draft standard.
>
> The normative text is reworded to avoid use of the term "must" for application requirements.
>
> The **-L** and **-S** operands are added for symbolic links and sockets.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/38 is applied, adding XSI margin marking and shading to a line in the OPERANDS section referring to the use of parentheses as arguments to the *test* utility.
>
> IEEE Std 1003.1-2001/Cor 2-2004, item XCU/TC2/D6/30 is applied, rewording the existence primaries for the *test* utility.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #107 is applied.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0143 [291] is applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0191 [898], XCU/TC2-2008/0192 [730], and XCU/TC2-2008/0193 [898] are applied.

#### Issue 8

> Austin Group Defect 375 is applied, adding the *pathname1***-ef***pathname2*, *pathname1***-nt***pathname2*, *pathname1***-ot***pathname2*, *s1***\>***s2*, and *s1***\<***s2* primaries.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1330 is applied, removing the obsolescent (and optional) **-a** and **-o** binary primaries, and `'('` and `')'` operators.
>
> Austin Group Defect 1348 is applied, removing "()" from "the *exec*() family of functions".
>
> Austin Group Defect 1373 is applied, clarifying that when the utility name used is *[* the utility does not conform to Guidelines 1 and 2.

*End of informative text.*

### Tests

#### Test: test true expression exits 0

The test utility evaluates an expression and indicates the result by its exit status. An exit status of zero indicates the expression evaluated as true.

```
begin test "test true expression exits 0"
  script
    test 1 -eq 1
    echo "$?"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "test true expression exits 0"
```

#### Test: test false expression exits 1

An exit status of 1 indicates the expression evaluated as false.

```
begin test "test false expression exits 1"
  script
    test 1 -eq 2
    echo "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "test false expression exits 1"
```

#### Test: test does not consume -- as end-of-options

The test utility does not recognize `--` as end-of-options per Guideline 10. Here `--` is treated as a regular string operand in a binary `=` comparison.

```
begin test "test does not consume -- as end-of-options"
  script
    test -- = --
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test does not consume -- as end-of-options"
```

#### Test: test -n non-empty string

`test -n string` is true if the length of string is non-zero.

```
begin test "test -n non-empty string"
  script
    test -n "hello"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -n non-empty string"
```

#### Test: test -n empty string returns false

`test -n string` is false if the length of string is zero.

```
begin test "test -n empty string returns false"
  script
    test -n ""
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -n empty string returns false"
```

#### Test: test -z empty string

`test -z string` is true if the length of string is zero.

```
begin test "test -z empty string"
  script
    test -z ""
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -z empty string"
```

#### Test: test -z non-empty string returns false

`test -z string` is false if the length of string is non-zero.

```
begin test "test -z non-empty string returns false"
  script
    test -z "hello"
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -z non-empty string returns false"
```

#### Test: test string equality

`test s1 = s2` is true if the strings s1 and s2 are identical.

```
begin test "test string equality"
  script
    test "abc" = "abc"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test string equality"
```

#### Test: test string inequality

`test s1 != s2` is true if the strings s1 and s2 are not identical.

```
begin test "test string inequality"
  script
    test "abc" != "xyz"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test string inequality"
```

#### Test: test string s1 greater than s2

`test s1 \> s2` is true if s1 collates after s2 in the current locale.

```
begin test "test string s1 greater than s2"
  script
    test abc \> aaa
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test string s1 greater than s2"
```

#### Test: test string s1 less than s2

`test s1 \< s2` is true if s1 collates before s2 in the current locale.

```
begin test "test string s1 less than s2"
  script
    test aaa \< abc
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test string s1 less than s2"
```

#### Test: test integer -eq

`test n1 -eq n2` is true if the integers n1 and n2 are algebraically equal.

```
begin test "test integer -eq"
  script
    test 42 -eq 42
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -eq"
```

#### Test: test integer -ne

`test n1 -ne n2` is true if the integers n1 and n2 are not algebraically equal.

```
begin test "test integer -ne"
  script
    test 1 -ne 2
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -ne"
```

#### Test: test integer -gt true

`test n1 -gt n2` is true if n1 is algebraically greater than n2.

```
begin test "test integer -gt true"
  script
    test 5 -gt 3
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -gt true"
```

#### Test: test integer -gt false

`test n1 -gt n2` is false when n1 is not greater than n2.

```
begin test "test integer -gt false"
  script
    test 3 -gt 5
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test integer -gt false"
```

#### Test: test integer -ge

`test n1 -ge n2` is true if n1 is algebraically greater than or equal to n2.

```
begin test "test integer -ge"
  script
    test 5 -ge 5 && test 6 -ge 5
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -ge"
```

#### Test: test integer -lt

`test n1 -lt n2` is true if n1 is algebraically less than n2.

```
begin test "test integer -lt"
  script
    test 3 -lt 5
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -lt"
```

#### Test: test integer -le

`test n1 -le n2` is true if n1 is algebraically less than or equal to n2.

```
begin test "test integer -le"
  script
    test 5 -le 5 && test 4 -le 5
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test integer -le"
```

#### Test: test -d on directory

`test -d pathname` is true if pathname resolves to an existing directory.

```
begin test "test -d on directory"
  script
    test -d /tmp
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -d on directory"
```

#### Test: test -d on nonexistent

`test -d` is false when the pathname does not resolve to an existing directory.

```
begin test "test -d on nonexistent"
  script
    test -d /nonexistent_dir_99999
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -d on nonexistent"
```

#### Test: test -f regular file

`test -f pathname` is true if pathname resolves to an existing regular file.

```
begin test "test -f regular file"
  script
    tmpf=$(mktemp)
    test -f "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -f regular file"
```

#### Test: test -e existing file

`test -e pathname` is true if pathname resolves to an existing directory entry.

```
begin test "test -e existing file"
  script
    tmpf=$(mktemp)
    test -e "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -e existing file"
```

#### Test: test -e nonexistent

`test -e pathname` is false if pathname cannot be resolved.

```
begin test "test -e nonexistent"
  script
    test -e /no_such_file_99999
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -e nonexistent"
```

#### Test: test -r readable file

`test -r pathname` is true if the file exists and read permission is granted.

```
begin test "test -r readable file"
  script
    tmpf=$(mktemp)
    test -r "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -r readable file"
```

#### Test: test -w writable file

`test -w pathname` is true if the file exists and write permission is granted.

```
begin test "test -w writable file"
  script
    tmpf=$(mktemp)
    test -w "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -w writable file"
```

#### Test: test -x executable file

`test -x pathname` is true if the file exists and execute permission is granted.

```
begin test "test -x executable file"
  script
    tmpf=$(mktemp)
    chmod +x "$tmpf"
    test -x "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -x executable file"
```

#### Test: test -s file with content

`test -s pathname` is true if the file exists and has a size greater than zero.

```
begin test "test -s file with content"
  script
    tmpf=$(mktemp)
    echo "data" > "$tmpf"
    test -s "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -s file with content"
```

#### Test: test -s empty file returns false

`test -s pathname` is false if the file has size zero.

```
begin test "test -s empty file returns false"
  script
    tmpf=$(mktemp)
    test -s "$tmpf"
    r=$?
    rm -f "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -s empty file returns false"
```

#### Test: test -h detects symlink

`test -h pathname` is true if pathname is a symbolic link, without following it.

```
begin test "test -h detects symlink"
  script
    tmpf=$(mktemp)
    lnk="${tmpf}_link"
    ln -s "$tmpf" "$lnk"
    test -h "$lnk"
    r=$?
    rm -f "$lnk" "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -h detects symlink"
```

#### Test: test -L detects symlink

`test -L pathname` is true if pathname is a symbolic link, without following it. This is equivalent to `-h`.

```
begin test "test -L detects symlink"
  script
    tmpf=$(mktemp)
    lnk="${tmpf}_Llink"
    ln -s "$tmpf" "$lnk"
    test -L "$lnk"
    r=$?
    rm -f "$lnk" "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -L detects symlink"
```

#### Test: test -ef same file

`test pathname1 -ef pathname2` is true if both pathnames resolve to the same file (e.g. via a hard link).

```
begin test "test -ef same file"
  script
    tmpf=$(mktemp)
    hlnk="${tmpf}_hard"
    ln "$tmpf" "$hlnk"
    test "$tmpf" -ef "$hlnk"
    r=$?
    rm -f "$tmpf" "$hlnk"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -ef same file"
```

#### Test: test -nt newer file

`test pathname1 -nt pathname2` is true if pathname1 is newer than pathname2 by last data modification timestamp.

```
begin test "test -nt newer file"
  script
    old=$(mktemp)
    sleep 1
    new=$(mktemp)
    test "$new" -nt "$old"
    r=$?
    rm -f "$old" "$new"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -nt newer file"
```

#### Test: test -ot older file

`test pathname1 -ot pathname2` is true if pathname1 is older than pathname2 by last data modification timestamp.

```
begin test "test -ot older file"
  script
    old=$(mktemp)
    sleep 1
    new=$(mktemp)
    test "$old" -ot "$new"
    r=$?
    rm -f "$old" "$new"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -ot older file"
```

#### Test: test -t on non-terminal fd

`test -t fd` is false when the file descriptor is not associated with a terminal. Redirecting stdin from /dev/null ensures fd 0 is not a terminal.

```
begin test "test -t on non-terminal fd"
  script
    test -t 0 < /dev/null
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test -t on non-terminal fd"
```

#### Test: test negation with !

The `!` operator negates the expression: true if the expression is false. With 3 arguments where $1 is `!`, the two-argument test of $2 and $3 is negated.

```
begin test "test negation with !"
  script
    test ! -d /no_such_dir_99999
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test negation with !"
```

#### Test: test ! null string is true

With 2 arguments where $1 is `!`, test exits true if $2 is null.

```
begin test "test ! null string is true"
  script
    test ! ""
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test ! null string is true"
```

#### Test: test ! negates three-argument test

With 4 arguments where $1 is `!`, test negates the three-argument test of $2, $3, and $4.

```
begin test "test ! negates three-argument test"
  script
    test ! 1 -eq 2
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test ! negates three-argument test"
```

#### Test: test zero arguments returns 1

With zero arguments, the test utility exits false (exit status 1).

```
begin test "test zero arguments returns 1"
  script
    test
    echo "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "test zero arguments returns 1"
```

#### Test: test one non-null argument returns 0

With one argument, test exits true (0) if $1 is not null.

```
begin test "test one non-null argument returns 0"
  script
    test something
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test one non-null argument returns 0"
```

#### Test: test one null argument returns 1

With one argument, test exits false if $1 is null (empty string).

```
begin test "test one null argument returns 1"
  script
    test ""
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "test one null argument returns 1"
```

#### Test: test single non-null operator-like argument exits 0

A single non-null argument is true even if it looks like an operator. With one argument, the algorithm treats it as a string, not as a unary primary.

```
begin test "test single non-null operator-like argument exits 0"
  script
    test -f
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "test single non-null operator-like argument exits 0"
```

#### Test: test -f resolves symlink to regular file

With the exception of `-h` and `-L`, if a pathname argument is a symbolic link, test evaluates by resolving the symlink. A symlink to a regular file passes `-f`.

```
begin test "test -f resolves symlink to regular file"
  script
    tmpf=$(mktemp)
    lnk="${tmpf}_flink"
    ln -s "$tmpf" "$lnk"
    test -f "$lnk"
    r=$?
    rm -f "$lnk" "$tmpf"
    exit "$r"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "test -f resolves symlink to regular file"
```

#### Test: test string comparison uses C locale byte order

In the C locale, string comparison uses byte ordering. Uppercase `A` (0x41)
sorts before lowercase `a` (0x61).

```
begin test "test string comparison uses C locale byte order"
  setenv "LC_ALL" "C"
  script
    test A \< a && echo yes || echo no
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "test string comparison uses C locale byte order"
```

#### Test: test string comparison uses UTF-8 locale collation

In C.UTF-8, `LC_COLLATE` determines the collation order. For ASCII
characters in C.UTF-8, the order matches the C locale byte order.

```
begin test "test string comparison uses UTF-8 locale collation"
  setenv "LC_ALL" "C.UTF-8"
  script
    test A \< a && echo yes || echo no
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "test string comparison uses UTF-8 locale collation"
```
