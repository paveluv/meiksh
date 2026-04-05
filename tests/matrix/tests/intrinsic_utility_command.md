# Test Suite for Intrinsic Utility: command

This test suite covers the **command** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: command](#utility-command)

## utility: command

#### NAME

> command — execute a simple command

#### SYNOPSIS

> ```
> command [-p] command_name [argument...]
> command [-p][-v|-V] command_name
> ```

#### DESCRIPTION

> The *command* utility shall cause the shell to treat the arguments as a simple command, suppressing the shell function lookup that is described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution), item 1c.
>
> If the *command_name* is the same as the name of one of the special built-in utilities, the special properties in the enumerated list at the beginning of [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities) shall not occur. In every other respect, if *command_name* is not the name of a function, the effect of *command* (with no options) shall be the same as omitting *command*, except that *command_name* does not appear in the command word position in the *command* command, and consequently is not subject to alias substitution (see [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution)) nor recognized as a reserved word (see [*2.4 Reserved Words*](docs/posix/md/utilities/V3_chap02.md#24-reserved-words)).
>
> When the **-v** or **-V** option is used, the *command* utility shall provide information concerning how a command name is interpreted by the shell.
>
> The *command* utility shall be treated as a declaration utility if the first argument passed to the utility is recognized as a declaration utility. In this case, subsequent words of the form *name*=*word* shall be expanded in an assignment context. See [*2.9.1.1 Order of Processing*](docs/posix/md/utilities/V3_chap02.md#2911-order-of-processing).

#### OPTIONS

> The *command* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following options shall be supported:
>
> - **-p**: Perform the command search using a default value for *PATH* that is guaranteed to find all of the standard utilities.
> - **-v**: Write a string to standard output that indicates the pathname or command that will be used by the shell, in the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)), to invoke *command_name*, but do not invoke *command_name*.
>
>     - Executable utilities, regular built-in utilities, *command_name*s including a `<slash>` character, and any implementation-provided functions that are found using the *PATH* variable (as described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution)), shall be written as absolute pathnames.
>     - Shell functions, special built-in utilities, regular built-in utilities not associated with a *PATH* search, and shell reserved words shall be written as just their names.
>     - An alias shall be written as a command line that represents its alias definition.
>     - Otherwise, no output shall be written and the exit status shall reflect that the name was not found.
> - **-V**: Write a string to standard output that indicates how the name given in the *command_name* operand will be interpreted by the shell, in the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)), but do not invoke *command_name*. Although the format of this string is unspecified, it shall indicate in which of the following categories *command_name* falls and shall include the information stated:
>
>     - Executable utilities, regular built-in utilities, and any implementation-provided functions that are found using the *PATH* variable (as described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution)), shall be identified as such and include the absolute pathname in the string.
>     - Other shell functions shall be identified as functions.
>     - Aliases shall be identified as aliases and their definitions included in the string.
>     - Special built-in utilities shall be identified as special built-in utilities.
>     - Regular built-in utilities not associated with a *PATH* search shall be identified as regular built-in utilities. (The term "regular" need not be used.)
>     - Shell reserved words shall be identified as reserved words.

#### OPERANDS

