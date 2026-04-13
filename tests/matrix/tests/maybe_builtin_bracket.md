# Test Suite for Maybe-Builtin Utility: [

This test suite covers the **[** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: [](#utility-bracket)

## utility: [

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

#### Test: [ ] with zero arguments exits 1

0 arguments between `[` and `]`: exit false (1). The `]` is not counted
in the argument-count algorithm.

```
begin test "[ ] with zero arguments exits 1"
  script
    [ ]
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "[ ] with zero arguments exits 1"
```

#### Test: [ non-null string ] exits 0

1 argument: exit true (0) if $1 is not null.

```
begin test "[ non-null string ] exits 0"
  script
    [ hello ]
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "[ non-null string ] exits 0"
```

#### Test: [ empty string ] exits 1

1 argument: exit false (1) if $1 is null (empty string).

```
begin test "[ empty string ] exits 1"
  script
    [ "" ]
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "[ empty string ] exits 1"
```

#### Test: [ string equality ]

`[ s1 = s2 ]` — true if the strings s1 and s2 are identical.

```
begin test "[ string equality ]"
  script
    [ "abc" = "abc" ] && echo "equal"
  expect
    stdout "equal"
    stderr ""
    exit_code 0
end test "[ string equality ]"
```

#### Test: [ string inequality ]

`[ s1 != s2 ]` — true if the strings s1 and s2 are not identical.

```
begin test "[ string inequality ]"
  script
    [ "abc" != "xyz" ] && echo "diff"
  expect
    stdout "diff"
    stderr ""
    exit_code 0
end test "[ string inequality ]"
```

#### Test: [ integer -eq ]

`[ n1 -eq n2 ]` — true if the integers are algebraically equal.

```
begin test "[ integer -eq ]"
  script
    [ 42 -eq 42 ]
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "[ integer -eq ]"
```

#### Test: [ integer -eq ] false exits 1

A false integer comparison shall return exit status 1.

```
begin test "[ integer -eq ] false exits 1"
  script
    [ 1 -eq 2 ]
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "[ integer -eq ] false exits 1"
```

#### Test: [ integer -gt ]

`[ n1 -gt n2 ]` — true if n1 is algebraically greater than n2.

```
begin test "[ integer -gt ]"
  script
    [ 10 -gt 5 ] && echo "greater"
  expect
    stdout "greater"
    stderr ""
    exit_code 0
end test "[ integer -gt ]"
```

#### Test: [ -d directory ]

`[ -d pathname ]` — true if pathname resolves to an existing directory.

```
begin test "[ -d directory ]"
  script
    mkdir -p tmp_bracket_dir
    [ -d tmp_bracket_dir ] && echo "isdir"
  expect
    stdout "isdir"
    stderr ""
    exit_code 0
end test "[ -d directory ]"
```

#### Test: [ -f regular file ]

`[ -f pathname ]` — true if pathname resolves to an existing regular file.

```
begin test "[ -f regular file ]"
  script
    touch tmp_bracket_file
    [ -f tmp_bracket_file ] && echo "isfile"
  expect
    stdout "isfile"
    stderr ""
    exit_code 0
end test "[ -f regular file ]"
```

#### Test: [ -e existing file ]

`[ -e pathname ]` — true if pathname resolves to an existing directory entry.

```
begin test "[ -e existing file ]"
  script
    touch tmp_bracket_exists
    [ -e tmp_bracket_exists ] && echo "exists"
  expect
    stdout "exists"
    stderr ""
    exit_code 0
end test "[ -e existing file ]"
```

#### Test: [ -n non-empty string ]

`[ -n string ]` — true if the length of string is non-zero.

```
begin test "[ -n non-empty string ]"
  script
    [ -n "hello" ] && echo "nonempty"
  expect
    stdout "nonempty"
    stderr ""
    exit_code 0
end test "[ -n non-empty string ]"
```

#### Test: [ -z empty string ]

`[ -z string ]` — true if the length of string is zero.

```
begin test "[ -z empty string ]"
  script
    [ -z "" ] && echo "zero"
  expect
    stdout "zero"
    stderr ""
    exit_code 0
end test "[ -z empty string ]"
```

#### Test: [ ! false expression ] is true

`[ ! expression ]` — 4-argument form where $1 is `!`, negating the
three-argument test of $2, $3, and $4.

```
begin test "[ ! false expression ] is true"
  script
    [ ! "abc" = "xyz" ] && echo "negated"
  expect
    stdout "negated"
    stderr ""
    exit_code 0
end test "[ ! false expression ] is true"
```

#### Test: [ -e nonexistent ] exits 1

`[ -e pathname ]` — false (exit 1) if pathname cannot be resolved.

```
begin test "[ -e nonexistent ] exits 1"
  script
    [ -e /no/such/path_bracket_test ]
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "[ -e nonexistent ] exits 1"
```
