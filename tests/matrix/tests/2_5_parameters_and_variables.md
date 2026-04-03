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

#### Test: parameters preserve representative high-bit bytes

Parameters can also hold non-null bytes outside the ASCII range. This test
stores bytes `0xFF` and `0x80` and confirms the exact byte sequence is
preserved.

```
begin test "parameters preserve representative high-bit bytes"
  script
    value=$(printf 'A\377B\200C')
    printf '%s' "$value" | od -An -t x1 | tr -d ' \n'
  expect
    stdout "41ff428043"
    stderr ""
    exit_code 0
end test "parameters preserve representative high-bit bytes"
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

#### Test: ${08} and ${008} use decimal positional index 8

Leading zeroes do not change the decimal interpretation of a positional
parameter index. `${08}` and `${008}` both expand to the eighth positional
parameter.

```
begin test "${08} and ${008} use decimal positional index 8"
  script
    $SHELL -c 'printf "%s\n%s\n" "${08}" "${008}"' sh 1 2 3 4 5 6 7 eighth
  expect
    stdout "eighth\neighth"
    stderr ""
    exit_code 0
end test "${08} and ${008} use decimal positional index 8"
```

#### Test: $10 is $1 followed by literal 0 without braces

Without braces, `$10` is interpreted as `$1` followed by the literal character
`0`, not as the tenth positional parameter.

```
begin test "$10 is $1 followed by literal 0 without braces"
  script
    $SHELL -c 'set -- one two three four five six seven eight nine ten; printf "<%s>\n" "$10"'
  expect
    stdout "<one0>"
    stderr ""
    exit_code 0
end test "$10 is $1 followed by literal 0 without braces"
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
- 0: (Zero.) Expands to the name of the shell or shell script. See [*sh*](../utilities/sh.md) for a detailed description of how this name is derived.

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

#### Test: quoted $@ preserves empty positional parameters

Within double-quotes, `"$@"` produces one field for each positional parameter
that is set, including empty positional parameters.

```
begin test "quoted $@ preserves empty positional parameters"
  script
    $SHELL -c 'set -- a "" c; for i in "$@"; do printf "<%s>" "$i"; done'
  expect
    stdout "<a><><c>"
    stderr ""
    exit_code 0
end test "quoted $@ preserves empty positional parameters"
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

#### Test: quoted $* uses first IFS character as separator

When `"$*"` is expanded and `IFS` is non-empty, the positional parameters are
joined using the first character of `IFS`.

```
begin test "quoted $* uses first IFS character as separator"
  script
    $SHELL -c 'IFS=:; set -- a b c; printf "%s\n" "$*"'
  expect
    stdout "a:b:c"
    stderr ""
    exit_code 0
end test "quoted $* uses first IFS character as separator"
```

#### Test: quoted $* uses only the first character of multi-character IFS

When `IFS` contains more than one character, `"$*"` joins positional
parameters using only the first character.

```
begin test "quoted $* uses only the first character of multi-character IFS"
  script
    $SHELL -c 'IFS=":;"; set -- a b c; printf "%s\n" "$*"'
  expect
    stdout "a:b:c"
    stderr ""
    exit_code 0
end test "quoted $* uses only the first character of multi-character IFS"
```

#### Test: quoted $* uses space when IFS is unset

When `"$*"` is expanded and `IFS` is unset, the positional parameters are
joined using a space.

```
begin test "quoted $* uses space when IFS is unset"
  script
    $SHELL -c 'unset IFS; set -- a b c; printf "%s\n" "$*"'
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "quoted $* uses space when IFS is unset"
```

#### Test: quoted $* uses no separator when IFS is null

When `"$*"` is expanded and `IFS` is set to a null string, the positional
parameters are joined with no separator.

```
begin test "quoted $* uses no separator when IFS is null"
  script
    $SHELL -c 'IFS=; set -- a b c; printf "%s\n" "$*"'
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "quoted $* uses no separator when IFS is null"
```

#### Test: quoted $@ joins prefix and suffix to first and last fields

When `"$@"` is embedded within a word, the first field joins with the prefix
and the last field joins with the suffix.

```
begin test "quoted $@ joins prefix and suffix to first and last fields"
  script
    $SHELL -c 'set -- "a b" c; for x in pre"$@"post; do printf "<%s>" "$x"; done'
  expect
    stdout "<prea b><cpost>"
    stderr ""
    exit_code 0
