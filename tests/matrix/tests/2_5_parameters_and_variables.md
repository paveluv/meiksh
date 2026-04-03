# Test Suite for 2.5 Parameters and Variables

This test suite covers **Section 2.5 Parameters and Variables** of the POSIX Shell
Command Language (POSIX.1-2024), which defines parameter types (named variables,
positional parameters, and special parameters), their assignment and expansion
semantics, and the shell variables that affect execution.

## Table of contents

- [2.5 Parameters and Variables](#25-parameters-and-variables)
- [2.5.1 Positional Parameters](#251-positional-parameters)
- [2.5.2 Special Parameters](#252-special-parameters)
- [2.5.3 Shell Variables](#253-shell-variables)

## 2.5 Parameters and Variables

A parameter can be denoted by a name, a number, or one of the special characters listed in [2.5.2 Special Parameters](#252-special-parameters). A variable is a parameter denoted by a name.

A parameter is set if it has an assigned value (null is a valid value). Once a variable is set, it can only be unset by using the [*unset*](#unset) special built-in command.

Parameters can contain arbitrary byte sequences, except for the null byte. The shell shall process their values as characters only when performing operations that are described in this standard in terms of characters.

### Tests

#### Test: parameters can be referenced by name number and special character

A parameter can be a named variable (`$var`), a positional parameter (`$1`),
or a special parameter (`$#`). All three forms work in a single expansion.

```
begin test "parameters can be referenced by name number and special character"
  script
    $SHELL -c 'set -- p1 p2; var=NAME; echo "$var|$1|$#"'
  expect
    stdout "NAME\|p1\|2"
    stderr ""
    exit_code 0
end test "parameters can be referenced by name number and special character"
```

#### Test: variable is parameter referenced by name

A variable is simply a parameter denoted by a name. Assigning `named_value=abc`
creates a variable that can be expanded with `$named_value`.

```
begin test "variable is parameter referenced by name"
  script
    named_value=abc
    echo "$named_value"
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "variable is parameter referenced by name"
```

#### Test: null assignment still means parameter is set

A parameter assigned the null string is still considered "set". The `${var+SET}`
expansion distinguishes between set-to-null and unset.

```
begin test "null assignment still means parameter is set"
  script
    empty=
    unset missing
    [ "${empty+SET}" = "SET" ] && echo empty_set
    [ "${missing+SET}" = "SET" ] || echo missing_unset
  expect
    stdout "empty_set\nmissing_unset"
    stderr ""
    exit_code 0
end test "null assignment still means parameter is set"
```

#### Test: assigned variable remains set until unset is used

Once a variable is set (even reassigned to null), it stays set until explicitly
removed with the `unset` built-in.

```
begin test "assigned variable remains set until unset is used"
  script
    v=hello
    v=
    [ "${v+SET}" = "SET" ] && echo still_set
    unset v
    [ "${v+SET}" = "SET" ] || echo now_unset
  expect
    stdout "still_set\nnow_unset"
    stderr ""
    exit_code 0
end test "assigned variable remains set until unset is used"
```

#### Test: parameters preserve representative non-null bytes

Parameters can hold arbitrary byte sequences except null. This test stores
a value containing a tab (`\t`) and newline (`\n`) and confirms the exact
bytes are preserved via `od`.

```
begin test "parameters preserve representative non-null bytes"
  script
    value=$(printf 'A\tB\nC')
    printf '%s' "$value" | od -An -t x1 | tr -d ' \n'
  expect
    stdout "4109420a43"
    stderr ""
    exit_code 0
end test "parameters preserve representative non-null bytes"
```

## 2.5.1 Positional Parameters

A positional parameter is a parameter denoted by a decimal representation of a positive integer. The digits denoting the positional parameters shall always be interpreted as a decimal value, even if there is a leading zero. When a positional parameter with more than one digit is specified, the application shall enclose the digits in braces (see [2.6.2 Parameter Expansion](#262-parameter-expansion)).

Examples:

- `"$8"`, `"${8}"`, `"${08}"`, `"${008}"`, etc. all expand to the value of the eighth positional parameter.
- `"${10}"` expands to the value of the tenth positional parameter.
- `"$10"` expands to the value of the first positional parameter followed by the character '0'.

**Note:** 0 is a special parameter, not a positional parameter, and therefore the results of expanding `${00}` are unspecified.

Positional parameters are initially assigned when the shell is invoked (see [*sh*](../utilities/sh.md)), temporarily replaced when a shell function is invoked (see [2.9.5 Function Definition Command](#295-function-definition-command)), and can be reassigned with the [*set*](#set) special built-in command.

### Tests

#### Test: positional parameters use positive integer indices

Positional parameters are denoted by positive integers. `$1` and `$2` expand to
the first and second arguments passed to the shell.

```
begin test "positional parameters use positive integer indices"
  script
    $SHELL -c 'set -- first second; echo "$1,$2"'
  expect
    stdout "first,second"
    stderr ""
    exit_code 0
end test "positional parameters use positive integer indices"
```

#### Test: $01 is $0 followed by literal 1, and ${10} expands to 10th arg

Without braces, `$01` is `$0` followed by the literal character `1`. Multi-digit
positional parameters require braces: `${10}` expands to the tenth argument.

```
begin test "$01 is $0 followed by literal 1, and ${10} expands to 10th arg"
  script
    $SHELL -c 'echo "$01"; echo "${10}"' '$SHELL' 1 2 3 4 5 6 7 8 9 10th
  expect
    stdout ".*1\n10th"
    stderr ""
    exit_code 0
end test "$01 is $0 followed by literal 1, and ${10} expands to 10th arg"
```

#### Test: positional parameters follow invocation function and set lifecycle

Positional parameters are set at shell invocation, temporarily replaced inside
a function call, and can be reassigned with `set --`. After the function returns,
the outer positional parameters are restored.

```
begin test "positional parameters follow invocation function and set lifecycle"
  script
    $SHELL -c 'echo "init:$1"; set -- reset1 reset2; echo "after_set:$1"; f(){ echo "in_func:$1"; }; f funcarg; echo "after_func:$1"' sh invoked1 invoked2
  expect
    stdout "init:invoked1\nafter_set:reset1\nin_func:funcarg\nafter_func:reset1"
    stderr ""
    exit_code 0
end test "positional parameters follow invocation function and set lifecycle"
```

#### Test: $@ with no positional params generates zero fields

When there are no positional parameters, `"$@"` generates zero fields — the
loop body does not execute at all.

```
begin test "$@ with no positional params generates zero fields"
  script
    $SHELL -c 'for i in "$@"; do echo "found: $i"; done' sh
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "$@ with no positional params generates zero fields"
```

## 2.5.2 Special Parameters

Listed below are the special parameters and the values to which they shall expand. Only the values of the special parameters are listed; see [2.6 Word Expansions](#26-word-expansions) for a detailed summary of all the stages involved in expanding words.

- `@`: Expands to the positional parameters, starting from one, initially producing one field for each positional parameter that is set. When the expansion occurs in a context where field splitting will be performed, any empty fields may be discarded and each of the non-empty fields shall be further split as described in [2.6.5 Field Splitting](#265-field-splitting). When the expansion occurs within double-quotes, the behavior is unspecified unless one of the following is true:

    - Field splitting as described in [2.6.5 Field Splitting](#265-field-splitting) would be performed if the expansion were not within double-quotes (regardless of whether field splitting would have any effect; for example, if *IFS* is null).
    - The double-quotes are within the *word* of a ${*parameter*:-*word*} or a ${*parameter*:+*word*} expansion (with or without the `<colon>`; see [2.6.2 Parameter Expansion](#262-parameter-expansion)) which would have been subject to field splitting if *parameter* had been expanded instead of *word*.

  If one of these conditions is true, the initial fields shall be retained as separate fields, except that if the parameter being expanded was embedded within a word, the first field shall be joined with the beginning part of the original word and the last field shall be joined with the end part of the original word. In all other contexts the results of the expansion are unspecified. If there are no positional parameters, the expansion of `'@'` shall generate zero fields, even when `'@'` is within double-quotes; however, if the expansion is embedded within a word which contains one or more other parts that expand to a quoted null string, these null string(s) shall still produce an empty field, except that if the other parts are all within the same double-quotes as the `'@'`, it is unspecified whether the result is zero fields or one empty field.
- `*`: Expands to the positional parameters, starting from one, initially producing one field for each positional parameter that is set. When the expansion occurs in a context where field splitting will be performed, any empty fields may be discarded and each of the non-empty fields shall be further split as described in [2.6.5 Field Splitting](#265-field-splitting). When the expansion occurs in a context where field splitting will not be performed, the initial fields shall be joined to form a single field with the value of each parameter separated by the first character of the *IFS* variable if *IFS* contains at least one character, or separated by a `<space>` if *IFS* is unset, or with no separation if *IFS* is set to a null string.
- `#`: Expands to the shortest representation of the decimal number of positional parameters. The command name (parameter 0) shall not be counted in the number given by `'#'` because it is a special parameter, not a positional parameter.
- `?`: Expands to the shortest representation of the decimal exit status (see [2.8.2 Exit Status for Commands](#282-exit-status-for-commands)) of the pipeline (see [2.9.2 Pipelines](#292-pipelines)) executed from the current shell execution environment (not a subshell environment) that most recently either terminated or, optionally but only if the shell is interactive and job control is enabled, was stopped by a signal. If this pipeline terminated, the status value shall be its exit status; otherwise, the status value shall be the same as the exit status that would have resulted if the pipeline had been terminated by a signal with the same number as the signal that stopped it. The value of the special parameter `'?'` shall be set to 0 during initialization of the shell. When a subshell environment is created, the value of the special parameter `'?'` from the invoking shell environment shall be preserved in the subshell.

    - **Note:** In `var=$(some_command); echo $?` the output is the exit status of `some_command`, which is executed in a subshell environment, but this is because its exit status becomes the exit status of the assignment command `var=$(some_command)` (see [2.9.1 Simple Commands](#291-simple-commands)) and this assignment command is the most recently completed pipeline. Likewise for any pipeline consisting entirely of a simple command that has no command word, but contains one or more command substitutions. (See [2.9.1 Simple Commands](#291-simple-commands).)
- `-`: (Hyphen.) Expands to the current option flags (the single-letter option names concatenated into a string) as specified on invocation, by the [*set*](#set) special built-in command, or implicitly by the shell. It is unspecified whether the **-c** and **-s** options are included in the expansion of `"$-"`. The **-i** option shall be included in `"$-"` if the shell is interactive, regardless of whether it was specified on invocation.
- `$`: Expands to the shortest representation of the decimal process ID of the invoked shell. In a subshell (see [2.13 Shell Execution Environment](#213-shell-execution-environment)), `'$'` shall expand to the same value as that of the current shell.
- `!`: Expands to the shortest representation of the decimal process ID associated with the most recent asynchronous AND-OR list (see [2.9.3.1 Asynchronous AND-OR Lists](#2931-asynchronous-and-or-lists)) executed from the current shell execution environment, or to the shortest representation of the decimal process ID of the last command specified in the currently executing pipeline in the job-control background job that most recently resumed execution through the use of [*bg*](../utilities/bg.md), whichever is the most recent.
- 0: (Zero.) Expands to the name of the shell or shell script. See [*sh*](../utilities/sh.md#) for a detailed description of how this name is derived.

See the description of the *IFS* variable in [2.5.3 Shell Variables](#253-shell-variables).

### Tests

#### Test: @ expands positional parameters in order

`"$@"` expands to each positional parameter as a separate field, preserving
their original order.

```
begin test "@ expands positional parameters in order"
  script
    $SHELL -c 'set -- a b c; for i in "$@"; do printf "<%s>" "$i"; done'
  expect
    stdout "<a><b><c>"
    stderr ""
    exit_code 0
end test "@ expands positional parameters in order"
```

#### Test: * expands positional parameters in order

Unquoted `$*` expands to the positional parameters, each as a separate field
subject to field splitting.

```
begin test "* expands positional parameters in order"
  script
    $SHELL -c 'set -- a b c; for i in $*; do printf "<%s>" "$i"; done'
  expect
    stdout "<a><b><c>"
    stderr ""
    exit_code 0
end test "* expands positional parameters in order"
```

#### Test: $* and $@ without quotes split on IFS

Without quotes, both `$*` and `$@` undergo field splitting on `IFS`, producing
the same individual fields.

```
begin test "$* and $@ without quotes split on IFS"
  script
    $SHELL -c 'for i in $*; do echo "$i"; done; for i in $@; do echo "$i"; done' sh a b c
  expect
    stdout "a\nb\nc\na\nb\nc"
    stderr ""
    exit_code 0
end test "$* and $@ without quotes split on IFS"
```

#### Test: quoted $* is single string, quoted $@ is distinct args

Inside double-quotes, `"$*"` joins all positional parameters into a single
field (separated by the first character of IFS), while `"$@"` retains each
as a separate field.

```
begin test "quoted $* is single string, quoted $@ is distinct args"
  script
    $SHELL -c 'for i in "$*"; do echo "$i"; done; for i in "$@"; do echo "$i"; done' sh a b c
  expect
    stdout "a b c\na\nb\nc"
    stderr ""
    exit_code 0
end test "quoted $* is single string, quoted $@ is distinct args"
```

#### Test: # expands to decimal positional count

`$#` expands to the number of positional parameters (not counting `$0`).

```
begin test "# expands to decimal positional count"
  script
    $SHELL -c 'set -- a b c d; echo "$#"'
  expect
    stdout "4"
    stderr ""
    exit_code 0
end test "# expands to decimal positional count"
```

#### Test: $# counts positional parameters not including $0

The command name (`$0`) is a special parameter and is not counted by `$#`.

```
begin test "$# counts positional parameters not including $0"
  script
    $SHELL -c 'echo "$#"' sh a b c
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "$# counts positional parameters not including $0"
```

#### Test: ? reflects most recent pipeline status

`$?` expands to the exit status of the most recently completed pipeline.
After `false`, it should be `1`.

```
begin test "? reflects most recent pipeline status"
  script
    $SHELL -c 'false; echo "$?"'
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "? reflects most recent pipeline status"
```

#### Test: $? is 0 at shell startup

The special parameter `$?` is initialized to 0 when the shell starts.

```
begin test "$? is 0 at shell startup"
  script
    echo "$?"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "$? is 0 at shell startup"
```

#### Test: $? preserved in subshell

When a subshell is created, the value of `$?` from the parent environment
is preserved. After `false`, a subshell sees `$?` as `1`.

```
begin test "$? preserved in subshell"
  script
    false
    (echo "$?")
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "$? preserved in subshell"
```

#### Test: $- includes options enabled by set

`$-` expands to the current single-letter option flags. After `set -u`, the
expansion includes `u`.

```
begin test "$- includes options enabled by set"
  script
    $SHELL -c 'set -u; case "$-" in (*u*) echo has_u;; (*) echo missing;; esac'
  expect
    stdout "has_u"
    stderr ""
    exit_code 0
end test "$- includes options enabled by set"
```

#### Test: $$ is decimal process id

`$$` expands to the decimal process ID of the invoked shell. The value must
consist only of digits.

```
begin test "$$ is decimal process id"
  script
    $SHELL -c 'case "$$" in (""|*[!0-9]*) echo bad;; (*) echo ok;; esac'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "$$ is decimal process id"
```

#### Test: $$ same in subshell

In a subshell, `$$` expands to the same value as in the parent shell, not
the subshell's own PID.

```
begin test "$$ same in subshell"
  script
    parent="$$"
    sub="$(echo "$$")"
    [ "$parent" = "$sub" ] && echo "same"
  expect
    stdout "same"
    stderr ""
    exit_code 0
end test "$$ same in subshell"
```

#### Test: $! gives pid of most recent async list

`$!` expands to the PID of the most recent asynchronous command. After
launching `sleep 0.1 &`, `$!` must be a valid decimal number.

```
begin test "$! gives pid of most recent async list"
  script
    $SHELL -c 'sleep 0.1 & p=$!; wait "$p"; case "$p" in (""|*[!0-9]*) echo bad;; (*) echo ok;; esac'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "$! gives pid of most recent async list"
```

#### Test: $0 expands to shell or script name

`$0` expands to the name of the shell or script. When invoked with `-c` and
a name argument, it reflects that name.

```
begin test "$0 expands to shell or script name"
  script
    $SHELL -c 'echo "$0"' custom_name
  expect
    stdout "custom_name"
    stderr ""
    exit_code 0
end test "$0 expands to shell or script name"
```

## 2.5.3 Shell Variables

Variables shall be initialized from the environment (as defined by XBD [*8. Environment Variables*](../basedefs/V1_chap08.md#8-environment-variables) and the *exec* function in the System Interfaces volume of POSIX.1-2024) and can be given new values with variable assignment commands. Shell variables shall be initialized only from environment variables that have valid names. If a variable is initialized from the environment, it shall be marked for export immediately; see the [*export*](#export) special built-in. New variables can be defined and initialized with variable assignments, with the [*read*](../utilities/read.md) or [*getopts*](../utilities/getopts.md) utilities, with the *name* parameter in a **for** loop, with the ${*name*=*word*} expansion, or with other mechanisms provided as implementation extensions.

The following variables shall affect the execution of the shell:

- *ENV*: The processing of the *ENV* shell variable shall be supported if the system supports the User Portability Utilities option. This variable, when and only when an interactive shell is invoked, shall be subjected to parameter expansion (see [2.6.2 Parameter Expansion](#262-parameter-expansion)) by the shell and the resulting value shall be used as a pathname of a file. Before any interactive commands are read, the shell shall tokenize (see [2.3 Token Recognition](#23-token-recognition)) the contents of the file, parse the tokens as a *program* (see [2.10 Shell Grammar](#210-shell-grammar)), and execute the resulting commands in the current environment. (In other words, the contents of the *ENV* file are not parsed as a single *compound_list*. This distinction matters because it influences when aliases take effect.) The file need not be executable. If the expanded value of *ENV* is not an absolute pathname, the results are unspecified. *ENV* shall be ignored if the user's real and effective user IDs or real and effective group IDs are different.
- *HOME*: The pathname of the user's home directory. The contents of *HOME* are used in tilde expansion (see [2.6.1 Tilde Expansion](#261-tilde-expansion)).
- *IFS*: A string treated as a list of characters that is used for field splitting, expansion of the `'*'` special parameter, and to split lines into fields with the [*read*](../utilities/read.md) utility. If the value of *IFS* includes any bytes that do not form part of a valid character, the results of field splitting, expansion of `'*'`, and use of the [*read*](../utilities/read.md) utility are unspecified. If *IFS* is not set, it shall behave as normal for an unset variable, except that field splitting by the shell and line splitting by the [*read*](../utilities/read.md) utility shall be performed as if the value of *IFS* is `<space>``<tab>``<newline>`; see [2.6.5 Field Splitting](#265-field-splitting). The shell shall set *IFS* to `<space>``<tab>``<newline>` when it is invoked.
- *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](../basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
- *LC_ALL*: The value of this variable overrides the *LC_** variables and *LANG ,* as described in XBD [*8. Environment Variables*](../basedefs/V1_chap08.md#8-environment-variables).
- *LC_COLLATE*: Determine the behavior of range expressions, equivalence classes, and multi-character collating elements within pattern matching.
- *LC_CTYPE*: Determine the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters), which characters are defined as letters (character class **alpha**) and `<blank>` characters (character class **blank**), and the behavior of character classes within pattern matching. Changing the value of *LC_CTYPE* after the shell has started shall not affect the lexical processing of shell commands in the current shell execution environment or its subshells. Invoking a shell script or performing [*exec*](#exec) [*sh*](../utilities/sh.md) subjects the new shell to the changes in *LC_CTYPE .*
- *LC_MESSAGES*: Determine the language in which messages should be written.
- *LINENO*: The processing of the *LINENO* shell variable shall be supported if the system supports the User Portability Utilities option. Set by the shell to a decimal number representing the current sequential line number (numbered starting with 1) within a script or function before it executes each command. If the user unsets or resets *LINENO ,* the variable may lose its special meaning for the life of the shell. If the shell is not currently executing a script or function, the value of *LINENO* is unspecified.
- *NLSPATH*: Determine the location of message catalogs for the processing of *LC_MESSAGES .*
- *PATH*: A string formatted as described in XBD [*8. Environment Variables*](../basedefs/V1_chap08.md#8-environment-variables), used to effect command interpretation; see [2.9.1.4 Command Search and Execution](#2914-command-search-and-execution).
- *PPID*: Set by the shell to the decimal value of its parent process ID during initialization of the shell. In a subshell (see [2.13 Shell Execution Environment](#213-shell-execution-environment)), *PPID* shall be set to the same value as that of the parent of the current shell. For example, [*echo*](../utilities/echo.md) $*PPID* and ([*echo*](../utilities/echo.md) $*PPID )* would produce the same value.
- *PS1*: The processing of the *PS1* shell variable shall be supported if the system supports the User Portability Utilities option. Each time an interactive shell is ready to read a command, the value of this variable shall be subjected to parameter expansion (see [2.6.2 Parameter Expansion](#262-parameter-expansion)) and exclamation-mark expansion (see below). Whether the value is also subjected to command substitution (see [2.6.3 Command Substitution](#263-command-substitution)) or arithmetic expansion (see [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion)) or both is unspecified. After expansion, the value shall be written to standard error. The expansions shall be performed in two passes, where the result of the first pass is input to the second pass. One of the passes shall perform only the exclamation-mark expansion described below. The other pass shall perform the other expansion(s) according to the rules in [2.6 Word Expansions](#26-word-expansions). Which of the two passes is performed first is unspecified. The default value shall be `"$ "`. For users who have specific additional implementation-defined privileges, the default may be another, implementation-defined value. Exclamation-mark expansion: The shell shall replace each instance of the `<exclamation-mark>` character (`'!'`) with the history file number (see [*Command History List*](../utilities/sh.md#command-history-list)) of the next command to be typed. An `<exclamation-mark>` character escaped by another `<exclamation-mark>` character (that is, `"!!"`) shall expand to a single `<exclamation-mark>` character.
- *PS2*: The processing of the *PS2* shell variable shall be supported if the system supports the User Portability Utilities option. Each time the user enters a `<newline>` prior to completing a command line in an interactive shell, the value of this variable shall be subjected to parameter expansion (see [2.6.2 Parameter Expansion](#262-parameter-expansion)). Whether the value is also subjected to command substitution (see [2.6.3 Command Substitution](#263-command-substitution)) or arithmetic expansion (see [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion)) or both is unspecified. After expansion, the value shall be written to standard error. The default value shall be `"> "`.
- *PS4*: The processing of the *PS4* shell variable shall be supported if the system supports the User Portability Utilities option. When an execution trace ([*set*](#set) **-x**) is being performed, before each line in the execution trace, the value of this variable shall be subjected to parameter expansion (see [2.6.2 Parameter Expansion](#262-parameter-expansion)). Whether the value is also subjected to command substitution (see [2.6.3 Command Substitution](#263-command-substitution)) or arithmetic expansion (see [2.6.4 Arithmetic Expansion](#264-arithmetic-expansion)) or both is unspecified. After expansion, the value shall be written to standard error. The default value shall be `"+ "`.
- *PWD*: Set by the shell and by the [*cd*](../utilities/cd.md) utility. In the shell the value shall be initialized from the environment as follows. If a value for *PWD* is passed to the shell in the environment when it is executed, the value is an absolute pathname of the current working directory that is no longer than {PATH_MAX} bytes including the terminating null byte, and the value does not contain any components that are dot or dot-dot, then the shell shall set *PWD* to the value from the environment. Otherwise, if a value for *PWD* is passed to the shell in the environment when it is executed, the value is an absolute pathname of the current working directory, and the value does not contain any components that are dot or dot-dot, then it is unspecified whether the shell sets *PWD* to the value from the environment or sets *PWD* to the pathname that would be output by [*pwd*](../utilities/pwd.md) **-P**. Otherwise, the [*sh*](../utilities/sh.md) utility sets *PWD* to the pathname that would be output by [*pwd*](../utilities/pwd.md) **-P**. In cases where *PWD* is set to the value from the environment, the value can contain components that refer to files of type symbolic link. In cases where *PWD* is set to the pathname that would be output by [*pwd*](../utilities/pwd.md) **-P**, if there is insufficient permission on the current working directory, or on any parent of that directory, to determine what that pathname would be, the value of *PWD* is unspecified. Assignments to this variable may be ignored. If an application sets or unsets the value of *PWD ,* the behaviors of the [*cd*](../utilities/cd.md) and [*pwd*](../utilities/pwd.md) utilities are unspecified.

### Tests

#### Test: environment variable is visible via env

Variables assigned and exported in the shell are visible in the environment
of child processes.

```
begin test "environment variable is visible via env"
  script
    TEST_ENV_VAR=value
    export TEST_ENV_VAR
    env | grep -q "^TEST_ENV_VAR=" && echo "exported"
  expect
    stdout "exported"
    stderr ""
    exit_code 0
end test "environment variable is visible via env"
```

#### Test: invalid environment names are not initialized as shell variables

Only environment variables with valid names (letters, digits, underscores,
starting with a non-digit) are initialized as shell variables. Names like
`BAD-NAME` containing hyphens are ignored.

```
begin test "invalid environment names are not initialized as shell variables"
  script
    env 'BAD-NAME=bad' GOOD_NAME=good $SHELL -c 'echo "$GOOD_NAME"; set | grep -q "^BAD-NAME=" && echo bad || echo ok'
  expect
    stdout "good\nok"
    stderr ""
    exit_code 0
end test "invalid environment names are not initialized as shell variables"
```

#### Test: environment-initialized variable is exported to child process

A variable initialized from the environment is automatically marked for
export and remains visible to child processes.

```
begin test "environment-initialized variable is exported to child process"
  script
    FOO_FROM_ENV=bar $SHELL -c 'env | grep -q "^FOO_FROM_ENV=bar$" && echo exported'
  expect
    stdout "exported"
    stderr ""
    exit_code 0
end test "environment-initialized variable is exported to child process"
```

#### Test: ENV file processed for interactive shell

When an interactive shell is invoked, the file named by the `ENV` variable
is read and executed before any interactive commands.

```
begin test "ENV file processed for interactive shell"
  script
    _env_file=_test_env.sh
    echo 'ENVMARKER=loaded' > $_env_file
    EFILE=$_env_file
    export EFILE
    ENV=$EFILE
    export ENV
    $SHELL -i -c 'echo $ENVMARKER' 2>/dev/null
  expect
    stdout "loaded"
    stderr ""
    exit_code 0
end test "ENV file processed for interactive shell"
```

#### Test: ENV is ignored for non-interactive shell

The `ENV` file is only processed for interactive shells. A non-interactive
shell invoked with `-c` does not execute the `ENV` file.

```
begin test "ENV is ignored for non-interactive shell"
  script
    _env_file=_test_env_noninteractive.sh
    echo 'echo env_ran_noninteractive' > "$_env_file"
    ENV=$_env_file $SHELL -c 'echo done'
  expect
    stdout "done"
    stderr ""
    exit_code 0
end test "ENV is ignored for non-interactive shell"
```

#### Test: ENV set to valid file does not crash non-interactive shell

Setting `ENV` to a valid file for a non-interactive shell should have no
effect — the shell runs normally without executing the `ENV` file.

```
begin test "ENV set to valid file does not crash non-interactive shell"
  script
    _env_file=_test_env2.sh
    echo 'echo env_ran' > $_env_file
    ENV=$_env_file $SHELL -c true 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "ENV set to valid file does not crash non-interactive shell"
```

#### Test: IFS default splits on space, tab, and newline

The shell initializes IFS to space, tab, and newline. A string containing all
three separators is split into four fields.

```
begin test "IFS default splits on space, tab, and newline"
  script
    foo="a b	c
    d"; for i in $foo; do echo "split"; done | wc -l | tr -d " "
  expect
    stdout "4"
    stderr ""
    exit_code 0
end test "IFS default splits on space, tab, and newline"
```

#### Test: LC_CTYPE change mid-script does not break lexical processing

Changing `LC_CTYPE` after the shell has started does not affect lexical
processing of commands in the current execution environment.

```
begin test "LC_CTYPE change mid-script does not break lexical processing"
  script
    LC_CTYPE=C
    export LC_CTYPE
    echo "hello world"
    LC_CTYPE=POSIX
    export LC_CTYPE
    echo "still works"
  expect
    stdout "hello world\nstill works"
    stderr ""
    exit_code 0
end test "LC_CTYPE change mid-script does not break lexical processing"
```

#### Test: PPID same in subshell

In a subshell, `PPID` is set to the same value as the parent of the current
shell (not the subshell's parent).

```
begin test "PPID same in subshell"
  script
    parent="$PPID"
    sub="$(echo "$PPID")"
    [ "$parent" = "$sub" ] && echo "same"
  expect
    stdout "same"
    stderr ""
    exit_code 0
end test "PPID same in subshell"
```

#### Test: set -x traces commands with default PS4 prefix

When `set -x` enables execution tracing, each traced command is prefixed
with the default PS4 value `"+ "` on stderr.

```
begin test "set -x traces commands with default PS4 prefix"
  script
    set -x
    echo "traced"
    set +x
  expect
    stdout "(.|\n)*"
    stderr "(.|\n)*\+ echo traced(.|\n)*"
    exit_code 0
end test "set -x traces commands with default PS4 prefix"
```

#### Test: changing PS4 alters trace prefix

Assigning a new value to `PS4` changes the prefix used for `set -x` trace
output. Here `PS4` includes `$LINENO` which is expanded before display.

```
begin test "changing PS4 alters trace prefix"
  script
    PS4="TRACE:\$LINENO> "
    set -x
    echo "traced"
    set +x
  expect
    stdout "(.|\n)*"
    stderr "(.|\n)*TRACE:(.|\n)*"
    exit_code 0
end test "changing PS4 alters trace prefix"
```

#### Test: PWD is set to current working directory

The shell sets `PWD` to the current working directory. It must be non-empty.

```
begin test "PWD is set to current working directory"
  script
    test -n "$PWD" && echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "PWD is set to current working directory"
```

#### Test: PWD is initialized from valid environment value

If a valid absolute pathname for the current directory is passed via the `PWD`
environment variable, the shell initializes `PWD` from it.

```
begin test "PWD is initialized from valid environment value"
  script
    cwd=$(pwd)
    PWD="$cwd" $SHELL -c '[ "$PWD" = "$1" ] && echo from_env_ok' sh "$cwd"
  expect
    stdout "from_env_ok"
    stderr ""
    exit_code 0
end test "PWD is initialized from valid environment value"
```

#### Test: PS1 parameter and exclamation-mark expansion

PS1 is subjected to parameter expansion and exclamation-mark expansion before
each interactive prompt. This test sets PS1 with a history number placeholder
and a command substitution, then verifies the prompt changes.

```
begin interactive test "PS1 parameter and exclamation-mark expansion"
  spawn -i
  expect "$ "
  send "PS1='cmd \\! var $(echo 1)> '"
  expect "cmd .* var 1>"
  send "echo interactive_test"
  expect "interactive_test"
  sendeof
  wait
end interactive test "PS1 parameter and exclamation-mark expansion"
```