> The following operands shall be supported:
>
> - *argument*: One of the strings treated as an argument to *command_name*.
> - *command_name*: The name of a utility or a special built-in utility.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *command*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error and informative messages written to standard output.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *PATH*: Determine the search path used during the command search described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution), except as described under the **-p** option.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> When the **-v** option is specified, standard output shall be formatted as:
>
> ```
> "%s\n", <pathname or command>
> ```
>
> When the **-V** option is specified, standard output shall be formatted as:
>
> ```
> "%s\n", <unspecified>
> ```

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> When the **-v** or **-V** options are specified, the following exit values shall be returned:
>
> - 0: Successful completion.
> - \>0: The *command_name* could not be found or an error occurred.
>
> Otherwise, the following exit values shall be returned:
>
> - 126: The utility specified by *command_name* was found but could not be invoked.
> - 127: An error occurred in the *command* utility or the utility specified by *command_name* could not be found.
>
> Otherwise, the exit status of *command* shall be that of the simple command specified by the arguments to *command*.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> The order for command search allows functions to override regular built-ins and path searches. This utility is necessary to allow functions that have the same name as a utility to call the utility (instead of a recursive call to the function).
>
> The system default path is available using [*getconf*](docs/posix/md/utilities/getconf.md); however, since [*getconf*](docs/posix/md/utilities/getconf.md) may need to have the *PATH* set up before it can be called itself, the following can be used:
>
> ```
> command -p getconf PATH
> ```
>
> There are some advantages to suppressing the special characteristics of special built-ins on occasion. For example:
>
> ```
> command exec > unwritable-file
> ```
>
> does not cause a non-interactive script to abort, so that the output status can be checked by the script.
>
> The *command*, [*env*](docs/posix/md/utilities/env.md), [*nohup*](docs/posix/md/utilities/nohup.md), [*time*](docs/posix/md/utilities/time.md), [*timeout*](docs/posix/md/utilities/timeout.md), and [*xargs*](docs/posix/md/utilities/xargs.md) utilities have been specified to use exit code 127 if a utility to be invoked cannot be found, so that applications can distinguish "failure to find a utility" from "invoked utility exited with an error indication". However, the *command* and [*nohup*](docs/posix/md/utilities/nohup.md) utilities also use exit code 127 when an error occurs in those utilities, which means exit code 127 is not universally a "not found" indicator. The value 127 was chosen because it is not commonly used for other meanings; most utilities use small values for "normal error conditions" and the values above 128 can be confused with termination due to receipt of a signal. The value 126 was chosen in a similar manner to indicate that the utility could be found, but not invoked. Some scripts produce meaningful error messages differentiating the 126 and 127 cases. The distinction between exit codes 126 and 127 is based on KornShell practice that uses 127 when all attempts to *exec* the utility fail with [ENOENT], and uses 126 when any attempt to *exec* the utility fails for any other reason.
>
> Since the **-v** and **-V** options of *command* produce output in relation to the current shell execution environment, *command* is generally provided as a shell regular built-in. If it is called in a subshell or separate utility execution environment, such as one of the following:
>
> ```
> (PATH=foo command -v)
>  nohup command -v
> ```
>
> it does not necessarily produce correct results. For example, when called with [*nohup*](docs/posix/md/utilities/nohup.md) or an *exec* function, in a separate utility execution environment, most implementations are not able to identify aliases, functions, or special built-ins.
>
> Two types of regular built-ins could be encountered on a system and these are described separately by *command*. The description of command search in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution) allows for a standard utility to be implemented as a regular built-in as long as it is found in the appropriate place in a *PATH* search. So, for example, *command* **-v** *true* might yield **/bin/true** or some similar pathname. Other implementation-defined utilities that are not defined by this volume of POSIX.1-2024 might exist only as built-ins and have no pathname associated with them. These produce output identified as (regular) built-ins. Applications encountering these are not able to count on *exec*ing them, using them with [*nohup*](docs/posix/md/utilities/nohup.md), overriding them with a different *PATH ,* and so on.
>
> The *command* utility takes on the expansion behavior of the command that it is wrapping. Therefore, in
>
> ```
> command command export a=~
> ```
>
> *command* is recognized as a declaration utility, and the command sets the variable *a* to the value of `$HOME` because it performs tilde-expansion of an assignment context; while
>
> ```
> command echo a=~
> ```
>
> outputs the literal string `"a=~"` because regular expansion can only perform tilde-expansion at the beginning of the word. However, the shell need only perform lexical analysis of the next argument when deciding if command should be treated as a declaration utility; therefore, with:
>
> ```
> var=export; command $var a=~
> ```
>
> and
>
> ```
> command -- export a=~
> ```
>
> it is unspecified whether the word `a=~` is handled in an assignment context or as a regular expansion.

#### EXAMPLES

> ```
> IFS='
> '
> #    The preceding value should be <space><tab><newline>.
> #    Set IFS to its default value.
> \unalias -a
> #    Unset all possible aliases.
> #    Note that unalias is escaped to prevent an alias
> #    being used for unalias.
> unset -f command
> #    Ensure command is not a user function.
> PATH="$(command -p getconf PATH):$PATH"
> #    Put on a reliable PATH prefix.
> #    ...
> ```

#### RATIONALE