end test "quoted $@ joins prefix and suffix to first and last fields"
```

#### Test: quoted $@ in ${parameter:-word} retains separate fields

When `"$@"` appears within the *word* of a `${parameter:-word}` expansion in a
context where field splitting would have been performed for `parameter`, its
fields are retained as separate fields.

```
begin test "quoted $@ in ${parameter:-word} retains separate fields"
  script
    $SHELL -c 'unset v; set -- "a b" c; for i in ${v:-"$@"}; do printf "<%s>" "$i"; done'
  expect
    stdout "<a b><c>"
    stderr ""
    exit_code 0
end test "quoted $@ in ${parameter:-word} retains separate fields"
```

#### Test: zero quoted $@ still preserves adjacent quoted null field

If there are no positional parameters and `"$@"` is embedded within a word that
also contains another quoted null part outside the same double-quotes, that
quoted null part still produces an empty field.

```
begin test "zero quoted $@ still preserves adjacent quoted null field"
  script
    set --
    for i in ''"$@"; do
        printf "<%s>" "$i"
    done
  expect
    stdout "<>"
    stderr ""
    exit_code 0
end test "zero quoted $@ still preserves adjacent quoted null field"
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

#### Test: $? after assignment with command substitution uses substitution status

When a command substitution appears in an assignment command, the exit status of
the assignment command becomes the exit status of the command substitution.

```
begin test "$? after assignment with command substitution uses substitution status"
  script
    $SHELL -c 'v=$(false); printf "%s\n" "$?"'
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "$? after assignment with command substitution uses substitution status"
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

#### Test: interactive $- includes i option flag

In an interactive shell, `$-` includes the `i` option flag regardless of how
the shell was invoked.

```
begin interactive test "interactive $- includes i option flag"
  spawn -i
  expect "$ "
  send "case \"\$-\" in (*i*) echo has_i;; (*) echo missing;; esac"
  expect "has_i"
  sendeof
  wait
end interactive test "interactive $- includes i option flag"
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

#### Test: $! updates to the most recent async list

When multiple asynchronous commands are launched, `$!` expands to the process
ID of the most recently started asynchronous command.

```
begin test "$! updates to the most recent async list"
  script
    $SHELL -c 'sleep 0.1 & p1=$!; sleep 0.1 & p2=$!; [ "$p1" != "$p2" ] && [ "$!" = "$p2" ] && echo ok; wait "$p1" "$p2"'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "$! updates to the most recent async list"
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

#### Test: $0 expands to invoked script pathname

When the shell is invoked to run a script, `$0` expands to the script name used
for that invocation.

```
begin test "$0 expands to invoked script pathname"
  script
    cat > ./show0.sh <<'EOF'
    printf "<%s>\n" "$0"
    EOF
    $SHELL ./show0.sh
  expect
    stdout "<./show0.sh>"
    stderr ""
    exit_code 0
end test "$0 expands to invoked script pathname"
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

#### Test: environment names starting with a digit are not initialized

Environment variables whose names start with a digit are not valid shell names
and are not initialized as shell variables.

```
begin test "environment names starting with a digit are not initialized"
  script
    env '1BAD=bad' GOOD_NAME=good $SHELL -c 'echo "$GOOD_NAME"; set | grep -q "^1BAD=" && echo bad || echo ok'
  expect
    stdout "good\nok"
    stderr ""
    exit_code 0
end test "environment names starting with a digit are not initialized"
```

#### Test: for loop name can define a shell variable

The loop control name in a `for` loop can define and initialize a shell
variable even if it was previously unset.

```
begin test "for loop name can define a shell variable"
  script
    unset loop_name
    for loop_name in first second; do :; done
    printf "%s\n" "$loop_name"
  expect
    stdout "second"
    stderr ""
    exit_code 0
end test "for loop name can define a shell variable"
```

#### Test: read utility can define and initialize a shell variable

The `read` utility can define a previously unset shell variable and initialize
it from input.

```
begin test "read utility can define and initialize a shell variable"
  script
    unset read_created
    read read_created <<'EOF'
    hello
    EOF
    printf "<%s>\n" "$read_created"
  expect
    stdout "<hello>"
    stderr ""
    exit_code 0
end test "read utility can define and initialize a shell variable"
```

#### Test: getopts utility can define and initialize a shell variable

