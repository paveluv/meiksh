# Test Suite for 2.9.4 Compound Commands

This test suite covers **Section 2.9.4 Compound Commands** of the POSIX.1-2024
Shell Command Language specification (part of 2.9 Shell Commands), including
grouping commands, for loops, case constructs, if constructs, while loops,
and until loops.

## Table of contents

- [2.9.4 Compound Commands](#294-compound-commands)
- [2.9.4.1 Grouping Commands](#2941-grouping-commands)
- [2.9.4.2 The for Loop](#2942-the-for-loop)
- [2.9.4.3 Case Conditional Construct](#2943-case-conditional-construct)
- [2.9.4.4 The if Conditional Construct](#2944-the-if-conditional-construct)
- [2.9.4.5 The while Loop](#2945-the-while-loop)
- [2.9.4.6 The until Loop](#2946-the-until-loop)

## 2.9.4 Compound Commands

The shell has several programming constructs that are "compound commands", which provide control flow for commands. Each of these compound commands has a reserved word or control operator at the beginning, and a corresponding terminator reserved word or operator at the end. In addition, each can be followed by redirections on the same line as the terminator. Each redirection shall apply to all the commands within the compound command that do not explicitly override that redirection.

In the descriptions below, the exit status of some compound commands is stated in terms of the exit status of a *compound-list*. The exit status of a *compound-list* shall be the value that the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)) would have immediately after execution of the *compound-list*.

### Tests

#### Test: redirection applies to all commands in group

A redirection on a compound command applies to all commands within it
that do not explicitly override it.

```
begin test "redirection applies to all commands in group"
  script
    { echo a; echo b; } > tmp_group.txt
    cat tmp_group.txt
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "redirection applies to all commands in group"
```

## 2.9.4.1 Grouping Commands

The format for grouping commands is as follows:

- ( *compound-list* ): Execute *compound-list* in a subshell environment; see [2.13 Shell Execution Environment](#213-shell-execution-environment). Variable assignments and built-in commands that affect the environment shall not remain in effect after the list finishes. If a character sequence beginning with `"(("` would be parsed by the shell as an arithmetic expansion if preceded by a `'$'`, shells which implement an extension whereby `"((expression))"` is evaluated as an arithmetic expression may treat the `"(("` as introducing as an arithmetic evaluation instead of a grouping command. A conforming application shall ensure that it separates the two leading `'('` characters with white space to prevent the shell from performing an arithmetic evaluation.
- { *compound-list* ; }: Execute *compound-list* in the current process environment. The semicolon shown here is an example of a control operator delimiting the **}** reserved word. Other delimiters are possible, as shown in [2.10 Shell Grammar](#210-shell-grammar); a `<newline>` is frequently used.

##### Exit Status

The exit status of a grouping command shall be the exit status of *compound-list*.

### Tests

#### Test: subshell variable isolation

Variable assignments in a subshell `(...)` do not affect the parent
environment.

```
begin test "subshell variable isolation"
  script
    FOO=parent
    (FOO=child)
    echo "$FOO"
  expect
    stdout "parent"
    stderr ""
    exit_code 0
end test "subshell variable isolation"
```

#### Test: subshell false propagates exit status

The exit status of a grouping command is the exit status of the
compound-list within it.

```
begin test "subshell false propagates exit status"
  script
    ( false )
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "subshell false propagates exit status"
```

#### Test: nested subshells with spaced parens

Two leading `(` characters must be separated by whitespace to avoid being
parsed as an arithmetic expression.

```
begin test "nested subshells with spaced parens"
  script
    ( ( echo hello ) )
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "nested subshells with spaced parens"
```

## 2.9.4.2 The for Loop

The **for** loop shall execute a sequence of commands for each member in a list of *items*. The **for** loop requires that the reserved words **do** and **done** be used to delimit the sequence of commands.

The format for the **for** loop is as follows:

```
for name [ in [word ... ]]
do
    compound-list
done
```

First, the list of words following **in** shall be expanded to generate a list of items. Then, the variable *name* shall be set to each item, in turn, and the *compound-list* executed each time. If no items result from the expansion, the *compound-list* shall not be executed. Omitting:

```
in word ...
```

shall be equivalent to:

```
in "$@"
```

##### Exit Status

If there is at least one item in the list of items, the exit status of a **for** command shall be the exit status of the last *compound-list* executed. If there are no items, the exit status shall be zero.

### Tests

#### Test: for loop iterates over items

The for loop sets the variable to each item in the list and executes the
body for each one.

```
begin test "for loop iterates over items"
  script
    for i in a b c; do
      printf "%s " "$i"
    done
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "for loop iterates over items"
```

#### Test: for-in expands word list correctly

The word list in a for loop undergoes normal expansion, including quote
handling.

```
begin test "for-in expands word list correctly"
  script
    for i in "a b" c; do
        echo "$i"
    done
  expect
    stdout "a b\nc"
    stderr ""
    exit_code 0
end test "for-in expands word list correctly"
```

#### Test: for loop with empty list does not execute body

If no items result from the expansion, the compound-list is not executed.

```
begin test "for loop with empty list does not execute body"
  script
    x=1
    for i in; do
      x=2
    done
    echo "$x"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "for loop with empty list does not execute body"
```

#### Test: for loop exit status is last command

The exit status of a for loop is the exit status of the last compound-list
executed; zero if the body never runs.

```
begin test "for loop exit status is last command"
  script
    for i in 1 2; do
    false
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "for loop exit status is last command"
```

#### Test: for loop without in iterates positional params

Omitting `in word ...` is equivalent to `in "$@"`, iterating over the
positional parameters.

```
begin test "for loop without in iterates positional params"
  script
    f() { for i do printf "%s " "$i"; done; }
    f x y
  expect
    stdout "x y"
    stderr ""
    exit_code 0
end test "for loop without in iterates positional params"
```

## 2.9.4.3 Case Conditional Construct

The conditional construct **case** shall execute the *compound-list* corresponding to the first *pattern* (see [2.14 Pattern Matching Notation](#214-pattern-matching-notation)), if any are present, that is matched by the string resulting from the tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal of the given word. The reserved word **in** shall denote the beginning of the patterns to be matched. Multiple patterns with the same *compound-list* shall be delimited by the `'|'` symbol. The control operator `')'` terminates a list of patterns corresponding to a given action. The terminated pattern list and the following *compound-list* is called a **case** statement *clause*. Each **case** statement clause, with the possible exception of the last, shall be terminated with either `";;"` or `";&"`. The **case** construct terminates with the reserved word **esac** (**case** reversed).

The format for the **case** construct is as follows:

```
case word in
    [[(] pattern[ | pattern] ... ) compound-list terminator] ...
    [[(] pattern[ | pattern] ... ) compound-list]
esac
```

Where *terminator* is either `";;"` or `";&"` and is optional for the last *compound-list*.

In order from the beginning to the end of the **case** statement, each *pattern* that labels a *compound-list* shall be subjected to tilde expansion, parameter expansion, command substitution, and arithmetic expansion, and the result of these expansions shall be compared against the expansion of *word*, according to the rules described in [2.14 Pattern Matching Notation](#214-pattern-matching-notation) (which also describes the effect of quoting parts of the pattern). After the first match, no more patterns in the **case** statement shall be expanded, and the *compound-list* of the matching clause shall be executed. If the **case** statement clause is terminated by `";;"`, no further clauses shall be examined. If the **case** statement clause is terminated by `";&"`, then the *compound-list* (if any) of each subsequent clause shall be executed, in order, until either a clause terminated by `";;"` is reached and its *compound-list* (if any) executed or there are no further clauses in the **case** statement. The order of expansion and comparison of multiple *pattern*s that label a *compound-list* statement is unspecified.

##### Exit Status

The exit status of **case** shall be zero if no patterns are matched. Otherwise, the exit status shall be the exit status of the *compound-list* of the last clause to be executed.

### Tests

#### Test: case ;; termination

A `;;` terminator causes no further clauses to be examined after a match.

```
begin test "case ;; termination"
  script
    case "xyz" in
        abc) echo no ;;
        xyz) echo yes ;;
        *) echo default ;;
    esac
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "case ;; termination"
```

#### Test: case with pipe patterns and early exit

Multiple patterns for the same clause are delimited by `|`.

```
begin test "case with pipe patterns and early exit"
  script
    case "apple" in
    banana|orange) echo "no" ;;
    apple|pear) echo "yes" ;;
    *) echo "default" ;;
    esac
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "case with pipe patterns and early exit"
```

#### Test: case exit status from matched clause

The exit status of `case` is the exit status of the compound-list of the
matched clause.

```
begin test "case exit status from matched clause"
  script
    case "xyz" in
    abc) false ;;
    xyz) false ;;
    esac
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "case exit status from matched clause"
```

#### Test: case exit status zero when no match

If no patterns match, the exit status of `case` is zero.

```
begin test "case exit status zero when no match"
  script
    case "xyz" in
    abc) false ;;
    esac
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "case exit status zero when no match"
```

#### Test: case ;& fallthrough

A `;&` terminator causes subsequent clauses to be executed without
pattern matching.

```
begin test "case ;& fallthrough"
  script
    case a in a) echo first ;& b) echo second ;; c) echo third ;; esac
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "case ;& fallthrough"
```

#### Test: case ;& multi-fallthrough until ;;

A `;&` terminator causes fallthrough to continue until a `;;` or end of
case is reached.

```
begin test "case ;& multi-fallthrough until ;;"
  script
    case x in x) echo one ;& y) echo two ;& z) echo three ;; w) echo four ;; esac
  expect
    stdout "one\ntwo\nthree"
    stderr ""
    exit_code 0
end test "case ;& multi-fallthrough until ;;"
```

#### Test: pattern expansion in case labels

Case patterns undergo parameter expansion before matching.

```
begin test "pattern expansion in case labels"
  script
    X=hello
    case "hello" in
        $X) echo matched ;;
        *) echo nomatch ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "pattern expansion in case labels"
```

#### Test: pattern with arithmetic expansion in case label

Case patterns undergo arithmetic expansion before matching.

```
begin test "pattern with arithmetic expansion in case label"
  script
    case "6" in
        $((2+4))) echo six ;;
        *) echo other ;;
    esac
  expect
    stdout "six"
    stderr ""
    exit_code 0
end test "pattern with arithmetic expansion in case label"
```

#### Test: pattern with command substitution in case label

Case patterns undergo command substitution before matching.

```
begin test "pattern with command substitution in case label"
  script
    case "3" in
        $(echo 3)) echo three ;;
        *) echo other ;;
    esac
  expect
    stdout "three"
    stderr ""
    exit_code 0
end test "pattern with command substitution in case label"
```

#### Test: pattern matching with star

Case patterns support `*` as a glob-style match-anything pattern.

```
begin test "pattern matching with star"
  script
    case hello in h*) echo yes ;; *) echo no ;; esac
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "pattern matching with star"
```

## 2.9.4.4 The if Conditional Construct

The **if** command shall execute a *compound-list* and use its exit status to determine whether to execute another *compound-list*.

The format for the **if** construct is as follows:

```
if compound-list
then
    compound-list
[elif compound-list
then
    compound-list] ...
[else
    compound-list]
fi
```

The **if** *compound-list* shall be executed; if its exit status is zero, the **then** *compound-list* shall be executed and the command shall complete. Otherwise, each **elif** *compound-list* shall be executed, in turn, and if its exit status is zero, the **then** *compound-list* shall be executed and the command shall complete. Otherwise, the **else** *compound-list* shall be executed.

##### Exit Status

The exit status of the **if** command shall be the exit status of the **then** or **else** *compound-list* that was executed, or zero, if none was executed.

**Note:** Although the exit status of the **if** or **elif** *compound-list* is ignored when determining the exit status of the **if** command, it is available through the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)) during execution of the next **then**, **elif**, or **else** *compound-list* (if any is executed) in the normal way.

### Tests

#### Test: if true branch

When the if compound-list exits zero, the then clause is executed.

```
begin test "if true branch"
  script
    if true; then
      echo "if"
    elif false; then
      echo "elif"
    else
      echo "else"
    fi
  expect
    stdout "if"
    stderr ""
    exit_code 0
end test "if true branch"
```

#### Test: elif true branch

When the if compound-list exits non-zero but an elif exits zero, that
elif's then clause is executed.

```
begin test "elif true branch"
  script
    if false; then
      echo "if"
    elif true; then
      echo "elif"
    else
      echo "else"
    fi
  expect
    stdout "elif"
    stderr ""
    exit_code 0
end test "elif true branch"
```

#### Test: if exit status from then clause

The exit status of the if command is the exit status of the then or else
compound-list that was executed.

```
begin test "if exit status from then clause"
  script
    if true; then
    false
    fi
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "if exit status from then clause"
```

## 2.9.4.5 The while Loop

The **while** loop shall continuously execute one *compound-list* as long as another *compound-list* has a zero exit status.

The format of the **while** loop is as follows:

```
while compound-list-1
do
    compound-list-2
done
```

The *compound-list-1* shall be executed, and if it has a non-zero exit status, the **while** command shall complete. Otherwise, the *compound-list-2* shall be executed, and the process shall repeat.

##### Exit Status

The exit status of the **while** loop shall be the exit status of the last *compound-list-2* executed, or zero if none was executed.

**Note:** Since the exit status of *compound-list-1* is ignored when determining the exit status of the **while** command, it is not possible to obtain the status of the command that caused the loop to exit, other than via the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)) during execution of *compound-list-1*, for example:

```
while some_command; st=$?; false; do ...
```

The exit status of *compound-list-1* is available through the special parameter `'?'` during execution of *compound-list-2*, but is known to be zero at that point anyway.

### Tests

#### Test: while loop iterates correctly

The while loop repeats the body as long as the condition has a zero exit
status.

```
begin test "while loop iterates correctly"
  script
    x=0
    while [ $x -lt 3 ]; do
      x=$((x+1))
      printf "%s " "$x"
    done
  expect
    stdout "1 2 3"
    stderr ""
    exit_code 0
end test "while loop iterates correctly"
```

#### Test: while loop with false condition does not execute

When the condition is immediately non-zero, the body is never executed.

```
begin test "while loop with false condition does not execute"
  script
    x=0
    while false; do
      x=$((x+1))
    done
    echo "$x"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "while loop with false condition does not execute"
```

#### Test: while loop exit status from last iteration

The exit status of the while loop is the exit status of the last
compound-list-2 executed.

```
begin test "while loop exit status from last iteration"
  script
    counter=0
    while [ "$counter" -lt 1 ]; do
    counter=$((counter + 1))
    false
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "while loop exit status from last iteration"
```

#### Test: while loop exit status zero when body never executes

If the body is never executed, the exit status of the while loop is zero.

```
begin test "while loop exit status zero when body never executes"
  script
    while false; do
    true
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "while loop exit status zero when body never executes"
```

## 2.9.4.6 The until Loop

The **until** loop shall continuously execute one *compound-list* as long as another *compound-list* has a non-zero exit status.

The format of the **until** loop is as follows:

```
until compound-list-1
do
    compound-list-2
done
```

The *compound-list-1* shall be executed, and if it has a zero exit status, the **until** command completes. Otherwise, the *compound-list-2* shall be executed, and the process repeats.

##### Exit Status

The exit status of the **until** loop shall be the exit status of the last *compound-list-2* executed, or zero if none was executed.

**Note:** Although the exit status of *compound-list-1* is ignored when determining the exit status of the **until** command, it is available through the special parameter `'?'` (see [2.5.2 Special Parameters](#252-special-parameters)) during execution of *compound-list-2* in the normal way.

### Tests

#### Test: until loop iterates correctly

The until loop repeats the body as long as the condition has a non-zero
exit status.

```
begin test "until loop iterates correctly"
  script
    x=0
    until [ $x -eq 3 ]; do
      x=$((x+1))
      printf "%s " "$x"
    done
  expect
    stdout "1 2 3"
    stderr ""
    exit_code 0
end test "until loop iterates correctly"
```

#### Test: until loop with true condition does not execute

When the condition is immediately zero, the body is never executed.

```
begin test "until loop with true condition does not execute"
  script
    x=0
    until true; do
      x=$((x+1))
    done
    echo "$x"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "until loop with true condition does not execute"
```
