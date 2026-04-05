# Test Suite for 2.13 Shell Execution Environment

This test suite covers **Section 2.13 Shell Execution Environment** of the
POSIX.1-2024 Shell Command Language specification. It tests the execution
environment model, including utility invocation in a separate environment,
subshell creation and isolation, and current-environment execution.

## Table of contents

- [2.13 Shell Execution Environment](#213-shell-execution-environment)

## 2.13 Shell Execution Environment

A shell execution environment consists of the following:

- Open files inherited upon invocation of the shell, plus open files controlled by [*exec*](#exec)
- Working directory as set by [*cd*](docs/posix/md/utilities/cd.md)
- File creation mask set by [*umask*](docs/posix/md/utilities/umask.md)
- File size limit as set by [*ulimit*](docs/posix/md/utilities/ulimit.md)
- Current traps set by [*trap*](#trap)
- Shell parameters that are set by variable assignment (see the [set](#tag_19_26) special built-in) or from the System Interfaces volume of POSIX.1-2024 environment inherited by the shell when it begins (see the [export](#tag_19_23) special built-in)
- Shell functions; see [2.9.5 Function Definition Command](#295-function-definition-command)
- Options turned on at invocation or by [*set*](#set)
- Background jobs and their associated process IDs, and process IDs of child processes created to execute asynchronous AND-OR lists while job control is disabled; together these process IDs constitute the process IDs "known to this shell environment". If the implementation supports non-job-control background jobs, the list of known process IDs and the list of background jobs may form a single list even though this standard describes them as being updated separately. See [2.9.3.1 Asynchronous AND-OR Lists](#2931-asynchronous-and-or-lists)
- Shell aliases; see [2.3.1 Alias Substitution](#231-alias-substitution)

Utilities other than the special built-ins (see [2.15 Special Built-In Utilities](#215-special-built-in-utilities)) shall be invoked in a separate environment that consists of the following. The initial value of these objects shall be the same as that for the parent shell, except as noted below.

- Open files inherited on invocation of the shell, open files controlled by the [*exec*](#exec) special built-in plus any modifications, and additions specified by any redirections to the utility
- Current working directory
- File creation mask
- If the utility is a shell script, traps caught by the shell shall be set to the default values and traps ignored by the shell shall be set to be ignored by the utility; if the utility is not a shell script, the trap actions (default or ignore) shall be mapped into the appropriate signal handling actions for the utility
- Variables with the [*export*](#export) attribute, along with those explicitly exported for the duration of the command, shall be passed to the utility environment variables
- It is unspecified whether environment variables that were passed to the invoking shell when it was invoked itself, but were not used to initialize shell variables (see [2.5.3 Shell Variables](#253-shell-variables)) because they had invalid names, are included in the invoked utility's environment.

The environment of the shell process shall not be changed by the utility unless explicitly specified by the utility description (for example, [*cd*](docs/posix/md/utilities/cd.md) and [*umask*](docs/posix/md/utilities/umask.md)).

A subshell environment shall be created as a duplicate of the shell environment, except that:

- Unless specified otherwise (see [trap](#tag_19_29)), traps that are not being ignored shall be set to the default action.
- If the shell is interactive, the subshell shall behave as a non-interactive shell in all respects except:
    - The expansion of the special parameter `'-'` may continue to indicate that it is interactive.
    - The [*set*](#set) **-n** option may be ignored.

Changes made to the subshell environment shall not affect the shell environment. Command substitution, commands that are grouped with parentheses, and asynchronous AND-OR lists shall be executed in a subshell environment. Additionally, each command of a multi-command pipeline is in a subshell environment; as an extension, however, any or all commands in a pipeline may be executed in the current environment. Except where otherwise stated, all other commands shall be executed in the current shell environment.

### Tests

#### Test: exported variable inherited by child

Utilities are invoked in a separate environment whose initial values match
the parent shell. Exported variables are visible to child processes.

```
begin test "exported variable inherited by child"
  script
    MYVAR=hello
    export MYVAR
    $SHELL -c 'echo $MYVAR'
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "exported variable inherited by child"
```

#### Test: non-exported variable not visible in child

Only variables with the export attribute are passed to the utility's
environment. Non-exported variables are not inherited.

```
begin test "non-exported variable not visible in child"
  script
    MYVAR=secret
    $SHELL -c 'echo ${MYVAR:-empty}'
  expect
    stdout "empty"
    stderr ""
    exit_code 0
end test "non-exported variable not visible in child"
```

#### Test: child shell script traps reset to default

When a utility is a shell script, traps caught by the parent shell are
reset to default values in the child. Sending the signal to the child
kills it (default action) rather than running the parent's trap handler.

```
begin test "child shell script traps reset to default"
  script
    s=$TMPDIR/_trap_child_$$.sh
    printf 'kill -USR1 $$; echo survived\n' > "$s"
    chmod +x "$s"
    trap 'echo parent_caught' USR1
    $SHELL "$s" 2>/dev/null
    rc=$?
    rm -f "$s"
    echo "child_rc=$rc"
  expect
    stdout "child_rc=(12[89]|1[3-9][0-9]|2[0-5][0-9])"
    stderr ""
    exit_code 0
end test "child shell script traps reset to default"
```

#### Test: child inherits ignored traps

When a utility is a shell script, traps that are ignored by the parent
shell remain ignored in the child.

```
begin test "child inherits ignored traps"
  script
    s=$TMPDIR/_trap_ign_$$.sh
    printf 'trap -p INT\n' > "$s"
    chmod +x "$s"
    trap '' INT
    $SHELL "$s"
    rm -f "$s"
  expect
    stdout "trap -- '' INT"
    stderr ""
    exit_code 0
end test "child inherits ignored traps"
```

#### Test: external utility does not change parent environment

The shell's environment is not changed by a utility unless the utility
description explicitly says so (e.g. `cd`, `umask`).

```
begin test "external utility does not change parent environment"
  script
    mkdir childdir
    parent="$PWD"
    $SHELL -c 'cd childdir; pwd'
    printf 'parent:%s\n' "$PWD"
  expect
    stdout ".+/childdir\nparent:.+"
    stderr ""
    exit_code 0
end test "external utility does not change parent environment"
```

#### Test: subshell traps reset to default

A subshell duplicates the shell environment, but traps not being ignored
are reset to their default action.

```
begin test "subshell traps reset to default"
  script
    trap "echo parent_trap" USR1
    (trap -p USR1; echo end)
  expect
    stdout "end"
    stderr ""
    exit_code 0
end test "subshell traps reset to default"
```

#### Test: subshell inherits ignored traps

A trap set to ignore (SIG_IGN) is preserved in the subshell — it is
not reset to default.

```
begin test "subshell inherits ignored traps"
  script
    trap '' USR1
    (trap -p USR1)
  expect
    stdout "trap -- '' USR1"
    stderr ""
    exit_code 0
end test "subshell inherits ignored traps"
```

#### Test: parent variable visible in subshell but changes do not propagate

A subshell is a duplicate of the parent environment, so parent variables
are visible. However, changes made in the subshell do not affect the
parent.

```
begin test "parent variable visible in subshell but changes do not propagate"
  script
    var="parent"
    (echo "$var"; var="child"; echo "$var")
    echo "$var"
  expect
    stdout "parent\nchild\nparent"
    stderr ""
    exit_code 0
end test "parent variable visible in subshell but changes do not propagate"
```

#### Test: command substitution runs in subshell

Command substitution is executed in a subshell environment. Variable
changes inside `$(...)` do not propagate to the parent.

```
begin test "command substitution runs in subshell"
  script
    var="parent"
    output=$(var="child"; echo "$var")
    echo "$output $var"
  expect
    stdout "child parent"
    stderr ""
    exit_code 0
end test "command substitution runs in subshell"
```

#### Test: command substitution resets non-ignored traps to default

Command substitution is executed in a subshell environment, so traps that are
not being ignored shall be reset to the default action there. Bash currently
does not comply and still shows the inherited caught trap.

```
begin test "command substitution resets non-ignored traps to default"
  script
    trap "echo parent_trap" USR1
    out=$(trap -p USR1; echo end)
    printf '%s\n' "$out"
  expect
    stdout "end"
    stderr ""
    exit_code 0
end test "command substitution resets non-ignored traps to default"
```

#### Test: parenthesized group runs in subshell

Commands grouped with parentheses execute in a subshell. Variable
changes inside `(...)` do not propagate.

```
begin test "parenthesized group runs in subshell"
  script
    var="before"
    (var="inside")
    echo "$var"
  expect
    stdout "before"
    stderr ""
    exit_code 0
end test "parenthesized group runs in subshell"
```

#### Test: asynchronous list runs in subshell

An asynchronous AND-OR list is executed in a subshell environment.
Variable changes do not propagate to the parent.

```
begin test "asynchronous list runs in subshell"
  script
    var="parent"
    var="child" &
    wait
    echo "$var"
  expect
    stdout "parent"
    stderr ""
    exit_code 0
end test "asynchronous list runs in subshell"
```

#### Test: group commands execute in current environment

Brace groups (`{ ... }`) execute in the current shell environment, not
a subshell. Variable changes persist after the group.

```
begin test "group commands execute in current environment"
  script
    var="parent"
    { var="child"; echo "$var"; }
    echo "$var"
  expect
    stdout "child\nchild"
    stderr ""
    exit_code 0
end test "group commands execute in current environment"
```

#### Test: if construct executes in current environment

Compound commands like `if` execute in the current shell environment.
Variable changes inside the body persist.

```
begin test "if construct executes in current environment"
  script
    var="parent"
    if true; then
      var="child"
    fi
    echo "$var"
  expect
    stdout "child"
    stderr ""
    exit_code 0
end test "if construct executes in current environment"
```

#### Test: while loop executes in current environment

`while` loops execute in the current shell environment. Variable
changes inside the loop body persist.

```
begin test "while loop executes in current environment"
  script
    var="before"
    i=0
    while [ "$i" -lt 2 ]; do
      var="iter$i"
      i=$((i + 1))
    done
    echo "$var"
  expect
    stdout "iter1"
    stderr ""
    exit_code 0
end test "while loop executes in current environment"
```

#### Test: child inherits working directory

The separate environment for a utility starts with the same working
directory as the parent.

```
begin test "child inherits working directory"
  script
    cd /tmp
    $SHELL -c 'pwd'
  expect
    stdout "/tmp"
    stderr ""
    exit_code 0
end test "child inherits working directory"
```

#### Test: child inherits file creation mask

The separate environment for a utility starts with the same file
creation mask (umask) as the parent.

```
begin test "child inherits file creation mask"
  script
    umask 0027
    result=$($SHELL -c 'umask')
    echo "$result"
  expect
    stdout "0027"
    stderr ""
    exit_code 0
end test "child inherits file creation mask"
```

#### Test: export for duration of command

Variables explicitly exported for the duration of a command are passed
to the utility's environment.

```
begin test "export for duration of command"
  script
    TMPVAR=value $SHELL -c 'echo $TMPVAR'
  expect
    stdout "value"
    stderr ""
    exit_code 0
end test "export for duration of command"
```

#### Test: subshell inherits functions

A subshell is a duplicate of the shell environment, including shell
functions.

```
begin test "subshell inherits functions"
  script
    myfunc() { echo "from_func"; }
    (myfunc)
  expect
    stdout "from_func"
    stderr ""
    exit_code 0
end test "subshell inherits functions"
```

#### Test: subshell inherits aliases

A subshell is a duplicate of the shell environment, including shell aliases.

```
begin test "subshell inherits aliases"
  script
    alias hi='echo hello'
    (hi)
    printf 'parent:'; hi
  expect
    stdout "hello\nparent:hello"
    stderr ""
    exit_code 0
end test "subshell inherits aliases"
```

#### Test: subshell inherits shell options

A subshell duplicates the parent's shell options. With `set -u`, expanding an
unset variable in the subshell should fail.

```
begin test "subshell inherits shell options"
  script
    set -u
    ( : "$UNSET_VAR" ) 2>/dev/null
    printf 'rc=%s\n' "$?"
  expect
    stdout "rc=1"
    stderr ""
    exit_code 0
end test "subshell inherits shell options"
```

#### Test: child utility inherits exec-open file descriptor

The separate environment for a utility starts with the shell's open files,
including file descriptors opened by the `exec` special built-in.

```
begin test "child utility inherits exec-open file descriptor"
  script
    exec 3>tmp_fd3.txt
    $SHELL -c 'echo child >&3'
    exec 3>&-
    cat tmp_fd3.txt
  expect
    stdout "child"
    stderr ""
    exit_code 0
end test "child utility inherits exec-open file descriptor"
```

#### Test: utility redirection additions do not affect parent open files

The utility's separate environment includes additions specified by redirections
to the utility itself, but those redirections do not modify the parent shell's
open files.

```
begin test "utility redirection additions do not affect parent open files"
  script
    exec 3>parent_fd3.txt
    $SHELL -c 'echo child >&3' 3>child_fd3.txt
    echo parent >&3
    exec 3>&-
    printf 'parent:'; cat parent_fd3.txt
    printf 'child:'; cat child_fd3.txt
  expect
    stdout "parent:parent\nchild:child"
    stderr ""
    exit_code 0
end test "utility redirection additions do not affect parent open files"
```