The `getopts` utility can define and initialize its option-name variable.

```
begin test "getopts utility can define and initialize a shell variable"
  script
    unset opt_name
    OPTIND=1
    set -- -a
    getopts a opt_name >/dev/null
    printf "<%s>\n" "$opt_name"
  expect
    stdout "<a>"
    stderr ""
    exit_code 0
end test "getopts utility can define and initialize a shell variable"
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

#### Test: ${name=word} can create and initialize a shell variable

The `${name=word}` expansion can define a previously unset shell variable and
initialize it to the given value.

```
begin test "${name=word} can create and initialize a shell variable"
  script
    unset created_by_expansion
    : "${created_by_expansion=made_here}"
    printf "%s\n" "$created_by_expansion"
  expect
    stdout "made_here"
    stderr ""
    exit_code 0
end test "${name=word} can create and initialize a shell variable"
```

#### Test: ENV file processed for interactive shell

When an interactive shell is invoked, the file named by the `ENV` variable
is read and executed before any interactive commands.

```
begin test "ENV file processed for interactive shell"
  script
    _env_file="$PWD/_test_env.sh"
    echo 'ENVMARKER=loaded' > "$_env_file"
    ENV=$_env_file
    export ENV
    $SHELL -i -c 'echo $ENVMARKER' 2>/dev/null
  expect
    stdout "loaded"
    stderr ""
    exit_code 0
end test "ENV file processed for interactive shell"
```

#### Test: ENV pathname is obtained by parameter expansion

When an interactive shell is invoked, the value of `ENV` is subjected to
parameter expansion and the resulting absolute pathname is used.

```
begin test "ENV pathname is obtained by parameter expansion"
  script
    env_root="$PWD"
    _env_file="$PWD/_test_env_param.sh"
    echo 'echo env_param_loaded' > "$_env_file"
    ENV='$env_root/_test_env_param.sh'
    export ENV env_root
    $SHELL -i -c ':' 2>/dev/null
  expect
    stdout "env_param_loaded"
    stderr ""
    exit_code 0
end test "ENV pathname is obtained by parameter expansion"
```

#### Test: ENV startup file need not be executable

The file named by `ENV` is executed for an interactive shell even if the file
itself does not have execute permission.

```
begin test "ENV startup file need not be executable"
  script
    _env_file="$PWD/_env_nonexec.sh"
    echo 'echo nonexec_env_loaded' > "$_env_file"
    chmod 644 "$_env_file"
    ENV="$_env_file"
    export ENV
    $SHELL -i -c ':' 2>/dev/null
  expect
    stdout "nonexec_env_loaded"
    stderr ""
    exit_code 0
end test "ENV startup file need not be executable"
```

#### Test: ENV file is parsed as program so aliases take effect on later lines

The `ENV` file is tokenized and parsed as a program, not as a single compound
list, so an alias defined on one line takes effect on a later line in the same
file.

```
begin test "ENV file is parsed as program so aliases take effect on later lines"
  script
    _env_file="$PWD/_test_env_alias.sh"
    printf '%s\n' 'alias envsay="printf '\''%s\n'\'' alias_ok"' 'envsay' > "$_env_file"
    ENV=$_env_file
    export ENV
    $SHELL -i -c ':' 2>/dev/null
  expect
    stdout "alias_ok"
    stderr ""
    exit_code 0
end test "ENV file is parsed as program so aliases take effect on later lines"
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

#### Test: LANG provides default locale for shell character classes

When no category-specific locale variable overrides it, `LANG` provides the
default locale used by shell pattern character classes.

```
begin test "LANG provides default locale for shell character classes"
  script
    LANG=test_EPTY.ISO-8859-1 LC_ALL= LC_CTYPE= $SHELL -c 'v=$(printf "\\351"); case "$v" in ([[:alpha:]]) echo alpha;; (*) echo no;; esac'
  expect
    stdout "alpha"
    stderr ""
    exit_code 0
end test "LANG provides default locale for shell character classes"
```

#### Test: LC_CTYPE controls shell pattern character classes

The `LC_CTYPE` variable determines which characters are treated as letters by
shell pattern character classes such as `[[:alpha:]]`.

