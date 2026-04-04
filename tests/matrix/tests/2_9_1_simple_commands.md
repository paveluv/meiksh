# Test Suite for 2.9.1 Simple Commands

This test suite covers **Section 2.9.1 Simple Commands** of the POSIX.1-2024
Shell Command Language specification (part of 2.9 Shell Commands), including
order of processing, variable assignments, commands with no command name,
command search and execution, standard file descriptors, and non-built-in
utility execution.

## Table of contents

- [2.9.1 Simple Commands](#291-simple-commands)
- [2.9.1.1 Order of Processing](#2911-order-of-processing)
- [2.9.1.2 Variable Assignments](#2912-variable-assignments)
- [2.9.1.3 Commands with no Command Name](#2913-commands-with-no-command-name)
- [2.9.1.4 Command Search and Execution](#2914-command-search-and-execution)
- [2.9.1.5 Standard File Descriptors](#2915-standard-file-descriptors)
- [2.9.1.6 Non-built-in Utility Execution](#2916-non-built-in-utility-execution)

## 2.9.1 Simple Commands

A "simple command" is a sequence of optional variable assignments and redirections, in any sequence, optionally followed by words and redirections.

### Tests

#### Test: simple command execution order

A simple command performs variable assignments and then executes the command
with the assigned values available.

```
begin test "simple command execution order"
  script
    X=value
    echo $X
  expect
    stdout "value"
    stderr ""
    exit_code 0
end test "simple command execution order"
```

## 2.9.1.1 Order of Processing

When a given simple command is required to be executed (that is, when any conditional construct such as an AND-OR list or a **case** statement has not bypassed the simple command), the following expansions, assignments, and redirections shall all be performed from the beginning of the command text to the end:

1. The words that are recognized as variable assignments or redirections according to [2.10.2 Shell Grammar Rules](#2102-shell-grammar-rules) are saved for processing in steps 3 and 4.
2. The first word (if any) that is not a variable assignment or redirection shall be expanded. If any fields remain following its expansion, the first field shall be considered the command name. If no fields remain, the next word (if any) shall be expanded, and so on, until a command name is found or no words remain. If there is a command name and it is recognized as a declaration utility, then any remaining words after the word that expanded to produce the command name, that would be recognized as a variable assignment in isolation, shall be expanded as a variable assignment (tilde expansion after the first `<equals-sign>` and after any unquoted `<colon>`, parameter expansion, command substitution, arithmetic expansion, and quote removal, but no field splitting or pathname expansion); while remaining words that would not be a variable assignment in isolation shall be subject to regular expansion (tilde expansion for only a leading `<tilde>`, parameter expansion, command substitution, arithmetic expansion, field splitting, pathname expansion, and quote removal). For all other command names, words after the word that produced the command name shall be subject only to regular expansion. All fields resulting from the expansion of the word that produced the command name and the subsequent words, except for the field containing the command name, shall be the arguments for the command.
3. Redirections shall be performed as described in [2.7 Redirection](#27-redirection).
4. Each variable assignment shall be expanded for tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal prior to assigning the value.

In the preceding list, the order of steps 3 and 4 may be reversed if no command name results from step 2 or if the command name matches the name of a special built-in utility; see [2.15 Special Built-In Utilities](#215-special-built-in-utilities).

When determining whether a command name is a declaration utility, an implementation may use only lexical analysis. It is unspecified whether assignment context will be used if the command name would only become recognized as a declaration utility after word expansions.

### Tests

#### Test: empty first word skipped to find command

If the first word expands to nothing, the shell continues to the next word
to find the command name.

```
begin test "empty first word skipped to find command"
  script
    empty=""
    $empty printf "%s\n" "hello"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "empty first word skipped to find command"
```

#### Test: variable expands into command name and argument

The first field from word expansion becomes the command name; subsequent
fields become arguments.

```
begin test "variable expands into command name and argument"
  script
    cmd="printf %s\n"; $cmd "hello"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "variable expands into command name and argument"
```

#### Test: redirection with no command creates file

Redirections are performed even when there is no command name (step 3).

```
begin test "redirection with no command creates file"
  script
    > tmp_redir.txt
    ls tmp_redir.txt
  expect
    stdout "tmp_redir.txt"
    stderr ""
    exit_code 0
end test "redirection with no command creates file"
```

#### Test: arithmetic expansion in variable assignment preceding command

Variable assignments (step 4) undergo arithmetic expansion before the value
is assigned.

```
begin test "arithmetic expansion in variable assignment preceding command"
  script
    my_var=$((2+3)) env | grep -q "^my_var=5$" && echo "expanded"
  expect
    stdout "expanded"
    stderr ""
    exit_code 0
end test "arithmetic expansion in variable assignment preceding command"
```

#### Test: export with variable assignment is visible in environment

When a declaration utility like `export` is the command name, assignment words
undergo assignment expansion and the variable is exported.

```
begin test "export with variable assignment is visible in environment"
  script
    export var=123
    env | grep "^var="
  expect
    stdout "var=123"
    stderr ""
    exit_code 0
end test "export with variable assignment is visible in environment"
```

#### Test: tilde expansion in assignment context

When a declaration utility is the command name, assignment words undergo tilde
expansion after `=` and after unquoted `:`.

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

## 2.9.1.2 Variable Assignments

Variable assignments shall be performed as follows:

- If no command name results, variable assignments shall affect the current execution environment.
- If the command name is not a special built-in utility or function, the variable assignments shall be exported for the execution environment of the command and shall not affect the current execution environment except as a side-effect of the expansions performed in step 4. In this case it is unspecified:
    - Whether or not the assignments are visible for subsequent expansions in step 4
    - Whether variable assignments made as side-effects of these expansions are visible for subsequent expansions in step 4, or in the current shell execution environment, or both
- If the command name is a standard utility implemented as a function (see XBD [*4.25 Utility*](docs/posix/md/basedefs/V1_chap04.md#425-utility)), the effect of variable assignments shall be as if the utility was not implemented as a function.
- If the command name is a special built-in utility, variable assignments shall affect the current execution environment before the utility is executed and remain in effect when the command completes; if an assigned variable is further modified by the utility, the modifications made by the utility shall persist. Unless the [*set*](#set) **-a** option is on (see [set](#tag_19_26)), it is unspecified:
    - Whether or not the variables gain the *export* attribute during the execution of the special built-in utility
    - Whether or not *export* attributes gained as a result of the variable assignments persist after the completion of the special built-in utility
- If the command name is a function that is not a standard utility implemented as a function, variable assignments shall affect the current execution environment during the execution of the function. It is unspecified:
    - Whether or not the variable assignments persist after the completion of the function
    - Whether or not the variables gain the *export* attribute during the execution of the function
    - Whether or not *export* attributes gained as a result of the variable assignments persist after the completion of the function (if variable assignments persist after the completion of the function)

If any of the variable assignments attempt to assign a value to a variable for which the *readonly* attribute is set in the current shell environment (regardless of whether the assignment is made in that environment), a variable assignment error shall occur. See [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) for the consequences of these errors.

### Tests

#### Test: assignment with no command alters shell state

When no command name is present, variable assignments persist in the current
execution environment.

```
begin test "assignment with no command alters shell state"
  script
    FOO=bar
    printf "%s\n" "$FOO"
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "assignment with no command alters shell state"
```

#### Test: assignment before regular command is temporary

Variable assignments before a non-special-built-in command are exported for
that command's environment but do not persist in the current shell.

```
begin test "assignment before regular command is temporary"
  script
    FOO=bar sh -c 'echo $FOO'
    echo "${FOO:-unset}"
  expect
    stdout "bar\nunset"
    stderr ""
    exit_code 0
end test "assignment before regular command is temporary"
```

#### Test: assignment before special built-in persists

Variable assignments before a special built-in utility affect the current
execution environment and persist after the command completes.

```
begin test "assignment before special built-in persists"
  script
    FOO=bar export DUMMY=1
    printf "%s\n" "$FOO"
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "assignment before special built-in persists"
```

#### Test: assignment to readonly variable fails

Assigning to a readonly variable causes a variable assignment error,
regardless of the command context.

```
begin test "assignment to readonly variable fails"
  script
    readonly FOO=1
    FOO=2
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "assignment to readonly variable fails"
```

#### Test: function call with var assignment affects function environment

Variable assignments before a function call affect the execution environment
during the function's execution.

```
begin test "function call with var assignment affects function environment"
  script
    my_func() { echo "$my_var"; }
    my_var="old"
    my_var="new" my_func
  expect
    stdout "new"
    stderr ""
    exit_code 0
end test "function call with var assignment affects function environment"
```

## 2.9.1.3 Commands with no Command Name

If a simple command has no command name after word expansion (see [2.9.1.1 Order of Processing](#2911-order-of-processing)), any redirections shall be performed in a subshell environment; it is unspecified whether this subshell environment is the same one as that used for a command substitution within the command. (To affect the current execution environment, see the [exec](#tag_19_21) special built-in.) If any of the redirections performed in the current shell execution environment fail, the command shall immediately fail with an exit status greater than zero, and the shell shall write an error message indicating the failure. See [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) for the consequences of these failures on interactive and non-interactive shells.

Additionally, if there is no command name but the command contains a command substitution, the command shall complete with the exit status of the command substitution whose exit status was the last to be obtained. Otherwise, the command shall complete with a zero exit status.

### Tests

#### Test: command substitution exit status propagates

When there is no command name but a command substitution is present, the exit
status is that of the last command substitution.

```
begin test "command substitution exit status propagates"
  script
    var=$(false)
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "command substitution exit status propagates"
```

#### Test: simple assignment completes with zero exit status

When there is no command name and no command substitution, the command
completes with exit status zero.

```
begin test "simple assignment completes with zero exit status"
  script
    FOO=bar
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "simple assignment completes with zero exit status"
```

## 2.9.1.4 Command Search and Execution

If a simple command has a command name and an optional list of arguments after word expansion (see [2.9.1.1 Order of Processing](#2911-order-of-processing)), the following actions shall be performed:

1. If the command name does not contain any `<slash>` characters, the first successful step in the following sequence shall occur:
    1. If the command name matches the name of a special built-in utility, that special built-in utility shall be invoked.
    2. If the command name matches the name of a utility listed in the following table, the results are unspecified.
      - *alloc*
      - *autoload*
      - *bind*
      - *bindkey*
      - *builtin*
      - *bye*
      - *caller*
      - *cap*
      - *chdir*
      - *clone*
      - *comparguments*
      - *compcall*
      - *compctl*
      - *compdescribe*
      - *compfiles*
      - *compgen*
      - *compgroups*
      - *complete*
      - *compound*
      - *compquote*
      - *comptags*
      - *comptry*
      - *compvalues*
      - *declare*
      - *dirs*
      - *disable*
      - *disown*
      - *dosh*
      - *echotc*
      - *echoti*
      - *enum*
      - *float*
      - *help*
      - *history*
      - *hist*
      - *integer*
      - *let*
      - *local*
      - *login*
      - *logout*
      - *map*
      - *mapfile*
      - *nameref*
      - *popd*
      - *print*
      - *pushd*
      - *readarray*
      - *repeat*
      - *savehistory*
      - *source*
      - *shopt*
      - *stop*
      - *suspend*
      - *typeset*
      - *whence*
    3. If the command name matches the name of a function known to this shell, the function shall be invoked as described in [2.9.5 Function Definition Command](#295-function-definition-command). If the implementation has provided a standard utility in the form of a function, and that function definition still exists (i.e. has not been removed using [*unset*](#unset) **-f** or replaced via another function definition with the same name), it shall not be recognized at this point. It shall be invoked in conjunction with the path search in step 1e.
    4. If the command name matches the name of an intrinsic utility (see [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities)), that utility shall be invoked.
    5. Otherwise, the command shall be searched for using the *PATH* environment variable as described in XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables):
          1. If the search is successful: Once a utility has been searched for and found (either as a result of this specific search or as part of an unspecified shell start-up activity), an implementation may remember its location and need not search for the utility again unless the *PATH* variable has been the subject of an assignment. If the remembered location fails for a subsequent invocation, the shell shall repeat the search to find the new location for the utility, if any.
                  1. If the system has implemented the utility as a built-in or as a shell function, and the built-in or function is associated with the directory that was most recently tested during the successful *PATH* search, that built-in or function shall be invoked.
                  2. Otherwise, the shell shall execute a non-built-in utility as described in [2.9.1.6 Non-built-in Utility Execution](#2916-non-built-in-utility-execution).
          2. If the search is unsuccessful, the command shall fail with an exit status of 127 and the shell shall write an error message.
2. If the command name contains at least one `<slash>`, the shell shall execute a non-built-in utility as described in [2.9.1.6 Non-built-in Utility Execution](#2916-non-built-in-utility-execution).

### Tests

#### Test: command name resolves to regular utility and executes

A command without slashes that is not a special built-in or function is
found via PATH search and executed.

```
begin test "command name resolves to regular utility and executes"
  script
    echo test_utility
  expect
    stdout "test_utility"
    stderr ""
    exit_code 0
end test "command name resolves to regular utility and executes"
```

#### Test: command without slashes resolves via PATH

A command name without slashes is searched for using the PATH variable.

```
begin test "command without slashes resolves via PATH"
  script
    mkdir -p tmp_path_test
    echo 'echo "found_in_path"' > tmp_path_test/my_custom_cmd
    chmod +x tmp_path_test/my_custom_cmd
    PATH="$PWD/tmp_path_test:$PATH" my_custom_cmd
  expect
    stdout "found_in_path"
    stderr ""
    exit_code 0
end test "command without slashes resolves via PATH"
```

#### Test: command with slash executes directly

A command name containing a slash is executed directly, bypassing the PATH
search.

```
begin test "command with slash executes directly"
  script
    mkdir -p tmp_path_test
    echo 'echo "found_in_path"' > tmp_path_test/my_custom_cmd
    chmod +x tmp_path_test/my_custom_cmd
    ./tmp_path_test/my_custom_cmd
  expect
    stdout "found_in_path"
    stderr ""
    exit_code 0
end test "command with slash executes directly"
```

#### Test: non-existent command returns 127

If the PATH search is unsuccessful, the command fails with exit status 127.

```
begin test "non-existent command returns 127"
  script
    this_command_does_not_exist_xyz123
    echo "$?"
  expect
    stdout "127"
    stderr ".+"
    exit_code 0
end test "non-existent command returns 127"
```

#### Test: prefix variable assignment passed to command

Variable assignments preceding a command name are exported to the command's
environment.

```
begin test "prefix variable assignment passed to command"
  script
    FOO=bar $SHELL -c 'echo $FOO'
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "prefix variable assignment passed to command"
```

## 2.9.1.5 Standard File Descriptors

If the utility would be executed with file descriptor 0, 1, or 2 closed, implementations may execute the utility with the file descriptor open to an unspecified file. If a standard utility or a conforming application is executed with file descriptor 0 not open for reading or with file descriptor 1 or 2 not open for writing, the environment in which the utility or application is executed shall be deemed non-conforming, and consequently the utility or application might not behave as described in this standard.

### Tests

## 2.9.1.6 Non-built-in Utility Execution

When the shell executes a non-built-in utility, if the execution is not being made via the [*exec*](#exec) special built-in utility, the shell shall execute the utility in a separate utility environment (see [2.13 Shell Execution Environment](#213-shell-execution-environment)).

If the execution is being made via the [*exec*](#exec) special built-in utility, the shell shall not create a separate utility environment for this execution; the new process image shall replace the current shell execution environment. If the current shell environment is a subshell environment, the new process image shall replace the subshell environment and the shell shall continue in the environment from which that subshell environment was invoked.

In either case, execution of the utility in the specified environment shall be performed as follows:

1. If the command name does not contain any `<slash>` characters, the command name shall be searched for using the *PATH* environment variable as described in XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables):
    1. If the search is successful, the shell shall execute the utility with actions equivalent to calling the [*execl*()](docs/posix/md/functions/execl.md) function as defined in the System Interfaces volume of POSIX.1-2024 with the *path* argument set to the pathname resulting from the search, *arg0* set to the command name, and the remaining [*execl*()](docs/posix/md/functions/execl.md) arguments set to the command arguments (if any) and the null terminator. If the [*execl*()](docs/posix/md/functions/execl.md) function fails due to an error equivalent to the [ENOEXEC] error defined in the System Interfaces volume of POSIX.1-2024, the shell shall execute a command equivalent to having a shell invoked with the pathname resulting from the search as its first operand, with any remaining arguments passed to the new shell, except that the value of `"$0"` in the new shell may be set to the command name. The shell may apply a heuristic check to determine if the file to be executed could be a script and may bypass this command execution if it determines that the file cannot be a script. In this case, it shall write an error message, and the command shall fail with an exit status of 126. It is unspecified whether environment variables that were passed to the shell when it was invoked, but were not used to initialize shell variables (see [2.5.3 Shell Variables](#253-shell-variables)) because they had invalid names, are included in the environment passed to [*execl*()](docs/posix/md/functions/execl.md) and (if [*execl*()](docs/posix/md/functions/execl.md) fails as described above) to the new shell.
      **Note:** A common heuristic for rejecting files that cannot be a script is locating a NUL byte prior to a `<newline>` byte within a fixed-length prefix of the file. Since [*sh*](docs/posix/md/utilities/sh.md) is required to accept input files with unlimited line lengths, the heuristic check cannot be based on line length.
    2. If the search is unsuccessful, the command shall fail with an exit status of 127 and the shell shall write an error message.
2. If the command name contains at least one `<slash>`:
    1. If the named utility exists, the shell shall execute the utility with actions equivalent to calling the [*execl*()](docs/posix/md/functions/execl.md) function defined in the System Interfaces volume of POSIX.1-2024 with the *path* and *arg0* arguments set to the command name, and the remaining [*execl*()](docs/posix/md/functions/execl.md) arguments set to the command arguments (if any) and the null terminator. If the [*execl*()](docs/posix/md/functions/execl.md) function fails due to an error equivalent to the [ENOEXEC] error, the shell shall execute a command equivalent to having a shell invoked with the command name as its first operand, with any remaining arguments passed to the new shell. The shell may apply a heuristic check to determine if the file to be executed could be a script and may bypass this command execution if it determines that the file cannot be a script. In this case, it shall write an error message, and the command shall fail with an exit status of 126. It is unspecified whether environment variables that were passed to the shell when it was invoked, but were not used to initialize shell variables (see [2.5.3 Shell Variables](#253-shell-variables)) because they had invalid names, are included in the environment passed to [*execl*()](docs/posix/md/functions/execl.md) and (if [*execl*()](docs/posix/md/functions/execl.md) fails as described above) to the new shell.
      **Note:** A common heuristic for rejecting files that cannot be a script is locating a NUL byte prior to a `<newline>` byte within a fixed-length prefix of the file. Since [*sh*](docs/posix/md/utilities/sh.md) is required to accept input files with unlimited line lengths, the heuristic check cannot be based on line length.
    2. If the named utility does not exist, the command shall fail with an exit status of 127 and the shell shall write an error message.

### Tests

#### Test: subshell does not affect parent variable

Non-built-in utilities run in a separate utility environment; changes to
variables in the child do not affect the parent shell.

```
begin test "subshell does not affect parent variable"
  script
    X=0
    (X=1)
    echo $X
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "subshell does not affect parent variable"
```

#### Test: file without magic header but with exec bit runs as shell script

When execl() fails with ENOEXEC, the shell falls back to invoking the file
as a shell script.

```
begin test "file without magic header but with exec bit runs as shell script"
  script
    mkdir -p tmp_path_test
    echo "echo executed_fallback" > tmp_path_test/no_magic_header
    chmod +x tmp_path_test/no_magic_header
    ./tmp_path_test/no_magic_header
  expect
    stdout "executed_fallback"
    stderr ""
    exit_code 0
end test "file without magic header but with exec bit runs as shell script"
```

#### Test: non-executable file returns 126

If the file exists but is not executable, the command fails with exit
status 126.

```
begin test "non-executable file returns 126"
  script
    touch tmp_not_exec
    chmod -x tmp_not_exec
    ./tmp_not_exec
    echo "$?"
  expect
    stdout "126"
    stderr ".+"
    exit_code 0
end test "non-executable file returns 126"
```

#### Test: non-existent file with slash fails with exit code 127

If the named utility (with a slash) does not exist, the command fails with
exit status 127.

```
begin test "non-existent file with slash fails with exit code 127"
  script
    mkdir -p tmp_path_test
    ./tmp_path_test/does_not_exist_123
  expect
    stdout ""
    stderr ".+"
    exit_code 127
end test "non-existent file with slash fails with exit code 127"
```
