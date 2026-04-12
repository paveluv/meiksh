# Test Suite for Intrinsic Utility: unalias

This test suite covers the **unalias** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: unalias](#utility-unalias)

## utility: unalias

#### NAME

> unalias — remove alias definitions

#### SYNOPSIS

> ```
> unalias alias-name...
> unalias -a
> ```

#### DESCRIPTION

> The *unalias* utility shall remove the definition for each alias name specified. See [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution). The aliases shall be removed from the current shell execution environment; see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment).

#### OPTIONS

> The *unalias* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following option shall be supported:
>
> - **-a**: Remove all alias definitions from the current shell execution environment.

#### OPERANDS

> The following operand shall be supported:
>
> - *alias-name*: The name of an alias to be removed.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *unalias*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
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
> - 0: Successful completion.
> - \>0: One of the *alias-name* operands specified did not represent a valid alias definition, or an error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *unalias* affects the current shell execution environment, it is generally provided as a shell regular built-in.

#### EXAMPLES

> None.

#### RATIONALE

> The *unalias* description is based on that from historical KornShell implementations. Known differences exist between that and the C shell. The KornShell version was adopted to be consistent with all the other KornShell features in this volume of POSIX.1-2024, such as command line editing.
>
> The **-a** option is the equivalent of the *unalias* * form of the C shell and is provided to address security concerns about unknown aliases entering the environment of a user (or application) through the allowable implementation-defined predefined alias route or as a result of an *ENV* file. (Although *unalias* could be used to simplify the "secure" shell script shown in the [*command*](docs/posix/md/utilities/command.md) rationale, it does not obviate the need to quote all command names. An initial call to *unalias* **-a** would have to be quoted in case there was an alias for *unalias*.)

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language), [*alias*](docs/posix/md/utilities/alias.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.

#### Issue 7

> The *unalias* utility is moved from the User Portability Utilities option to the Base. User Portability Utilities is now an option for interactive utilities.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*

*End of informative text.*

### Tests

#### Test: unalias removes named alias

`unalias` removes a previously defined alias.

```
begin test "unalias removes named alias"
  script
    alias myalias="echo hello"
    unalias myalias
    alias myalias 2>/dev/null
    echo $?
  expect
    stdout "[1-9].*"
    stderr ""
    exit_code 0
end test "unalias removes named alias"
```

#### Test: unalias -a removes all aliases

`unalias -a` removes all alias definitions.

```
begin test "unalias -a removes all aliases"
  script
    alias a1="echo a"
    alias a2="echo b"
    unalias -a
    count=$(alias 2>/dev/null | wc -l)
    echo "$count"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "unalias -a removes all aliases"
```

#### Test: querying removed alias fails

After `unalias` removes an alias, querying that alias name shall fail
because the definition no longer exists.

```
begin test "querying removed alias fails"
  script
    alias rmme="echo gone"
    unalias rmme
    alias rmme >/dev/null 2>&1
    echo $?
  expect
    stdout "[1-9].*"
    stderr ""
    exit_code 0
end test "querying removed alias fails"
```

#### Test: unaliased command does not expand

After an alias is removed with `unalias`, using the former alias name as a command no longer triggers alias expansion and instead fails as an unknown command.

```
begin test "unaliased command does not expand"
  script
    alias rmme2="echo gone"
    unalias rmme2
    rmme2
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "unaliased command does not expand"
```

#### Test: unalias removes multiple aliases

`unalias` accepts multiple alias-name operands and removes each of them, leaving other aliases intact.

```
begin test "unalias removes multiple aliases"
  script
    alias a="echo A"
    alias b="echo B"
    alias c="echo C"
    unalias a b
    alias
  expect
    stdout ".*c=.*echo C.*"
    stderr ""
    exit_code 0
end test "unalias removes multiple aliases"
```

#### Test: unalias of unknown name exits non-zero

If an operand does not name a valid alias definition, `unalias` shall
exit with a status greater than zero.

```
begin test "unalias of unknown name exits non-zero"
  script
    unalias definitely_not_an_alias
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "unalias of unknown name exits non-zero"
```