```
begin test "LC_CTYPE controls shell pattern character classes"
  script
    LANG=C LC_ALL= LC_CTYPE=test_EPTY.ISO-8859-1 $SHELL -c 'v=$(printf "\\351"); case "$v" in ([[:alpha:]]) echo alpha;; (*) echo no;; esac'
  expect
    stdout "alpha"
    stderr ""
    exit_code 0
end test "LC_CTYPE controls shell pattern character classes"
```

#### Test: LC_ALL overrides LANG and LC_CTYPE for shell character classes

If `LC_ALL` is set to a non-empty value, it overrides `LANG` and `LC_CTYPE`
when the shell evaluates pattern character classes.

```
begin test "LC_ALL overrides LANG and LC_CTYPE for shell character classes"
  script
    LANG=test_EPTY.ISO-8859-1 LC_CTYPE=test_EPTY.ISO-8859-1 LC_ALL=C $SHELL -c 'v=$(printf "\\351"); case "$v" in ([[:alpha:]]) echo alpha;; (*) echo no;; esac'
  expect
    stdout "no"
    stderr ""
    exit_code 0
end test "LC_ALL overrides LANG and LC_CTYPE for shell character classes"
```

#### Test: LINENO tracks sequential line numbers in a script

The shell sets `LINENO` to the current sequential line number (starting with 1)
within a script before executing each command.

```
begin test "LINENO tracks sequential line numbers in a script"
  script
    cat > _lineno.sh <<'SCRIPT'
    echo "$LINENO"
    echo "$LINENO"
    echo "$LINENO"
    SCRIPT
    $SHELL _lineno.sh
  expect
    stdout "1\n2\n3"
    stderr ""
    exit_code 0
end test "LINENO tracks sequential line numbers in a script"
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

#### Test: IFS is initialized to space tab newline

On shell invocation, `IFS` is initialized to the three default separator
characters: space, tab, and newline.

```
begin test "IFS is initialized to space tab newline"
  script
    printf '%s' "$IFS" | od -An -t x1 | tr -d ' \n'
  expect
    stdout "20090a"
    stderr ""
    exit_code 0
end test "IFS is initialized to space tab newline"
```

#### Test: unset IFS still uses default field-splitting characters

If `IFS` is unset, field splitting is performed as if it were space, tab, and
newline.

```
begin test "unset IFS still uses default field-splitting characters"
  script
    unset IFS
    foo="a b	c
    d"
    for i in $foo; do echo split; done | wc -l | tr -d " "
  expect
    stdout "4"
    stderr ""
    exit_code 0
end test "unset IFS still uses default field-splitting characters"
```

#### Test: IFS affects field splitting performed by read

The value of `IFS` is also used by the `read` utility to split an input line
into fields.

```
begin test "IFS affects field splitting performed by read"
  script
    IFS=:
    read first second <<'EOF'
    a:b
    EOF
    printf "<%s><%s>\n" "$first" "$second"
  expect
    stdout "<a><b>"
    stderr ""
    exit_code 0
end test "IFS affects field splitting performed by read"
```

#### Test: unset IFS makes read use default separators

If `IFS` is unset, the `read` utility splits input as if `IFS` were space, tab,
and newline.

```
begin test "unset IFS makes read use default separators"
  script
    unset IFS
    read first second <<'EOF'
    a b
    EOF
    printf "<%s><%s>\n" "$first" "$second"
  expect
    stdout "<a><b>"
    stderr ""
    exit_code 0
end test "unset IFS makes read use default separators"
```

#### Test: HOME is used for tilde expansion

The contents of `HOME` are used for tilde expansion.

```
begin test "HOME is used for tilde expansion"
  script
    home_dir="$PWD/home_base"
    mkdir "$home_dir"
    HOME="$home_dir" $SHELL -c 'printf "%s\n" ~'
  expect
    stdout ".*/home_base"
    stderr ""
    exit_code 0
end test "HOME is used for tilde expansion"
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

#### Test: PPID is decimal process id

`PPID` is set by the shell to the decimal value of its parent process ID.

```
begin test "PPID is decimal process id"
  script
    case "$PPID" in (""|*[!0-9]*) echo bad;; (*) echo ok;; esac
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "PPID is decimal process id"
```

#### Test: PATH affects command interpretation

The value of `PATH` affects command interpretation by determining where command
names are searched.

