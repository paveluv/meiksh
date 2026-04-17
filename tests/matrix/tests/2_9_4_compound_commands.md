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

#### Test: inner redirection overrides compound command redirection

A redirection on a compound command applies to enclosed commands unless a
command explicitly overrides that redirection.

```
begin test "inner redirection overrides compound command redirection"
  script
    { echo outer; echo inner > tmp_inner.txt; } > tmp_outer.txt
    printf '%s\n' "$(cat tmp_outer.txt)"
    printf '%s\n' "$(cat tmp_inner.txt)"
  expect
    stdout "outer\ninner"
    stderr ""
    exit_code 0
end test "inner redirection overrides compound command redirection"
```

#### Test: for loop with redirection on terminator

Redirections on compound command terminators apply to all commands within.
This verifies the "each" in the general 2.9.4 statement applies to `for`.

```
begin test "for loop with redirection on terminator"
  script
    for i in 1 2 3; do
      echo "$i"
    done > tmp_for_redir.txt
    cat tmp_for_redir.txt
  expect
    stdout "1\n2\n3"
    stderr ""
    exit_code 0
end test "for loop with redirection on terminator"
```

#### Test: subshell with redirection on terminator

Redirections on the `( ... )` terminator apply to all commands within the
subshell.

```
begin test "subshell with redirection on terminator"
  script
    ( echo hello ) > tmp_sub_redir.txt
    cat tmp_sub_redir.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "subshell with redirection on terminator"
```

#### Test: stderr redirect on brace group captures fd 2

Each redirection on a compound command shall apply to all commands within.
A stderr redirect on a brace group causes explicit writes to fd 2 inside
the group to go to the redirected target.

```
begin test "stderr redirect on brace group captures fd 2"
  script
    { echo out; echo err >&2; } 2>tmp_cc_stderr.txt
    printf "captured:"
    cat tmp_cc_stderr.txt
  expect
    stdout "out\ncaptured:err"
    stderr ""
    exit_code 0
end test "stderr redirect on brace group captures fd 2"
```

#### Test: stdin redirect on brace group feeds fd 0

Each redirection on a compound command shall apply to all commands within.
A stdin redirect on a brace group causes reads from fd 0 inside the group
to come from the redirected source.

```
begin test "stdin redirect on brace group feeds fd 0"
  script
    printf 'hello\n' > tmp_cc_stdin.txt
    { read line; echo "$line"; } < tmp_cc_stdin.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "stdin redirect on brace group feeds fd 0"
```

#### Test: command not found diagnostic captured by brace group stderr redirect

A "not found" diagnostic from a command inside a brace group shall go to
the compound command's redirected fd 2, not the original stderr.

```
begin test "command not found diagnostic captured by brace group stderr redirect"
  script
    { nonexistent_cmd_xyz_cc; } 2>tmp_cc_notfound.txt
    cat tmp_cc_notfound.txt
  expect
    stdout ".+"
    stderr ""
    exit_code 0
end test "command not found diagnostic captured by brace group stderr redirect"
```

#### Test: readonly assignment diagnostic captured by brace group stderr redirect

A readonly variable assignment error inside a brace group shall write its
diagnostic to the compound command's redirected fd 2. Per 2.8.1 the shell
exits, so we verify by checking nothing leaked to the original stderr.

```
begin test "readonly assignment diagnostic captured by brace group stderr redirect"
  script
    readonly RO_CC=fixed
    { RO_CC=changed; } 2>tmp_cc_ro.txt
    echo "should not reach"
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "readonly assignment diagnostic captured by brace group stderr redirect"
```

#### Test: eval syntax error diagnostic captured by brace group stderr redirect

A syntax error from `eval` inside a brace group shall write its diagnostic
to the compound command's redirected fd 2. Per 2.9.5 the syntax error
causes the non-interactive shell to exit, so we verify by checking nothing
leaked to the original stderr.

```
begin test "eval syntax error diagnostic captured by brace group stderr redirect"
  script
    { eval 'if'; } 2>tmp_cc_eval.txt
    echo "should not reach"
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "eval syntax error diagnostic captured by brace group stderr redirect"
```

#### Test: body redirection error diagnostic captured by brace group stderr redirect

When a redirect inside the brace group body fails, the error message shall
go through the compound command's redirected fd 2, not the original stderr.

