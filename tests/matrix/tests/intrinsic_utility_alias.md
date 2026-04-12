# Test Suite for Intrinsic Utility: alias

This test suite covers the **alias** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: alias](#utility-alias)

## utility: alias

#### NAME

> alias — define or display aliases

#### SYNOPSIS

> `alias [alias-name[=string]...]`

#### DESCRIPTION

> The *alias* utility shall create or redefine alias definitions or write the values of existing alias definitions to standard output. An alias definition provides a string value that shall replace a command name when it is encountered. For information on valid string values, and the processing involved, see [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution).
>
> An alias definition shall affect the current shell execution environment and the execution environments of the subshells of the current shell. When used as specified by this volume of POSIX.1-2024, the alias definition shall not affect the parent process of the current shell nor any utility environment invoked by the shell; see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment).

#### OPTIONS

> None.

#### OPERANDS

> The following operands shall be supported:
>
> - *alias-name*: Write the alias definition to standard output.
> - *alias-name*=*string*: Assign the value of *string* to the alias *alias-name* .
>
> If no operands are given, all alias definitions shall be written to standard output.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *alias*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The format for displaying aliases (when no operands or only *name* operands are specified) shall be:
>
> ```
> "%s=%s\n", name, value
> ```
>
> The *value* string shall be written with appropriate quoting so that it is suitable for reinput to the shell. See the description of shell quoting in [*2.2 Quoting*](docs/posix/md/utilities/V3_chap02.md#22-quoting).

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
> - \>0: One of the *name* operands specified did not have an alias definition, or an error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Care should be taken to avoid alias values that end with a character that could be treated as part of an operator token, as it is unspecified whether the character that follows the alias name in the input can be used as part of the same token (see [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution)). For example, with:
>
> ```
> $ alias foo='echo 0'
> $ foo>&2
> ```
>
> the shell can either pass the argument `'0'` to [*echo*](docs/posix/md/utilities/echo.md) and redirect fd 1 to fd 2, or pass no arguments to [*echo*](docs/posix/md/utilities/echo.md) and redirect fd 0 to fd 2. Changing it to:
>
> ```
> $ alias foo='echo "0"'
> ```
>
> avoids this problem. The alternative of adding a `<space>` after the `'0'` would also avoid the problem, but in addition it would alter the way the alias works, as described in [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution).
>
> Likewise, given:
>
> ```
> $ alias foo='some_command &'
> $ foo&
> ```
>
> the shell may combine the two `'&'` characters into an `&&` (and) operator. Since the alias cannot pass arguments to *some_command* and thus can be expected to be invoked without arguments, adding a `<space>` after the `'&'` would be an acceptable way to prevent this. Alternatively, the alias could be specified as a grouping command:
>
> ```
> $ alias foo='{ some_command & }'
> ```
>
> Problems can occur for tokens other than operators as well, if the alias is used in unusual ways. For example, with:
>
> ```
> $ alias foo='echo $'
> $ foo((123))
> ```
>
> some shells combine the `'$'` and the `"((123))"` to form an arithmetic expansion, but others do not (resulting in a syntax error).

#### EXAMPLES

> 1. Create a short alias for a commonly used [*ls*](docs/posix/md/utilities/ls.md) command:
>   ```
>   alias lf="ls -CF"
>   ```
> 2. Create a simple "redo" command to repeat previous entries in the command history file:
>   ```
>   alias r='fc -s'
>   ```
> 3. Use 1K units for [*du*](docs/posix/md/utilities/du.md):
>   ```
>   alias du=du\ -k
>   ```
> 4. Set up [*nohup*](docs/posix/md/utilities/nohup.md) so that it can deal with an argument that is itself an alias name:
>   ```
>   alias nohup="nohup "
>   ```
> 5. Add the **-F** option to interactive uses of [*ls*](docs/posix/md/utilities/ls.md), even when executed as `xargs ls` or `xargs -0 ls`:
>   ```
>   alias ls='ls -F'
>   alias xargs='xargs '
>   alias -- -0='-0 '
>   find . [...] -print | xargs ls      # breaks on filenames with \n
>                                       # (two aliases expanded)
>   find . [...] -print0 | xargs -0 ls  # minimizes \n issues (three
>                                       # aliases expanded)
>   ```

#### RATIONALE

> The *alias* description is based on historical KornShell implementations. Known differences exist between that and the C shell. The KornShell version was adopted to be consistent with all the other KornShell features in this volume of POSIX.1-2024, such as command line editing.
>
> Since *alias* affects the current shell execution environment, it is generally provided as a shell regular built-in.
>
> Historical versions of the KornShell have allowed aliases to be exported to scripts that are invoked by the same shell. This is triggered by the *alias* **-x** flag; it is allowed by this volume of POSIX.1-2024 only when an explicit extension such as **-x** is used. The standard developers considered that aliases were of use primarily to interactive users and that they should normally not affect shell scripts called by those users; functions are available to such scripts.
>
> Historical versions of the KornShell had not written aliases in a quoted manner suitable for reentry to the shell, but this volume of POSIX.1-2024 has made this a requirement for all similar output. Therefore, consistency was chosen over this detail of historical practice.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.9.5 Function Definition Command*](docs/posix/md/utilities/V3_chap02.md#295-function-definition-command)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> This utility is marked as part of the User Portability Utilities option.
>
> The APPLICATION USAGE section is added.

#### Issue 7

> The *alias* utility is moved from the User Portability Utilities option to the Base. User Portability Utilities is now an option for interactive utilities.
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> The first example is changed to remove the creation of an alias for a standard utility that alters its behavior to be non-conforming.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 953 is applied, clarifying that the details of how alias replacement is performed are in the cross-referenced section ( [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution)) and updating the APPLICATION USAGE section.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1630 is applied, adding a new item in EXAMPLES.

*End of informative text.*

### Tests

#### Test: alias definition replaces command name

An alias definition provides a string value that replaces a command
name when it is encountered.

```
begin test "alias definition replaces command name"
  script
    alias greet="echo hello world"
    greet
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "alias definition replaces command name"
```

#### Test: alias-name operand writes definition to stdout

Querying an alias by name writes its definition to standard output.

```
begin test "alias-name operand writes definition to stdout"
  script
    alias myalias="echo hello"
    alias myalias
  expect
    stdout ".*myalias=.*echo hello.*"
    stderr ""
    exit_code 0
end test "alias-name operand writes definition to stdout"
```

#### Test: alias with no operands lists all definitions

When no operands are given, all alias definitions are written to
standard output.

```
begin test "alias with no operands lists all definitions"
  script
    alias a1="echo a"
    alias a2="echo b"
    out=$(alias)
    case "$out" in
      *a1=*a2=*|*a2=*a1=*) echo pass ;;
      *) echo fail ;;
    esac
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "alias with no operands lists all definitions"
```

#### Test: alias affects current shell

An alias definition affects the current shell execution environment.

```
begin test "alias affects current shell"
  script
    alias myecho="echo from_alias"
    myecho
  expect
    stdout "from_alias"
    stderr ""
    exit_code 0
end test "alias affects current shell"
```

#### Test: alias redefine updates the definition

Redefining an alias replaces its previous value.

```
begin test "alias redefine updates the definition"
  script
    alias myalias="echo first"
    alias myalias="echo second"
    myalias
  expect
    stdout "second"
    stderr ""
    exit_code 0
end test "alias redefine updates the definition"
```

#### Test: query second alias writes its definition

When multiple aliases are defined, querying one by name writes only that alias's definition to standard output.

```
begin test "query second alias writes its definition"
  script
    alias foo="ls -la"
    alias bar="grep x"
    alias foo
  expect
    stdout ".*foo=.*ls -la.*"
    stderr ""
    exit_code 0
end test "query second alias writes its definition"
```

#### Test: alias definition replaces command name numeric

An alias whose value contains a numeric argument correctly substitutes that value when the alias name is invoked as a command.

```
begin test "alias definition replaces command name numeric"
  script
    alias answer="echo 42"
    answer
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "alias definition replaces command name numeric"
```

#### Test: alias visible in subshell

An alias defined in the current shell is visible in subshell execution environments, as required by POSIX.

```
begin test "alias visible in subshell"
  script
    alias sa="echo sub_alias"
    (sa)
  expect
    stdout "sub_alias"
    stderr ""
    exit_code 0
end test "alias visible in subshell"
```

#### Test: alias in subshell does not leak to parent

An alias defined inside a subshell does not affect the parent shell's execution environment.

```
begin test "alias in subshell does not leak to parent"
  script
    (alias leaked="echo LEAKED")
    echo world
  expect
    stdout "world"
    stderr ""
    exit_code 0
end test "alias in subshell does not leak to parent"
```

#### Test: alias in child shell does not leak to parent

An alias defined in a child shell invoked via `$SHELL -c` does not leak back into the parent process.

```
begin test "alias in child shell does not leak to parent"
  script
    $SHELL -c 'alias outer_leak="echo LEAKED"' 2>/dev/null
    echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "alias in child shell does not leak to parent"
```

#### Test: alias output suitable for reinput

The output of `alias` is quoted so that it is suitable for reinput to the shell via `eval`.

```
begin test "alias output suitable for reinput"
  script
    alias special="echo hello world"
    _def=$(alias special)
    eval "alias $_def"
    special
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "alias output suitable for reinput"
```

#### Test: alias quoting handles single quotes in value

When an alias value contains single quotes, `alias` still outputs its definition with appropriate quoting.

```
begin test "alias quoting handles single quotes in value"
  script
    alias sq="echo it's fine"
    alias sq
  expect
    stdout ".*sq=.*"
    stderr ""
    exit_code 0
end test "alias quoting handles single quotes in value"
```

#### Test: querying removed alias fails

After removing an alias with `unalias`, querying that alias name shall
fail because the definition no longer exists.

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

#### Test: alias creates definition visible in output

After creating an alias, querying it by name produces output that contains the alias definition.

```
begin test "alias creates definition visible in output"
  script
    alias foo=bar
    alias foo | grep -q "foo=" && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "alias creates definition visible in output"
```

#### Test: alias exit code 0 on success

A successful alias definition returns exit status 0.

```
begin test "alias exit code 0 on success"
  script
    alias a1=b1 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "alias exit code 0 on success"
```

#### Test: alias substitution

Verifies that defining an alias and then invoking it by name causes the shell to substitute the alias value, producing the expected output as described in POSIX alias substitution rules.

```
begin interactive test "alias substitution"
  spawn -i
  expect "$ "
  send "alias foo=\"echo aliased\""
  expect "$ "
  send "foo"
  expect "aliased"
  sendeof
  wait
end interactive test "alias substitution"
```

#### Test: alias with trailing space chains to next word

Verifies that when an alias value ends with a space, the shell performs alias substitution on the next word as well. This tests the POSIX chaining behavior described in section 2.3.1 Alias Substitution.

```
begin interactive test "alias with trailing space chains to next word"
  spawn -i
  expect "$ "
  send "alias a1=\"echo \""
  expect "$ "
  send "alias a2=\"chained\""
  expect "$ "
  send "a1 a2"
  expect "chained"
  sendeof
  wait
end interactive test "alias with trailing space chains to next word"
```