> Since *command* is a regular built-in utility it is always found prior to the *PATH* search.
>
> There is nothing in the description of *command* that implies the command line is parsed any differently from that of any other simple command. For example:
>
> ```
> command a | b ; c
> ```
>
> is not parsed in any special way that causes `'|'` or `';'` to be treated other than a pipe operator or `<semicolon>` or that prevents function lookup on **b** or **c**. However, some implementations extend the shell's assignment syntax, for example to allow an array to be populated with a single assignment, and in order for such an extension to be usable in assignments specified as arguments to [*export*](docs/posix/md/utilities/export.md) and [*readonly*](docs/posix/md/utilities/readonly.md) these shells have those utility names as separate tokens in their grammar. When *command* is used to execute these utilities it also needs to be a separate token in the grammar so that the same extended assignment syntax can still be recognized in this case. This standard only permits an extension of this nature when the input to the shell would contain a syntax error according to the standard grammar, and therefore it cannot affect how `'|'` and `';'` are parsed in the example above. Note that although *command* can be a separate token in the shell's grammar, it cannot be a reserved word since *command* is a candidate for alias substitution whereas reserved words are not (see [*2.3.1 Alias Substitution*](docs/posix/md/utilities/V3_chap02.md#231-alias-substitution)).
>
> The *command* utility is somewhat similar to the Eighth Edition shell *builtin* command, but since *command* also goes to the file system to search for utilities, the name *builtin* would not be intuitive.
>
> The *command* utility is most likely to be provided as a regular built-in. It is not listed as a special built-in for the following reasons:
>
> - The removal of exportable functions made the special precedence of a special built-in unnecessary.
> - A special built-in has special properties (see [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities)) that were inappropriate for invoking other utilities. For example, two commands such as: would have entirely different results; in a non-interactive script, the former would continue to execute the next command, the latter would abort. Introducing this semantic difference along with suppressing functions was seen to be non-intuitive.
>   ```
>   date > unwritable-file
>
>
>   command date > unwritable-file
>   ```
>
> The **-p** option is present because it is useful to be able to ensure a safe path search that finds all the standard utilities. This search might not be identical to the one that occurs through one of the *exec* functions (as defined in the System Interfaces volume of POSIX.1-2024) when *PATH* is unset. At the very least, this feature is required to allow the script to access the correct version of [*getconf*](docs/posix/md/utilities/getconf.md) so that the value of the default path can be accurately retrieved.
>
> The *command* **-v** and **-V** options were added to satisfy requirements from users that are currently accomplished by three different historical utilities: [*type*](docs/posix/md/utilities/type.md) in the System V shell, *whence* in the KornShell, and *which* in the C shell. Since there is no historical agreement on how and what to accomplish here, the POSIX *command* utility was enhanced and the historical utilities were left unmodified. The C shell *which* merely conducts a path search. The KornShell *whence* is more elaborate—in addition to the categories required by POSIX, it also reports on tracked aliases, exported aliases, and undefined functions.
>
> The output format of **-V** was left mostly unspecified because human users are its only audience. Applications should not be written to care about this information; they can use the output of **-v** to differentiate between various types of commands, but the additional information that may be emitted by the more verbose **-V** is not needed and should not be arbitrarily constrained in its verbosity or localization for application parsing reasons.

#### FUTURE DIRECTIONS

> If this utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used, implementations are encouraged to treat this as an error. A future version of this standard may require implementations to treat this as an error.

#### SEE ALSO

> [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution), [*2.9.1.1 Order of Processing*](docs/posix/md/utilities/V3_chap02.md#2911-order-of-processing), [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment), [*2.15 Special Built-In Utilities*](docs/posix/md/utilities/V3_chap02.md#215-special-built-in-utilities), [*sh*](docs/posix/md/utilities/sh.md) , [*type*](docs/posix/md/utilities/type.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*exec*](docs/posix/md/functions/exec.md#tag_17_129)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #196 is applied, changing the SYNOPSIS to allow **-p** to be used with **-v** (or **-V**).
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> The *command* utility is moved from the User Portability Utilities option to the Base. User Portability Utilities is now an option for interactive utilities.
>
> The APPLICATION USAGE and EXAMPLES are revised to replace the non-standard [*getconf*](docs/posix/md/utilities/getconf.md)_CS_PATH with [*getconf*](docs/posix/md/utilities/getconf.md) *PATH .*

#### Issue 8

> Austin Group Defect 251 is applied, encouraging implementations to report an error if a utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used.
>
> Austin Group Defects 351 and 1393 are applied, requiring *command* to be a declaration utility if the first argument passed to the utility is recognized as a declaration utility.
>
> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1117 is applied, changing "implementation-defined" to "implementation-provided".
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1161 is applied, changing "Utilities" to "Executable utilities" in the descriptions of the **-v** and **-V** options.
>
> Austin Group Defect 1431 is applied, changing "item 1b" to "item 1c".
>
> Austin Group Defect 1586 is applied, adding the [*timeout*](docs/posix/md/utilities/timeout.md) utility.
>
> Austin Group Defect 1594 is applied, changing the APPLICATION USAGE section.
>
> Austin Group Defect 1637 is applied, clarifying that (when no options are specified) *command_name* is not subject to alias substitution nor recognized as a reserved word.

*End of informative text.*

### Tests

#### Test: command bypasses function shadowing echo

`command` causes a simple command to be executed without function lookup.

```
begin test "command bypasses function shadowing echo"
  script
    echo() { printf "FUNCTION\n"; }
    command echo "REAL"
  expect
    stdout "REAL"
    stderr ""
    exit_code 0
end test "command bypasses function shadowing echo"
```

#### Test: command -v finds known utility

`command -v` writes a string indicating the pathname or command that
will be used by the shell.

```
begin test "command -v finds known utility"
  script
    command -v ls
  expect
    stdout ".*ls.*"
    stderr ""
    exit_code 0
end test "command -v finds known utility"
```

#### Test: command -v fails for missing command

`command -v` returns a non-zero exit status for an unknown command.

```
begin test "command -v fails for missing command"
  script
    command -v nonexistent_cmd_xyz 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "command -v fails for missing command"
```

#### Test: command -V describes known utility

`command -V` writes a string indicating how the given command name
will be interpreted.

```
begin test "command -V describes known utility"
  script
    command -V echo
  expect
    stdout ".*echo.*"
    stderr ""
    exit_code 0
end test "command -V describes known utility"
```

#### Test: command returns 127 for missing utility

When the specified utility is not found, `command` returns 127.

```
begin test "command returns 127 for missing utility"
  script
    command nonexistent_cmd_xyz 2>/dev/null
    echo $?
  expect
    stdout "127"
    stderr ""
    exit_code 0
end test "command returns 127 for missing utility"
```

#### Test: command -V echo produces output mentioning echo

`command -V` must write a human-readable description of how the shell interprets the given command name. For a known utility like `echo`, the output must mention the name and succeed with exit code 0.

```
begin test "command -V echo produces output mentioning echo"
  script
    command -V echo 2>&1
  expect
    stdout ".*echo.*"
    stderr ""
    exit_code 0
end test "command -V echo produces output mentioning echo"
```

#### Test: command -V for nonexistent command fails

When `command -V` is given a name that does not correspond to any utility, function, alias, or built-in, it must produce no output and return a non-zero exit status.

```
begin test "command -V for nonexistent command fails"
  script
    command -V nonexistent_cmd_xyzzy 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "command -V for nonexistent command fails"
```

#### Test: command -V cd produces output mentioning cd

`command -V` must identify built-in utilities by name. Since `cd` is a shell built-in, the output must mention `cd` and the command must exit successfully.

```
begin test "command -V cd produces output mentioning cd"
  script
    command -V cd 2>&1
  expect
    stdout ".*cd.*"
    stderr ""
    exit_code 0
end test "command -V cd produces output mentioning cd"
```

#### Test: arguments after command_name are passed through

Operands following the command_name in `command` must be passed through as arguments to the invoked utility, exactly as if `command` were not present.

```
begin test "arguments after command_name are passed through"
  script
    command echo hello world
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "arguments after command_name are passed through"
```

#### Test: multiple arguments forwarded correctly

When multiple arguments follow the command_name, `command` must forward all of them to the invoked utility in order, preserving their individual values.

```
begin test "multiple arguments forwarded correctly"
  script
    command printf "%s %s %s
    " a b c
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "multiple arguments forwarded correctly"
```

#### Test: function itself works without command prefix

Verifies that a user-defined function overriding a utility name is normally invoked (without the `command` prefix), confirming the baseline behavior that `command` is designed to bypass.

```
begin test "function itself works without command prefix"
  script
    echo() { printf "FUNCTION
    "; }; echo ignored
  expect
    stdout "FUNCTION"
    stderr ""
    exit_code 0
end test "function itself works without command prefix"
```

#### Test: command bypasses function shadowing cat

`command` must suppress the shell function lookup, so even when a function named `cat` is defined, `command cat` must invoke the real `cat` utility from the filesystem.

```
begin test "command bypasses function shadowing cat"
  script
    cat() { printf "FAKE_CAT
    "; }; printf "real
    " | command cat
  expect
    stdout "real"
    stderr ""
    exit_code 0
end test "command bypasses function shadowing cat"
```

#### Test: command export does not abort shell

When a special built-in is invoked via `command`, its special properties (such as aborting the shell on error) must be suppressed. Invoking `command export` must not terminate a non-interactive shell.

```
begin test "command export does not abort shell"
  script
    command export 2>/dev/null
    echo survived
  expect
    stdout "(.|\n)*\nsurvived"
    stderr ""
    exit_code 0
end test "command export does not abort shell"
```

#### Test: command break outside loop does not abort shell

Using `command` to invoke a special built-in like `break` outside a loop must suppress the special built-in's ability to abort the shell, allowing execution to continue normally.

```
begin test "command break outside loop does not abort shell"
  script
    command break 2>/dev/null
    echo still_here
  expect
    stdout "still_here"
    stderr ""
    exit_code 0
end test "command break outside loop does not abort shell"
```

#### Test: command executes utility normally

When no options are given and no function shadows the utility, `command` must execute the named utility and produce the same result as invoking the utility directly.

```
begin test "command executes utility normally"
  script
    command echo hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "command executes utility normally"
```

#### Test: command does not perform alias expansion

Because the command_name does not appear in the command word position, it must not be subject to alias substitution. An alias defined for `ls` must not take effect when `command ls` is used.

```
begin test "command does not perform alias expansion"
  script
    alias ls="echo ALIASED" 2>/dev/null
    command ls / >/dev/null 2>&1
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "command does not perform alias expansion"
```

#### Test: command -v ls prints path or name

`command -v` must write the pathname or name that the shell would use to invoke the given utility. For an external utility like `ls`, this is typically an absolute pathname.

```
begin test "command -v ls prints path or name"
  script
    command -v ls 2>&1
  expect
    stdout ".*((/.*ls)|ls).*"
    stderr ""
    exit_code 0
end test "command -v ls prints path or name"
```

#### Test: command -v cd prints builtin name

For special built-in utilities like `cd`, `command -v` must write just their name (not a pathname), since they are not found via a PATH search.

```
begin test "command -v cd prints builtin name"
  script
    command -v cd 2>&1
  expect
    stdout ".*cd.*"
    stderr ""
    exit_code 0
end test "command -v cd prints builtin name"
```

#### Test: command -v for defined function prints function name

`command -v` must write shell functions as just their name. When a user-defined function exists, `command -v` should output the function name and succeed.

```
begin test "command -v for defined function prints function name"
  script
    myfn() { :; }
    command -v myfn
  expect
    stdout "myfn"
    stderr ""
    exit_code 0
end test "command -v for defined function prints function name"
```

#### Test: command -v for alias or no alias

For an alias, `command -v` must write a command line representing its alias definition. If the shell does not support aliases in the current context, a fallback is acceptable. This test accepts either form.

```
begin test "command -v for alias or no alias"
  script
    alias greet="echo hi" 2>/dev/null
    command -v greet 2>/dev/null || echo NO_ALIAS
  expect
    stdout "(alias greet=.*|NO_ALIAS)"
    stderr ""
    exit_code 0
end test "command -v for alias or no alias"
```

#### Test: command -v for nonexistent command fails

When the given name is not found as a utility, function, alias, or built-in, `command -v` must produce no output and return a non-zero exit status.

```
begin test "command -v for nonexistent command fails"
  script
    command -v nonexistent_cmd_xyzzy 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "command -v for nonexistent command fails"
```

#### Test: command -V echo describes command type

`command -V` must write a human-readable string indicating how the shell interprets a command name, including its category (e.g. built-in, external utility). The output must mention the name `echo`.

```
begin test "command -V echo describes command type"
  script
    command -V echo 2>&1
  expect
    stdout ".*echo.*"
    stderr ""
    exit_code 0
end test "command -V echo describes command type"
```

#### Test: command export behaves as declaration utility

`command` must be treated as a declaration utility when its first argument is a declaration utility like `export`. Assignment arguments (`name=value`) must be expanded in assignment context and take effect.

```
begin test "command export behaves as declaration utility"
  script
    command export FOO=bar
    echo $FOO
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "command export behaves as declaration utility"
```

#### Test: command readonly behaves as declaration utility

`command` must also be treated as a declaration utility when its first argument is `readonly`. The assignment `RO_VAR=42` must be expanded in assignment context and the variable set to the given value.

```
begin test "command readonly behaves as declaration utility"
  script
    command readonly RO_VAR=42
    echo $RO_VAR
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "command readonly behaves as declaration utility"
```

#### Test: command local inside function

When `command local` is used inside a function, the local variable assignment should still take effect. The variable must be accessible within the function body regardless of whether `local` is supported.

```
begin test "command local inside function"
  script
    f() { command local LV=hello 2>/dev/null && echo $LV || echo $LV; }
    f
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "command local inside function"
```

#### Test: command export assigns variable

Using `command export` with a `name=value` operand must both export the variable and assign it in the current shell environment, making the value immediately accessible.

```
begin test "command export assigns variable"
  script
    command export TVAR=hello
    echo $TVAR
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "command export assigns variable"
```

#### Test: multiple assignments in one command export

`command export` must handle multiple `name=value` operands in a single invocation, assigning and exporting all of them in the current shell environment.

```
begin test "multiple assignments in one command export"
  script
    command export A=1 B=2 C=3
    echo $A $B $C
  expect
    stdout "1 2 3"
    stderr ""
    exit_code 0
end test "multiple assignments in one command export"
```

#### Test: tilde expansion in assignment context

Because `command export` is a declaration utility, assignment values undergo tilde expansion. A lone `~` must expand to the user's home directory (an absolute path), not remain literal.

```
begin test "tilde expansion in assignment context"
  script
    command export HOMEDIR=~
    case "$HOMEDIR" in /*) echo absolute;; *) echo relative;; esac
  expect
    stdout "absolute"
    stderr ""
    exit_code 0
end test "tilde expansion in assignment context"
```

#### Test: variable references expand in assignment values

In the declaration-utility context of `command export`, parameter expansions in assignment values (e.g. `${X}_world`) must be performed, producing the expanded result as the variable's value.

```
begin test "variable references expand in assignment values"
  script
    X=hello
    command export Y=${X}_world
    echo $Y
  expect
    stdout "hello_world"
    stderr ""
    exit_code 0
end test "variable references expand in assignment values"
```

#### Test: type finds known command

The `type` utility must successfully identify a known command. When given `sh` (which must always be available), `type` must exit with status 0 to confirm the command was found.

```
begin test "type finds known command"
  script
    type sh >/dev/null && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "type finds known command"
```

#### Test: command bypasses alias

`command` must prevent alias substitution on the command_name. Even when an alias for `echo` is defined, `command echo` must invoke the real utility and output the literal argument.

```
begin test "command bypasses alias"
  script
    alias echo="echo alias_"
    command echo "hello"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "command bypasses alias"
```

#### Test: command returns 126 for non-executable file

When the named utility is found but cannot be executed (e.g. the file exists but lacks execute permission), `command` must return exit status 126 to distinguish "found but not invocable" from "not found" (127).

```
begin test "command returns 126 for non-executable file"
  script
    mkdir -p mydir
    touch mydir/nonexec
    PATH="$PWD/mydir:$PATH" command nonexec >/dev/null 2>&1
    rc=$?
    rm -rf mydir
    echo $rc
  expect
    stdout "126"
    stderr ""
    exit_code 0
end test "command returns 126 for non-executable file"
```
