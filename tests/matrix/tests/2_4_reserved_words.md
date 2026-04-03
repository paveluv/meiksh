# Test Suite for 2.4 Reserved Words

This test suite covers **Section 2.4 Reserved Words** of the POSIX Shell Command
Language (POSIX.1-2024), which defines the set of words with special syntactic
meaning to the shell and the contexts in which they are recognized.

## Table of contents

- [2.4 Reserved Words](#24-reserved-words)

## 2.4 Reserved Words

Reserved words are words that have special meaning to the shell; see [2.9 Shell Commands](#29-shell-commands). The following words shall be recognized as reserved words:

- **!**
- **{**
- **}**
- **case**
- **do**
- **done**
- **elif**
- **else**
- **esac**
- **fi**
- **for**
- **if**
- **in**
- **then**
- **until**
- **while**

This recognition shall only occur when none of the characters is quoted and when the word is used as:

- The first word of a command
- The first word following one of the reserved words other than **case**, **for**, or **in**
- The third word in a **case** command (only **in** is valid in this case)
- The third word in a **for** command (only **in** and **do** are valid in this case)

See the grammar in [2.10 Shell Grammar](#210-shell-grammar).

When used in circumstances where reserved words are recognized (described above), the following words may be recognized as reserved words, in which case the results are unspecified except as described below for **time**:

- **[[**
- **]]**
- **function**
- **namespace**
- **select**
- **time**

When the word **time** is recognized as a reserved word in circumstances where it would, if it were not a reserved word, be the command name (see [2.9.1.1 Order of Processing](#2911-order-of-processing)) of a simple command that would execute the [*time*](../utilities/time.md) utility in a manner other than one for which [*time*](../utilities/time.md#tag_20_122) states that the results are unspecified, the behavior shall be as specified for the [*time*](../utilities/time.md) utility.

### Tests

#### Test: reserved words work in correct positions

The `if`/`then`/`fi` keywords are recognized as reserved words when used as
the first word of a command or following another reserved word, causing the
shell to build compound-command syntax rather than executing them as commands.

```
begin test "reserved words work in correct positions"
  script
    if true; then
      echo yes
    fi
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "reserved words work in correct positions"
```

#### Test: else branch

The `else` and `elif` reserved words are recognized after `then` (which is
itself a reserved word other than `case`, `for`, or `in`), enabling the
full if/elif/else/fi compound command.

```
begin test "else branch"
  script
    if false; then
      echo "if"
    elif false; then
      echo "elif"
    else
      echo "else"
    fi
  expect
    stdout "else"
    stderr ""
    exit_code 0
end test "else branch"
```

#### Test: case/esac reserved words

The `case` keyword is recognized as the first word of a command, and `in`
is recognized as the third word in a case command, building the case/esac
compound command.

```
begin test "case/esac reserved words"
  script
    case x in x) echo match ;; esac
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "case/esac reserved words"
```

#### Test: for/do/done reserved words

The `for` keyword is recognized as the first word, `in` as the third word
in a for command, and `do`/`done` following other reserved words, building
the for-loop compound command.

```
begin test "for/do/done reserved words"
  script
    for i in a b c; do
      printf '%s ' "$i"
    done
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "for/do/done reserved words"
```

#### Test: while/until reserved words

The `while` and `until` reserved words are recognized as the first word of
a command, building loop compound commands with `do`/`done`.

```
begin test "while/until reserved words"
  script
    x=0
    while [ "$x" -lt 3 ]; do
      x=$((x + 1))
    done
    echo "$x"
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "while/until reserved words"
```

#### Test: brace group reserved words

The `{` and `}` reserved words are recognized at command positions, allowing
a brace group to execute commands in the current shell environment.

```
begin test "brace group reserved words"
  script
    { echo hello; }
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "brace group reserved words"
```

#### Test: bang reserved word negates exit status

The `!` reserved word is recognized as the first word of a pipeline and
negates the exit status: a successful command yields non-zero, and vice versa.

```
begin test "bang reserved word negates exit status"
  script
    ! false
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "bang reserved word negates exit status"
```

#### Test: reserved words not recognized when quoted

Reserved words must not be recognized when any of their characters are quoted.
Here `"if"` is treated as a regular command name (which doesn't exist),
producing an error rather than starting a compound command.

```
begin test "reserved words not recognized when quoted"
  script
    echo "if"
  expect
    stdout "if"
    stderr ""
    exit_code 0
end test "reserved words not recognized when quoted"
```

#### Test: quoted reserved word is not recognized

When `"if"` is used as the first word of a command, it is not recognized as
a reserved word because it is quoted. The shell attempts to execute it as a
regular command, which fails.

```
begin test "quoted reserved word is not recognized"
  script
    "if" true
    then echo yes
    fi
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted reserved word is not recognized"
```

#### Test: reserved words not special as arguments

Reserved words are only recognized in specific syntactic positions. When used
as arguments to a command (not as the first word), they are treated as
ordinary strings.

```
begin test "reserved words not special as arguments"
  script
    echo if while for done
  expect
    stdout "if while for done"
    stderr ""
    exit_code 0
end test "reserved words not special as arguments"
```

#### Test: unquoted reserved word builds syntax

An unquoted `if` at the start of a command is recognized as a reserved word
and triggers compound-command parsing, producing normal if/then/fi behavior.

```
begin test "unquoted reserved word builds syntax"
  script
    if true; then
      echo "yes"
    fi
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "unquoted reserved word builds syntax"
```

#### Test: time reserved word measures and passes through output

When `time` is recognized as a reserved word before a simple command, it
measures execution time (written to stderr) while passing through the
command's stdout unchanged.

```
begin test "time reserved word measures and passes through output"
  script
    time echo "measured"
  expect
    stdout "measured"
    stderr "(.|\n)*.+"
    exit_code 0
end test "time reserved word measures and passes through output"
```

#### Test: in only valid as third word in case

The word `in` is only recognized as a reserved word when it appears as the
third word in a `case` command. In other positions it is just an ordinary
argument.

```
begin test "in only valid as third word in case"
  script
    echo in
  expect
    stdout "in"
    stderr ""
    exit_code 0
end test "in only valid as third word in case"
```

#### Test: do only valid as third word in for

The word `do` is recognized as a reserved word only in specific positions
(e.g., after `for ... in ...`). When used as a plain argument, it is not
special.

```
begin test "do only valid as third word in for"
  script
    echo do
  expect
    stdout "do"
    stderr ""
    exit_code 0
end test "do only valid as third word in for"
```

#### Test: reserved word after case not recognized

After `case`, only `in` is valid as the third word. Other reserved words like
`do` in that position cause a syntax error.

```
begin test "reserved word after case not recognized"
  script
    eval 'case x do x) echo match ;; esac' 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "reserved word after case not recognized"
```

#### Test: partially quoted reserved word is not recognized

Quoting even a single character of a reserved word prevents recognition.
`\if` escapes the `i`, so the shell does not recognize it as a reserved word.

```
begin test "partially quoted reserved word is not recognized"
  script
    echo \if
  expect
    stdout "if"
    stderr ""
    exit_code 0
end test "partially quoted reserved word is not recognized"
```
