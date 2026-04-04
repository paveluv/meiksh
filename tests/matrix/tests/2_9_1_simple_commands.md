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

#### Test: assignment and redirection intermixed with no command name

A simple command is a sequence of optional variable assignments and
redirections in any sequence. When there is no command name, both the
assignment and the redirection are processed regardless of textual order.

```
begin test "assignment and redirection intermixed with no command name"
  script
    rm -f tmp_intermixed.txt
    > tmp_intermixed.txt VAR=intermixed
    echo "$VAR"
    test -f tmp_intermixed.txt && echo "file_exists"
  expect
    stdout "intermixed\nfile_exists"
    stderr ""
    exit_code 0
end test "assignment and redirection intermixed with no command name"
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

#### Test: first expanded field is command name and remaining fields are arguments

If the first non-assignment, non-redirection word expands to multiple fields,
the first resulting field is the command name and the remaining fields are
arguments to that command.

```
begin test "first expanded field is command name and remaining fields are arguments"
  script
    cmd='set -- one two'
    $cmd
    printf "%s\n%s\n%s\n" "$#" "$1" "$2"
  expect
    stdout "2\none\ntwo"
    stderr ""
    exit_code 0
end test "first expanded field is command name and remaining fields are arguments"
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

#### Test: parameter expansion in variable assignment preceding command

Variable assignments in step 4 undergo parameter expansion before the value is
assigned.

```
begin test "parameter expansion in variable assignment preceding command"
  script
    P=world
    A="hello ${P}"
    printf "%s\n" "$A"
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "parameter expansion in variable assignment preceding command"
```

#### Test: command substitution in variable assignment preceding command

Variable assignments in step 4 undergo command substitution before the value
is assigned.

```
begin test "command substitution in variable assignment preceding command"
  script
    A="$(printf cmdsub)"
    printf "%s\n" "$A"
  expect
    stdout "cmdsub"
    stderr ""
    exit_code 0
end test "command substitution in variable assignment preceding command"
```

#### Test: quote removal in variable assignment preceding command

Variable assignments in step 4 undergo quote removal before the value is
assigned, so quotes used to preserve blanks do not remain in the value.

```
begin test "quote removal in variable assignment preceding command"
  script
    A='a b'
    printf "%s\n" "$A"
  expect
    stdout "a b"
    stderr ""
    exit_code 0
end test "quote removal in variable assignment preceding command"
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

#### Test: declaration utility assignment does not field split

When the command name is a declaration utility, words recognized as variable
assignments are expanded as assignments, so field splitting is not performed
on the assigned value.

```
begin test "declaration utility assignment does not field split"
  script
    value='aa bb'
    command export ASSIGN=$value
    printf "%s\n" "$ASSIGN"
  expect
    stdout "aa bb"
    stderr ""
    exit_code 0
end test "declaration utility assignment does not field split"
```

#### Test: declaration utility assignment does not pathname expand

When the command name is a declaration utility, assignment words are expanded
as assignments, so pathname expansion is not performed on the assigned value.

```
begin test "declaration utility assignment does not pathname expand"
  script
    touch aa bb
    command export GLOB=*
    printf "%s\n" "$GLOB"
  expect
    stdout "\*"
    stderr ""
    exit_code 0