```
begin test "PATH affects command interpretation"
  script
    bindir="$PWD/path_bin"
    mkdir "$bindir"
    cat > "$bindir/path_cmd" <<'EOF'
    #!/bin/sh
    printf '%s\n' path-hit
    EOF
    chmod +x "$bindir/path_cmd"
    PATH="$bindir" $SHELL -c 'path_cmd'
  expect
    stdout "path-hit"
    stderr ""
    exit_code 0
end test "PATH affects command interpretation"
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

#### Test: PS4 uses parameter expansion

Before each execution-trace line, `PS4` is subjected to parameter expansion.

```
begin test "PS4 uses parameter expansion"
  script
    prefix=TRACE
    PS4='$prefix> '
    set -x
    echo "traced"
    set +x
  expect
    stdout "(.|\n)*"
    stderr "(.|\n)*TRACE> echo traced(.|\n)*"
    exit_code 0
end test "PS4 uses parameter expansion"
```

#### Test: PWD is set to current working directory

The shell sets `PWD` to the current working directory pathname.

```
begin test "PWD is set to current working directory"
  script
    [ -n "$PWD" ] && [ "$PWD" = "$(pwd -P)" ] && echo ok
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

#### Test: default PS1 prompt is written to stderr

When an interactive shell is ready to read a command, the default PS1 prompt is
written to standard error.

```
begin test "default PS1 prompt is written to stderr"
  script
    stderr_file="$PWD/ps1.stderr"
    $SHELL -i > /dev/null 2>"$stderr_file" <<'EOF'
    exit
    EOF
    grep -q '\$ ' "$stderr_file" && echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "default PS1 prompt is written to stderr"
```

#### Test: PS1 uses parameter expansion

PS1 is subjected to parameter expansion before each interactive prompt.

```
begin interactive test "PS1 uses parameter expansion"
  spawn -i
  expect "$ "
  send "marker=VALUE"
  expect "$ "
  send "PS1='cmd $marker> '"
  expect "cmd VALUE> "
  send "echo interactive_test"
  expect "interactive_test"
  sendeof
  wait
end interactive test "PS1 uses parameter expansion"
```

#### Test: PS1 uses exclamation-mark expansion

Within PS1, `!!` is an escaped exclamation mark and expands to a single literal
`!`.

```
begin interactive test "PS1 uses exclamation-mark expansion"
  spawn -i
  expect "$ "
  send "PS1='cmd !!> '"
  expect "cmd !> "
  send "echo interactive_test"
  expect "interactive_test"
  sendeof
  wait
end interactive test "PS1 uses exclamation-mark expansion"
```

#### Test: PS1 single exclamation-mark expands to next history number

Within PS1, a single `!` expands to the history file number of the next command
to be typed.

```
begin interactive test "PS1 single exclamation-mark expands to next history number"
  spawn -i
  expect "$ "
  send "PS1='cmd !> '"
  expect "cmd [0-9][0-9]*> "
  send "echo interactive_test"
  expect "interactive_test"
  sendeof
  wait
end interactive test "PS1 single exclamation-mark expands to next history number"
```

#### Test: PS2 prompt is written to stderr

When an interactive shell receives a newline before a command is complete, the
expanded PS2 prompt is written to standard error.

```
begin test "PS2 prompt is written to stderr"
  script
    stderr_file="$PWD/ps2.stderr"
    PS2='CONT> ' $SHELL -i > /dev/null 2>"$stderr_file" <<'EOF'
    echo 'unterminated
    '
    exit
    EOF
    grep -q 'CONT> ' "$stderr_file" && echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "PS2 prompt is written to stderr"
```

#### Test: PS2 default prompt is greater-than space

When an interactive shell reads a newline before a command is complete, the
default `PS2` prompt is `"> "`.

```
begin interactive test "PS2 default prompt is greater-than space"
  spawn -i
  expect "$ "
  send "echo 'unterminated"
  expect "> "
  send "'"
  expect "unterminated"
  expect "$ "
  sendeof
  wait
end interactive test "PS2 default prompt is greater-than space"
```

#### Test: PS2 uses parameter expansion

Before each continuation prompt, `PS2` is subjected to parameter expansion.

```
begin interactive test "PS2 uses parameter expansion"
  spawn -i
  expect "$ "
  send "marker=CONT"
  expect "$ "
  send "PS2='\$marker> '"
  expect "$ "
  send "echo 'unterminated"
  expect "CONT> "
  send "'"
  expect "unterminated"
  expect "$ "
  sendeof
  wait
end interactive test "PS2 uses parameter expansion"
```