```
begin test "body redirection error diagnostic captured by brace group stderr redirect"
  script
    { echo hello > /no/such/dir/file; } 2>tmp_cc_badredir.txt
    cat tmp_cc_badredir.txt
  expect
    stdout ".+"
    stderr ""
    exit_code 0
end test "body redirection error diagnostic captured by brace group stderr redirect"
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

#### Test: subshell built-in effects do not persist

Built-in commands that affect the environment inside `( ... )` shall not
remain in effect after the subshell finishes.

```
begin test "subshell built-in effects do not persist"
  script
    start=$PWD
    mkdir elsewhere
    ( cd elsewhere )
    if [ "$PWD" = "$start" ]; then echo unchanged; else echo changed; fi
  expect
    stdout "unchanged"
    stderr ""
    exit_code 0
end test "subshell built-in effects do not persist"
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

#### Test: brace group exit status follows compound-list

The exit status of a grouping command shall be the exit status of its
compound-list for `{ ... ; }` as well as `( ... )`.

```
begin test "brace group exit status follows compound-list"
  script
    { false; }
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "brace group exit status follows compound-list"
```

#### Test: brace group runs in current shell environment

Grouping with `{ ... ; }` executes the compound-list in the current process
environment, so variable assignments remain in effect afterward.

```
begin test "brace group runs in current shell environment"
  script
    X=old
    { X=new; }
    printf '%s\n' "$X"
  expect
    stdout "new"
    stderr ""
    exit_code 0
end test "brace group runs in current shell environment"
```

#### Test: brace group allows newline before closing brace

For `{ ... ; }`, the semicolon before `}` is only an example of a control
operator delimiter; a `<newline>` may delimit the closing `}` instead.

```
begin test "brace group allows newline before closing brace"
  script
    {
      echo hi
    }
  expect
    stdout "hi"
    stderr ""
    exit_code 0
end test "brace group allows newline before closing brace"
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

#### Test: for-in word list performs command and arithmetic expansion

The words following `in` are expanded to generate the list of items, including
command substitution and arithmetic expansion.

```
begin test "for-in word list performs command and arithmetic expansion"
  script
    for i in $(printf 'a b') $((1+1)); do
      printf '<%s>\n' "$i"
    done
  expect
    stdout "<a>\n<b>\n<2>"
    stderr ""
    exit_code 0
end test "for-in word list performs command and arithmetic expansion"
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

#### Test: for loop exit status zero when no items

If no items result from expansion, the `for` loop body is not executed and the
exit status of the command shall be zero.

```
begin test "for loop exit status zero when no items"
  script
    false
    for i in; do
      false
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "for loop exit status zero when no items"
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

#### Test: for loop without in preserves quoted positional parameters

Omitting `in word ...` is equivalent to iterating over `"$@"`, so each
positional parameter is preserved as a separate item even if it contains
blanks.

```
begin test "for loop without in preserves quoted positional parameters"
  script
    set -- 'a b' c
    for i do
      printf '<%s>\n' "$i"
    done
  expect
    stdout "<a b>\n<c>"
    stderr ""
    exit_code 0
end test "for loop without in preserves quoted positional parameters"
```

#### Test: for loop variable retains last value after loop

The variable name is set to each item in the current execution environment,
so after the loop completes it retains the value from the last iteration.

```
begin test "for loop variable retains last value after loop"
  script
    for i in a b c; do
      :
    done
    echo "$i"
  expect
    stdout "c"
    stderr ""
    exit_code 0
end test "for loop variable retains last value after loop"
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

#### Test: case clause may start with leading paren

An optional leading `(` may appear before a `case` pattern list.

```
begin test "case clause may start with leading paren"
  script
    case a in
      (a) echo leadparen ;;
    esac
  expect
    stdout "leadparen"
    stderr ""
    exit_code 0
end test "case clause may start with leading paren"
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
    false
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

#### Test: case ;& exit status is from last executed clause

When `;&` fallthrough executes later clauses, the exit status of `case` shall
be the exit status of the last clause that was executed.

```
begin test "case ;& exit status is from last executed clause"
  script
    case a in
      a) : ;&
      b) false ;;
    esac
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "case ;& exit status is from last executed clause"
```

#### Test: case ;& can end with zero exit status

When `;&` fallthrough reaches a later clause whose compound-list succeeds, the
exit status of `case` shall be zero because that last executed clause
succeeded.

```
begin test "case ;& can end with zero exit status"
  script
    case a in
      a) : ;&
      b) : ;;
    esac
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "case ;& can end with zero exit status"
```

#### Test: last case clause may omit terminator

The final `case` clause does not require a `;;` or `;&` terminator.

```
begin test "last case clause may omit terminator"
  script
    case a in
      a) echo noterm
    esac
  expect
    stdout "noterm"
    stderr ""
    exit_code 0