end test "declaration utility assignment does not pathname expand"
```

#### Test: tilde expansion in plain variable assignment

Variable assignments (step 4) undergo tilde expansion before the value is
assigned. A leading `~` in the value expands to the home directory.

```
begin test "tilde expansion in plain variable assignment"
  script
    HOMEDIR=~
    case "$HOMEDIR" in /*) echo absolute;; *) echo relative;; esac
  expect
    stdout "absolute"
    stderr ""
    exit_code 0
end test "tilde expansion in plain variable assignment"
```

#### Test: declaration utility tilde expansion after colon

When a declaration utility is the command name, assignment words undergo tilde
expansion after the first `=` and after any unquoted `:`. Both tildes in a
colon-separated path are expanded.

```
begin test "declaration utility tilde expansion after colon"
  script
    command export TPATH=~:/tmp:~
    first="${TPATH%%:*}"
    last="${TPATH##*:}"
    case "$first" in /*) echo "first_expanded";; *) echo "first_literal";; esac
    case "$last" in /*) echo "last_expanded";; *) echo "last_literal";; esac
  expect
    stdout "first_expanded\nlast_expanded"
    stderr ""
    exit_code 0
end test "declaration utility tilde expansion after colon"
```

#### Test: assignment values do not undergo pathname expansion

Variable assignments (step 4) are expanded for tilde expansion, parameter
expansion, command substitution, arithmetic expansion, and quote removal only.
Pathname expansion is not listed and shall not be performed.

```
begin test "assignment values do not undergo pathname expansion"
  script
    touch tmp_glob_a tmp_glob_b
    VAR=tmp_glob_*
    printf "%s\n" "$VAR"
  expect
    stdout "tmp_glob_\*"
    stderr ""
    exit_code 0
end test "assignment values do not undergo pathname expansion"
```

#### Test: assignment values do not undergo field splitting

Variable assignments (step 4) undergo tilde expansion, parameter expansion,
command substitution, arithmetic expansion, and quote removal. Field splitting
is not listed and shall not be performed on assignment values.

```
begin test "assignment values do not undergo field splitting"
  script
    IFS=:
    VAR=$(printf "a:b:c")
    printf "%s\n" "$VAR"
  expect
    stdout "a:b:c"
    stderr ""
    exit_code 0
end test "assignment values do not undergo field splitting"
```

#### Test: assignment-looking word after regular command is an argument

For command names that are not declaration utilities, words after the command
name are subject to regular expansion, not assignment context. An assignment-
looking word is passed as an ordinary argument and does not set a variable.

```
begin test "assignment-looking word after regular command is an argument"
  script
    unset FOO
    printf "%s\n" FOO=bar
    printf "%s\n" "${FOO-unset}"
  expect
    stdout "FOO=bar\nunset"
    stderr ""
    exit_code 0
end test "assignment-looking word after regular command is an argument"
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

#### Test: prefix assignment before printf does not persist

Even if `printf` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before printf does not persist"
  script
    unset X
    X=val printf '%s\n' text
    printf '%s\n' "${X-unset}"
  expect
    stdout "text\nunset"
    stderr ""
    exit_code 0
end test "prefix assignment before printf does not persist"
```

#### Test: prefix assignment before echo does not persist

Even if `echo` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before echo does not persist"
  script
    unset X
    X=val echo text
    printf '%s\n' "${X-unset}"
  expect
    stdout "text\nunset"
    stderr ""
    exit_code 0
end test "prefix assignment before echo does not persist"
```

#### Test: prefix assignment before test does not persist

Even if `test` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before test does not persist"
  script
    unset X
    X=val test 1 -eq 1
    printf "%s\n" "${X-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "prefix assignment before test does not persist"
```

#### Test: prefix assignment before bracket does not persist

Even if `[` is implemented internally by the shell, it is a standard utility
and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before bracket does not persist"
  script
    unset X
    X=val [ 1 -eq 1 ]
    printf "%s\n" "${X-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "prefix assignment before bracket does not persist"
```

#### Test: prefix assignment before pwd does not persist

Even if `pwd` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before pwd does not persist"
  script
    unset X
    X=val pwd >/dev/null
    printf "%s\n" "${X-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "prefix assignment before pwd does not persist"
```

#### Test: prefix assignment before true does not persist

Even if `true` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before true does not persist"
  script
    unset X
    X=val true
    printf "%s\n" "${X-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "prefix assignment before true does not persist"
```

#### Test: prefix assignment before false does not persist

Even if `false` is implemented internally by the shell, it is a standard
utility and prefix assignments shall behave as if it were not a function: the
assignment does not persist in the current shell after the command completes.

```
begin test "prefix assignment before false does not persist"
  script
    unset X
    X=val false
    printf "%s\n" "${X-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "prefix assignment before false does not persist"
```

#### Test: side-effect of step 4 expansion persists in current shell

Variable assignments before a non-special-built-in command shall not affect
the current shell, except as a side-effect of the expansions in step 4. A
`${VAR:=val}` expansion inside the assignment value assigns to VAR as a side
effect, and that assignment persists.

```
begin test "side-effect of step 4 expansion persists in current shell"
  script
    unset Y
    X=${Y:=side} /usr/bin/true
    printf "%s\n" "${Y}"
  expect
    stdout "side"
    stderr ""
    exit_code 0
end test "side-effect of step 4 expansion persists in current shell"
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

#### Test: assignment before special built-in remains after utility execution

If the command name is a special built-in utility, variable assignments affect
the current execution environment before execution and remain in effect after
the utility completes.

```
begin test "assignment before special built-in remains after utility execution"
  script
    export FOO=bar >/dev/null
    printf "%s\n" "$FOO"
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "assignment before special built-in remains after utility execution"
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

#### Test: readonly assignment before regular command is an error

If a variable assignment attempts to modify a readonly variable, a variable
assignment error shall occur even when the assignment precedes a regular
command.

```
begin test "readonly assignment before regular command is an error"
  script
    readonly X=1
    X=2 env >/dev/null
    printf "%s\n" "$?"
    echo survived
  expect
    stdout "1\nsurvived"
    stderr ".+"
    exit_code 0
end test "readonly assignment before regular command is an error"
```

#### Test: readonly prefix assignment before special built-in exits shell

A variable assignment error before a special built-in utility causes the
(non-interactive) shell to exit, unlike before a regular command where the
shell continues execution (see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)).

```
begin test "readonly prefix assignment before special built-in exits shell"
  script
    (
      readonly FOO=1
      FOO=2 :
      echo "survived"
    )
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "readonly prefix assignment before special built-in exits shell"
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

#### Test: special built-in modifications to assigned variables persist

If an assigned variable is further modified by a special built-in utility, the
modification made by the utility shall persist after the command completes.

```
begin test "special built-in modifications to assigned variables persist"
  script
    OPTIND=1 getopts a opt -a >/dev/null 2>&1
    printf "%s\n" "$OPTIND"
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "special built-in modifications to assigned variables persist"
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

#### Test: last command substitution determines exit status

If there is no command name and there are multiple command substitutions, the
simple command shall complete with the exit status of the command substitution
whose exit status was obtained last.

```
begin test "last command substitution determines exit status"
  script
    foo=$(true) bar=$(false)
    printf "%s\n" "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "last command substitution determines exit status"
```

#### Test: last command substitution exit status with success last

When there are multiple command substitutions in a no-command-name simple
command, the exit status is that of the last substitution obtained. If the
last one succeeds, the exit status shall be zero.

```
begin test "last command substitution exit status with success last"
  script
    foo=$(false) bar=$(true)
    printf "%s\n" "$?"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "last command substitution exit status with success last"
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

#### Test: redirection failure with no command yields non-zero status

If a simple command has no command name and a redirection performed in the
current shell execution environment fails, the command shall fail immediately
with an exit status greater than zero and write an error message.

```
begin test "redirection failure with no command yields non-zero status"
  script
    < /definitely_missing_no_command_2_9_1
    printf "%s\n" "$?"
    echo survived
  expect
    stdout "1\nsurvived"
    stderr ".+"
    exit_code 0
end test "redirection failure with no command yields non-zero status"
```

#### Test: no-command redirections do not persist in current shell

When a simple command has no command name, redirections are performed in a
subshell environment and therefore do not leave file descriptors open in the
current shell.

```
begin test "no-command redirections do not persist in current shell"
  script
    exec 3>&-
    3>tmp_no_persist.txt
    printf 'test' >&3 || echo closed
  expect
    stdout "closed"
    stderr ".+"
    exit_code 0
end test "no-command redirections do not persist in current shell"
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

#### Test: special built-in is invoked before PATH search

If a command name matches a special built-in utility, that special built-in
shall be invoked instead of any external utility found in `PATH`.

```
begin test "special built-in is invoked before PATH search"
  script
    mkdir bin
    printf '#!/bin/sh\necho path_export\n' > bin/export
    chmod +x bin/export
    PATH="$PWD/bin:$PATH"
    export TESTVAR=1 >/dev/null
    printf "%s\n" "$TESTVAR"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "special built-in is invoked before PATH search"
```

#### Test: intrinsic utility command is invoked before PATH search

If a command name matches an intrinsic utility, that utility shall be invoked
instead of an external utility of the same name found in `PATH`.

```
begin test "intrinsic utility command is invoked before PATH search"
  script
    mkdir bin
    printf '#!/bin/sh\necho external_command\n' > bin/command
    chmod +x bin/command
    PATH="$PWD/bin:$PATH"
    command -v command
  expect
    stdout "command"
    stderr ""
    exit_code 0
end test "intrinsic utility command is invoked before PATH search"
```

#### Test: intrinsic utility type is invoked before PATH search

If a command name matches an intrinsic utility, that utility shall be invoked
before any external utility of the same name in `PATH`.

```
begin test "intrinsic utility type is invoked before PATH search"
  script
    mkdir bin
    printf '#!/bin/sh\necho external_type\n' > bin/type
    chmod +x bin/type
    PATH="$PWD/bin:$PATH"
    type command
  expect
    stdout ".+"
    stderr ""
    exit_code 0
end test "intrinsic utility type is invoked before PATH search"
```

#### Test: function name is invoked before PATH search

If a command name matches a known shell function, the function shall be
invoked during command search.

```
begin test "function name is invoked before PATH search"
  script
    myfun() { printf "%s\n" "function_called"; }
    myfun arg1
  expect
    stdout "function_called"
    stderr ""
    exit_code 0
end test "function name is invoked before PATH search"
```

#### Test: function is invoked before matching PATH utility

If a command name matches a known shell function, that function shall be
invoked before a utility with the same name found via `PATH`.

```
begin test "function is invoked before matching PATH utility"
  script
    mkdir bin
    printf '#!/bin/sh\necho path_cmd\n' > bin/foo
    chmod +x bin/foo
    foo() { echo function_cmd; }
    PATH="$PWD/bin:$PATH"
    foo
  expect
    stdout "function_cmd"
    stderr ""
    exit_code 0
end test "function is invoked before matching PATH utility"
```

#### Test: function is invoked before intrinsic utility

If a command name matches the name of a known function (step 1c), that
function shall be invoked before an intrinsic utility of the same name
(step 1d).

```
begin test "function is invoked before intrinsic utility"
  script
    cd () { echo "function_cd"; }
    cd /tmp
    unset -f cd
  expect
    stdout "function_cd"
    stderr ""
    exit_code 0
end test "function is invoked before intrinsic utility"
```

#### Test: intrinsic utility cd is invoked before matching PATH utility

The intrinsic utility `cd` is not subject to `PATH` search, so an external
utility named `cd` in `PATH` is not invoked.

```
begin test "intrinsic utility cd is invoked before matching PATH utility"
  script
    mkdir bin target
    printf '#!/bin/sh\necho external_cd\n' > bin/cd
    chmod +x bin/cd
    PATH="$PWD/bin:$PATH"
    cd target
    pwd
  expect
    stdout ".+/target"
    stderr ""
    exit_code 0
end test "intrinsic utility cd is invoked before matching PATH utility"
```

#### Test: PATH assignment causes command to be re-searched

After a utility has been found, the shell may remember its location, but if
`PATH` has been assigned it shall search again for subsequent invocations.

```
begin test "PATH assignment causes command to be re-searched"
  script
    mkdir bin1 bin2
    printf '#!/bin/sh\necho one\n' > bin1/cmdx
    chmod +x bin1/cmdx
    PATH="$PWD/bin1:$PATH" cmdx
    rm bin1/cmdx
    printf '#!/bin/sh\necho two\n' > bin2/cmdx
    chmod +x bin2/cmdx
    PATH="$PWD/bin2:$PATH" cmdx
  expect
    stdout "one\ntwo"
    stderr ""
    exit_code 0
end test "PATH assignment causes command to be re-searched"
```

#### Test: failed remembered location triggers a new PATH search

If a remembered utility location fails for a later invocation, the shell shall
repeat the search to find the new location, if any.

```
begin test "failed remembered location triggers a new PATH search"
  script
    mkdir dir1 dir2
    printf '#!/bin/sh\necho first\n' > dir1/retrycmd
    chmod +x dir1/retrycmd
    PATH="$PWD/dir1:$PWD/dir2:$PATH"
    retrycmd
    rm dir1/retrycmd
    printf '#!/bin/sh\necho second\n' > dir2/retrycmd
    chmod +x dir2/retrycmd
    retrycmd
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "failed remembered location triggers a new PATH search"
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

#### Test: exec replaces current shell execution environment

When execution is made via the `exec` special built-in, the shell shall not
create a separate utility environment; the new process image replaces the
current shell execution environment.

```
begin test "exec replaces current shell execution environment"
  script
    exec /usr/bin/printf 'exec_replaced\n'
    echo survived
  expect
    stdout "exec_replaced"
    stderr ""
    exit_code 0
end test "exec replaces current shell execution environment"
```

#### Test: exec in subshell replaces only the subshell environment

If the current shell environment is a subshell environment, `exec` shall
replace that subshell environment and the shell shall continue in the parent
environment from which the subshell was invoked.

```
begin test "exec in subshell replaces only the subshell environment"
  script
    ( exec /usr/bin/printf 'subshell_exec\n' )
    echo parent_survived
  expect
    stdout "subshell_exec\nparent_survived"
    stderr ""
    exit_code 0
end test "exec in subshell replaces only the subshell environment"
```

#### Test: PATH searched file without shebang falls back to shell execution

If a PATH search finds an executable file and executing it yields `ENOEXEC`,
the shell shall execute a command equivalent to invoking a shell on that file.

```
begin test "PATH searched file without shebang falls back to shell execution"
  script
    mkdir bin
    printf 'echo path_fallback\n' > bin/noshebang
    chmod +x bin/noshebang
    PATH="$PWD/bin:$PATH"
    noshebang
  expect
    stdout "path_fallback"
    stderr ""
    exit_code 0
end test "PATH searched file without shebang falls back to shell execution"
```

#### Test: ENOEXEC fallback via PATH passes arguments to script

When `execl()` fails with `ENOEXEC` and the shell falls back to invoking a
shell on the file found via `PATH`, any remaining arguments shall be passed
to the new shell.

```
begin test "ENOEXEC fallback via PATH passes arguments to script"
  script
    mkdir bin
    printf 'echo "$@"\n' > bin/argscript
    chmod +x bin/argscript
    PATH="$PWD/bin:$PATH"
    argscript one two three
  expect
    stdout "one two three"
    stderr ""
    exit_code 0
end test "ENOEXEC fallback via PATH passes arguments to script"
```

#### Test: ENOEXEC fallback with slash passes arguments to script

When `execl()` fails with `ENOEXEC` and the command name contains a slash,
any remaining arguments shall be passed to the new shell that executes the
file.

```
begin test "ENOEXEC fallback with slash passes arguments to script"
  script
    printf 'echo "$@"\n' > tmp_argscript
    chmod +x tmp_argscript
    ./tmp_argscript one two three
  expect
    stdout "one two three"
    stderr ""
    exit_code 0
end test "ENOEXEC fallback with slash passes arguments to script"
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
