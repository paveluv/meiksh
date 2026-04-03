# Test Suite for 2.9.5 Function Definition Command

This test suite covers **Section 2.9.5 Function Definition Command** of the
POSIX.1-2024 Shell Command Language specification (part of 2.9 Shell Commands).

## Table of contents

- [2.9.5 Function Definition Command](#295-function-definition-command)

## 2.9.5 Function Definition Command

A function is a user-defined name that is used as a simple command to call a compound command with new positional parameters. A function is defined with a "function definition command".

The format of a function definition command is as follows:

```
fname ( ) compound-command [io-redirect ...]
```

The function is named *fname*; the application shall ensure that it is a name (see XBD [*3.216 Name*](../basedefs/V1_chap03.md#3216-name)) and that it is not the name of a special built-in utility. An implementation may allow other characters in a function name as an extension. The implementation shall maintain separate name spaces for functions and variables.

The argument *compound-command* represents a compound command, as described in [2.9.4 Compound Commands](#294-compound-commands).

When the function is declared, none of the expansions in [2.6 Word Expansions](#26-word-expansions) shall be performed on the text in *compound-command* or *io-redirect*; all expansions shall be performed as normal each time the function is called. Similarly, the optional *io-redirect* redirections and any variable assignments within *compound-command* shall be performed during the execution of the function itself, not the function definition. See [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) for the consequences of failures of these operations on interactive and non-interactive shells.

When a function is executed, it shall have the syntax-error properties described for special built-in utilities in the first item in the enumerated list at the beginning of [2.15 Special Built-In Utilities](#215-special-built-in-utilities).

The *compound-command* shall be executed whenever the function name is specified as the name of a simple command (see [2.9.1.4 Command Search and Execution](#2914-command-search-and-execution)). The operands to the command temporarily shall become the positional parameters during the execution of the *compound-command*; the special parameter `'#'` also shall be changed to reflect the number of operands. The special parameter 0 shall be unchanged. When the function completes, the values of the positional parameters and the special parameter `'#'` shall be restored to the values they had before the function was executed. If the special built-in [*return*](#return) (see [return](#tag_19_25)) is executed in the *compound-command*, the function completes and execution shall resume with the next command after the function call.

##### Exit Status

The exit status of a function definition shall be zero if the function was declared successfully; otherwise, it shall be greater than zero. The exit status of a function invocation shall be the exit status of the last command executed by the function.

### Tests

#### Test: define and invoke a function

A function defined with `fname() { ... }` can be called by name as a
simple command.

```
begin test "define and invoke a function"
  script
    myfunc() { echo "func executing"; }
    myfunc
  expect
    stdout "func executing"
    stderr ""
    exit_code 0
end test "define and invoke a function"
```

#### Test: variable and function with same name do not conflict

The shell maintains separate namespaces for functions and variables.

```
begin test "variable and function with same name do not conflict"
  script
    foo=var_value
    foo() { echo "func_value"; }
    echo "$foo"
    foo
  expect
    stdout "var_value\nfunc_value"
    stderr ""
    exit_code 0
end test "variable and function with same name do not conflict"
```

#### Test: variable inside function not expanded at declaration time

Expansions in the function body are deferred until the function is called,
not performed at declaration time.

```
begin test "variable inside function not expanded at declaration time"
  script
    my_var="declared"
    myfunc() {
        echo "$my_var"
    }
    my_var="called"
    myfunc
  expect
    stdout "called"
    stderr ""
    exit_code 0
end test "variable inside function not expanded at declaration time"
```

#### Test: positional parameters pass to function and restore after

Function arguments become positional parameters during execution and are
restored when the function returns.

```
begin test "positional parameters pass to function and restore after"
  script
    set -- parent_arg
    myfunc() {
        printf "%s " "$1"
        printf "%s " "$#"
    }
    printf "%s " "$1"
    printf "%s " "$#"
    myfunc "child_arg"
    printf "%s " "$1"
    printf "%s" "$#"
  expect
    stdout "parent_arg 1 child_arg 1 parent_arg 1"
    stderr ""
    exit_code 0
end test "positional parameters pass to function and restore after"
```

#### Test: function does not change dollar zero

The special parameter `$0` is not changed when a function is executed.

```
begin test "function does not change dollar zero"
  script
    outer="$0"
    myfunc() {
        echo "$0"
    }
    inner=$(myfunc)
    [ "$outer" = "$inner" ] && echo "same" || echo "different"
  expect
    stdout "same"
    stderr ""
    exit_code 0
end test "function does not change dollar zero"
```

#### Test: return from function sets exit status and skips remaining

The `return` special built-in completes the function and sets its exit
status; commands after `return` are not executed.

```
begin test "return from function sets exit status and skips remaining"
  script
    myfunc() {
        return 42
        echo "should not execute"
    }
    myfunc
    echo "$?"
  expect
    stdout "42"
    stderr ""
    exit_code 0
end test "return from function sets exit status and skips remaining"
```

#### Test: successful function definition exits zero

A function definition that succeeds has an exit status of zero.

```
begin test "successful function definition exits zero"
  script
    myfunc() { echo hello; }
    echo $?
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "successful function definition exits zero"
```

#### Test: function exit status is last command exit status

The exit status of a function invocation is the exit status of the last
command executed within the function body.

```
begin test "function exit status is last command exit status"
  script
    myfunc() {
        true
        false
    }
    myfunc
    echo $?
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "function exit status is last command exit status"
```

#### Test: redirection error on function call yields non-zero exit

Redirections on a function call are performed during execution; a redirection
error causes a non-zero exit status.

```
begin test "redirection error on function call yields non-zero exit"
  script
    myfunc() {
        echo "executing"
    }
    myfunc > /invalid/dir/does/not/exist
    exit $?
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "redirection error on function call yields non-zero exit"
```

#### Test: function with syntax error in body causes non-interactive shell to exit

Functions have the same syntax-error properties as special built-in
utilities; a syntax error in a function body causes a non-interactive
shell to exit.

```
begin test "function with syntax error in body causes non-interactive shell to exit"
  script
    myfunc() {
        eval 'if'
    }
    myfunc 2>/dev/null
    echo "should not reach"
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "function with syntax error in body causes non-interactive shell to exit"
```
