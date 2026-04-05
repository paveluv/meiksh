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

The function is named *fname*; the application shall ensure that it is a name (see XBD [*3.216 Name*](docs/posix/md/basedefs/V1_chap03.md#3216-name)) and that it is not the name of a special built-in utility. An implementation may allow other characters in a function name as an extension. The implementation shall maintain separate name spaces for functions and variables.

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

#### Test: command substitution in function body is deferred until call

No expansions in the function body are performed when the function is
declared. Command substitution in the body runs only when the function is
called.

```
begin test "command substitution in function body is deferred until call"
  script
    rm -f tmp_func_touch.txt
    myfunc() {
        out=$(touch tmp_func_touch.txt; printf made)
        echo "$out"
    }
    if test -f tmp_func_touch.txt; then echo early; else echo not_yet; fi
    myfunc
    if test -f tmp_func_touch.txt; then echo after; else echo missing; fi
  expect
    stdout "not_yet\nmade\nafter"
    stderr ""
    exit_code 0
end test "command substitution in function body is deferred until call"
```

#### Test: expansions performed fresh each time function is called

The standard requires that all expansions shall be performed as normal
"each time" the function is called. Calling a function twice with
different variable values between calls shall produce different results,
confirming that expansions are re-evaluated on every invocation.

```
begin test "expansions performed fresh each time function is called"
  script
    myfunc() { echo "$VAR"; }
    VAR=first
    myfunc
    VAR=second
    myfunc
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "expansions performed fresh each time function is called"
```

#### Test: assignments in function body are performed during execution

Variable assignments within the compound-command are performed when the
function executes, not when it is defined.

```
begin test "assignments in function body are performed during execution"
  script
    VALUE=outer
    myfunc() { VALUE=inner; }
    printf "before:%s\n" "$VALUE"
    myfunc
    printf "after:%s\n" "$VALUE"
  expect
    stdout "before:outer\nafter:inner"
    stderr ""
    exit_code 0
end test "assignments in function body are performed during execution"
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

#### Test: function temporarily changes sharp to operand count

During function execution, the special parameter `#` reflects the number of
operands passed to the function, and when the function completes it is restored
to its previous value.

```
begin test "function temporarily changes sharp to operand count"
  script
    set -- outer1 outer2 outer3
    myfunc() {
        printf "inside:%s\n" "$#"
    }
    printf "before:%s\n" "$#"
    myfunc a b
    printf "after:%s\n" "$#"
  expect
    stdout "before:3\ninside:2\nafter:3"
    stderr ""
    exit_code 0
end test "function temporarily changes sharp to operand count"
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

#### Test: positional parameters restored even after set in function

When a function uses `set --` to modify its own positional parameters, the
caller's positional parameters and `#` shall still be restored when the
function completes.

```
begin test "positional parameters restored even after set in function"
  script
    set -- a b c
    myfunc() {
      set -- x y
      echo "$@"
    }
    myfunc arg1
    echo "$@"
  expect
    stdout "x y\na b c"
    stderr ""
    exit_code 0
end test "positional parameters restored even after set in function"
```

#### Test: function with no arguments has empty positional parameters

When a function is called with zero operands, `$#` shall be 0 and the
positional parameters shall be empty during execution. The caller's
parameters shall be restored afterward.

```
begin test "function with no arguments has empty positional parameters"
  script
    set -- a b c
    myfunc() {
      printf "%s:%s\n" "$#" "$*"
    }
    myfunc
    echo "$# $1"
  expect
    stdout "0:\n3 a"
    stderr ""
    exit_code 0
end test "function with no arguments has empty positional parameters"
```

#### Test: nested function calls restore parameters independently

When a function calls another function, each level's positional parameters
shall be independently saved and restored. After the inner function returns,
the outer function's parameters are restored; after the outer returns, the
caller's original parameters are restored.

```
begin test "nested function calls restore parameters independently"
  script
    inner() { printf "inner:%s:%s\n" "$#" "$*"; }
    outer() { printf "outer_before:%s:%s\n" "$#" "$*"; inner x y z; printf "outer_after:%s:%s\n" "$#" "$*"; }
    set -- a b
    outer p q r
    printf "main:%s:%s\n" "$#" "$*"
  expect
    stdout "outer_before:3:p q r\ninner:3:x y z\nouter_after:3:p q r\nmain:2:a b"
    stderr ""
    exit_code 0
end test "nested function calls restore parameters independently"
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

#### Test: return resumes with next command after function call

If `return` is executed in the function body, the function completes and
execution resumes with the command following the function invocation.

```
begin test "return resumes with next command after function call"
  script
    myfunc() {
        return 7
        echo "bad"
    }
    myfunc
    echo "after_call"
  expect
    stdout "after_call"
    stderr ""
    exit_code 0
end test "return resumes with next command after function call"
```

#### Test: return without operand preserves last command exit status

When `return` is executed without an operand inside a function, the
function's exit status shall be the exit status of the last command
executed before `return` (since `return` without *n* is equivalent to
`return $?`).

```
begin test "return without operand preserves last command exit status"
  script
    myfunc() {
        false
        return
    }
    myfunc
    echo "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "return without operand preserves last command exit status"
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

#### Test: function definition resets exit status to zero

A successful function definition shall have exit status zero, even if the
previous command had a non-zero exit status.

```
begin test "function definition resets exit status to zero"
  script
    false
    myfunc() { echo hello; }
    echo "$?"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "function definition resets exit status to zero"
```

#### Test: function body can be a subshell compound command

The compound-command in a function definition can be any compound command from
2.9.4, not just a brace group. A subshell `( ... )` is a valid function body.

```
begin test "function body can be a subshell compound command"
  script
    myfunc() (
      echo "$1"
    )
    myfunc hello
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "function body can be a subshell compound command"
```

#### Test: function body can be a for loop compound command

The compound-command in a function definition can be any compound command from
2.9.4, not just a brace group. A `for` loop is a valid function body.

```
begin test "function body can be a for loop compound command"
  script
    myfunc() for i in 1 2; do echo $i; done
    myfunc
  expect
    stdout "1\n2"
    stderr ""
    exit_code 0
end test "function body can be a for loop compound command"
```

#### Test: function body can be an if compound command

The compound-command in a function definition can be any compound command from
2.9.4, not just a brace group. An `if` construct is a valid function body.

```
begin test "function body can be an if compound command"
  script
    myfunc() if true; then echo "is_if"; fi
    myfunc
  expect
    stdout "is_if"
    stderr ""
    exit_code 0
end test "function body can be an if compound command"
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

#### Test: function call with successful redirection

Redirections on a function invocation are applied during execution of the
function. A successful redirection causes the function's output to go to
the specified target.

```
begin test "function call with successful redirection"
  script
    myfunc() { echo "redirected"; }
    myfunc > tmp_func_redir.txt
    cat tmp_func_redir.txt
  expect
    stdout "redirected"
    stderr ""
    exit_code 0
end test "function call with successful redirection"
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

#### Test: assignment error in function body exits non-interactive shell

The standard references 2.8.1 for consequences of assignment failures
during function execution. Per the 2.8.1 table, a variable assignment
error (such as writing to a readonly variable) in a non-interactive shell
shall cause the shell to exit — this applies even inside a function body.

```
begin test "assignment error in function body exits non-interactive shell"
  script
    readonly RO=fixed
    myfunc() { RO=changed; }
    myfunc
    echo "should not reach"
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "assignment error in function body exits non-interactive shell"
```

#### Test: function definition redirection is deferred until function call

Trailing redirections in a function definition are neither expanded nor
performed when the function is declared. They are expanded and applied when
the function is called.

```
begin test "function definition redirection is deferred until function call"
  script
    suffix=before
    myfunc() { echo body; } > "tmp_$suffix.txt"
    suffix=after
    printf "before:"
    if test -f tmp_before.txt; then cat tmp_before.txt; else echo missing; fi
    myfunc
    printf "after:"
    if test -f tmp_after.txt; then cat tmp_after.txt; else echo missing; fi
  expect
    stdout "before:missing\nafter:body"
    stderr ""
    exit_code 0
end test "function definition redirection is deferred until function call"
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

#### Test: function shadows external command of same name

When a function is defined with the same name as an external command, the
function shall be executed (per the command search and execution order in
2.9.1.4, which checks functions before PATH).

```
begin test "function shadows external command of same name"
  script
    echo() { printf "func:%s\n" "$1"; }
    echo hello
    unset -f echo
  expect
    stdout "func:hello"
    stderr ""
    exit_code 0
end test "function shadows external command of same name"
```

#### Test: failed function definition returns non-zero status

If a function definition is not declared successfully, its exit status shall
be greater than zero.

```
begin test "failed function definition returns non-zero status"
  script
    /usr/bin/bash --posix -c 'myfunc() { ' >/dev/null 2>&1
    echo $?
  expect
    stdout "[1-9][0-9]*"
    stderr ""
    exit_code 0
end test "failed function definition returns non-zero status"
```