end test "last case clause may omit terminator"
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

#### Test: case word undergoes parameter expansion before matching

The `case` word is expanded before matching, so parameter expansion in the
word determines which clause matches.

```
begin test "case word undergoes parameter expansion before matching"
  script
    word=hello
    case "$word" in
        $(printf hello)) echo matched ;;
        *) echo nomatch ;;
    esac
  expect
    stdout "matched"
    stderr ""
    exit_code 0
end test "case word undergoes parameter expansion before matching"
```

#### Test: case word undergoes command substitution before matching

The `case` word is expanded before matching, so command substitution in the
word determines which clause matches.

```
begin test "case word undergoes command substitution before matching"
  script
    case $(printf x) in
      x) echo cmdsub ;;
      *) echo no ;;
    esac
  expect
    stdout "cmdsub"
    stderr ""
    exit_code 0
end test "case word undergoes command substitution before matching"
```

#### Test: case word undergoes arithmetic expansion before matching

The `case` word is expanded before matching, so arithmetic expansion in the
word determines which clause matches.

```
begin test "case word undergoes arithmetic expansion before matching"
  script
    case $((2+3)) in
      5) echo arithmetic ;;
      *) echo no ;;
    esac
  expect
    stdout "arithmetic"
    stderr ""
    exit_code 0
end test "case word undergoes arithmetic expansion before matching"
```

#### Test: case word undergoes tilde expansion before matching

The `case` word is expanded before matching, so tilde expansion in the word
determines which clause matches.

```
begin test "case word undergoes tilde expansion before matching"
  script
    HOME="$PWD/home"
    mkdir -p "$HOME"
    case ~ in
      "$HOME") echo tilde ;;
      *) echo no ;;
    esac
  expect
    stdout "tilde"
    stderr ""
    exit_code 0
end test "case word undergoes tilde expansion before matching"
```

#### Test: case stops expanding patterns after first match

After the first matching `case` clause is found, no later patterns in the
statement shall be expanded.

```
begin test "case stops expanding patterns after first match"
  script
    rm -f later.txt
    case x in
      x) echo first ;;
      $(touch later.txt)) echo later ;;
    esac
    if test -f later.txt; then echo expanded; else echo not_expanded; fi
  expect
    stdout "first\nnot_expanded"
    stderr ""
    exit_code 0
end test "case stops expanding patterns after first match"
```

#### Test: quoted case pattern matches literally

Quoted parts of a `case` pattern are matched literally rather than as pattern
metacharacters.

```
begin test "quoted case pattern matches literally"
  script
    case 'a*b' in
      'a*b') echo literal ;;
      *) echo other ;;
    esac
  expect
    stdout "literal"
    stderr ""
    exit_code 0
end test "quoted case pattern matches literally"
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

#### Test: case ;& fallthrough through empty clause

When `;&` fallthrough passes through a clause whose compound-list is empty
(the "(if any)" case), the fallthrough shall continue to subsequent clauses.

```
begin test "case ;& fallthrough through empty clause"
  script
    case a in
      a) echo matched ;&
      b) ;&
      c) echo reached ;;
    esac
  expect
    stdout "matched\nreached"
    stderr ""
    exit_code 0
end test "case ;& fallthrough through empty clause"
```

#### Test: pattern with tilde expansion in case label

Case patterns undergo tilde expansion before matching, as explicitly listed
in the specification alongside parameter, command substitution, and arithmetic
expansion.

```
begin test "pattern with tilde expansion in case label"
  script
    HOME=/test/home
    case "/test/home" in
      ~) echo tilde ;;
      *) echo no ;;
    esac
  expect
    stdout "tilde"
    stderr ""
    exit_code 0
end test "pattern with tilde expansion in case label"
```

#### Test: case ;& fallthrough to end of case statement

When `;&` fallthrough reaches the final clause and there are no further
clauses in the case statement, the case command completes.

```
begin test "case ;& fallthrough to end of case statement"
  script
    case a in
      a) echo one ;&
      b) echo two
    esac
  expect
    stdout "one\ntwo"
    stderr ""
    exit_code 0
end test "case ;& fallthrough to end of case statement"
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

#### Test: case question-mark matches one byte in C locale

In the C locale, `?` matches a single byte. The two-byte UTF-8 sequence
`\303\251` is two characters, so `?` does not match the whole value.

```
begin test "case question-mark matches one byte in C locale"
  setenv "LC_ALL" "C"
  script
    v=$(printf '\303\251')
    case "$v" in
      (??) echo two;;
      (?) echo one;;
      (*) echo no;;
    esac
  expect
    stdout "two"
    stderr ""
    exit_code 0
end test "case question-mark matches one byte in C locale"
```

#### Test: case question-mark matches one multi-byte character

In C.UTF-8, the two-byte sequence `\303\251` (U+00E9) is a single character,
so `?` matches the whole value.

```
begin test "case question-mark matches one multi-byte character"
  setenv "LC_ALL" "C.UTF-8"
  script
    v=$(printf '\303\251')
    case "$v" in
      (?) echo one;;
      (*) echo no;;
    esac
  expect
    stdout "one"
    stderr ""
    exit_code 0
end test "case question-mark matches one multi-byte character"
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

#### Test: if stops after first successful elif

Each `elif` compound-list shall be executed in turn until one has zero exit
status; after that `then` compound-list runs and the command completes.

```
begin test "if stops after first successful elif"
  script
    if false; then
      echo if
    elif true; then
      echo elif1
    elif true; then
      echo elif2
    fi
  expect
    stdout "elif1"
    stderr ""
    exit_code 0
end test "if stops after first successful elif"
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

#### Test: if executes else branch on failure

If the initial `if` and all `elif` compound-lists have non-zero status, the
`else` compound-list shall be executed.

```
begin test "if executes else branch on failure"
  script
    if false; then
      echo if
    else
      echo else
    fi
  expect
    stdout "else"
    stderr ""
    exit_code 0
end test "if executes else branch on failure"
```

#### Test: if executes else after failed elif chain

If the initial `if` and each `elif` compound-list have non-zero status, the
`else` compound-list shall be executed.

```
begin test "if executes else after failed elif chain"
  script
    if false; then
      echo if
    elif false; then
      echo elif1
    else
      echo else
    fi
  expect
    stdout "else"
    stderr ""
    exit_code 0
end test "if executes else after failed elif chain"
```

#### Test: if exit status from else clause

When the `else` compound-list is executed, the exit status of the `if`
command shall be the exit status of that `else` compound-list.

```
begin test "if exit status from else clause"
  script
    if false; then
      echo if
    else
      false
    fi
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "if exit status from else clause"
```

#### Test: if exit status from elif then clause

The exit status of the `if` command shall be the exit status of whichever
`then` compound-list was executed, including one reached via an `elif`.

```
begin test "if exit status from elif then clause"
  script
    if false; then
      true
    elif true; then
      false
    fi
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "if exit status from elif then clause"
```

#### Test: if exit status zero when no branch body executes

If neither a `then` nor an `else` compound-list is executed, the exit status
of the `if` command shall be zero.

```
begin test "if exit status zero when no branch body executes"
  script
    false
    if false; then
      echo if
    elif false; then
      echo elif
    fi
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "if exit status zero when no branch body executes"
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

#### Test: while loop re-executes condition list each iteration

The `while` command shall execute its first compound-list repeatedly, once
before each possible execution of the body.

```
begin test "while loop re-executes condition list each iteration"
  script
    count=0
    while count=$((count + 1)); [ "$count" -le 2 ]; do
      printf "%s\n" "$count"
    done
  expect
    stdout "1\n2"
    stderr ""
    exit_code 0
end test "while loop re-executes condition list each iteration"
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
    false
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

#### Test: until loop re-executes condition list each iteration

The `until` command shall execute its first compound-list repeatedly, once
before each possible execution of the body.

```
begin test "until loop re-executes condition list each iteration"
  script
    count=0
    until count=$((count + 1)); [ "$count" -gt 2 ]; do
      printf "%s\n" "$count"
    done
  expect
    stdout "1\n2"
    stderr ""
    exit_code 0
end test "until loop re-executes condition list each iteration"
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

#### Test: until loop exit status from last iteration

The exit status of the `until` loop is the exit status of the last
compound-list-2 executed.

```
begin test "until loop exit status from last iteration"
  script
    counter=0
    until [ "$counter" -ge 1 ]; do
      counter=$((counter + 1))
      false
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 1
end test "until loop exit status from last iteration"
```

#### Test: until loop exit status zero when body never executes

If the body is never executed, the exit status of the `until` loop shall be
zero.

```
begin test "until loop exit status zero when body never executes"
  script
    false
    until true; do
      false
    done
    exit $?
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "until loop exit status zero when body never executes"
```
