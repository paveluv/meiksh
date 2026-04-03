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

#### Test: reserved words recognized as first word of command

Reserved words such as `if`, `then`, and `fi` are recognized when they appear as the first word of a command, enabling compound command syntax.

```
begin test "reserved words recognized as first word of command"
  script
    if true; then echo yes; fi
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "reserved words recognized as first word of command"
```

#### Test: reserved words recognized after another reserved word

A reserved word is recognized when it is the first word following another reserved word (provided the preceding word is not `case`, `for`, or `in`). Here, `if` follows `!`.

```
begin test "reserved words recognized after another reserved word"
  script
    ! if false; then echo no; else echo yes; fi
  expect
    stdout "yes"
    stderr ""
    exit_code !=0
end test "reserved words recognized after another reserved word"
```

#### Test: reserved word not recognized after case

The word immediately following the `case` reserved word is not recognized as a reserved word. It is treated as an ordinary word (the word to be matched).

```
begin test "reserved word not recognized after case"
  script
    case if in if) echo ok;; esac
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "reserved word not recognized after case"
```

#### Test: reserved word not recognized after for

The word immediately following the `for` reserved word is not recognized as a reserved word. It is treated as an ordinary word (the variable name).

```
begin test "reserved word not recognized after for"
  script
    for do in 1; do echo "loop-$do"; done
  expect
    stdout "loop-1"
    stderr ""
    exit_code 0
end test "reserved word not recognized after for"
```

#### Test: reserved word not recognized after in

The word immediately following the `in` reserved word (in a `for` loop) is not recognized as a reserved word. It is treated as an ordinary word (an item to iterate over).

```
begin test "reserved word not recognized after in"
  script
    for i in if then else; do echo "$i"; done
  expect
    stdout "if\nthen\nelse"
    stderr ""
    exit_code 0
end test "reserved word not recognized after in"
```

#### Test: in recognized as third word in case

The word `in` is recognized as a reserved word when it is the third word in a `case` command.

```
begin test "in recognized as third word in case"
  script
    case x in x) echo ok;; esac
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "in recognized as third word in case"
```

#### Test: do recognized as third word in for

The word `do` is recognized as a reserved word when it is the third word in a `for` command (which implicitly loops over positional parameters).

```
begin test "do recognized as third word in for"
  script
    set -- a b c
    for i do echo "$i"; done
  expect
    stdout "a\nb\nc"
    stderr ""
    exit_code 0
end test "do recognized as third word in for"
```

#### Test: quoted reserved word is not recognized

A reserved word is not recognized if any of its characters are quoted. It is treated as a regular command name, which typically results in a "command not found" error.

```
begin test "quoted reserved word is not recognized"
  script
    $SHELL -c '"if" true; then echo yes; fi'
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted reserved word is not recognized"
```

#### Test: partially quoted reserved word is not recognized

A reserved word is not recognized even if only a single character is quoted via a backslash.

```
begin test "partially quoted reserved word is not recognized"
  script
    $SHELL -c '\if true; then echo yes; fi'
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "partially quoted reserved word is not recognized"
```

#### Test: quoted in is not recognized as third word in case

If the third word in a `case` command is a quoted `"in"`, it is not recognized as a reserved word, resulting in a syntax error because the unquoted `in` reserved word is strictly required.

```
begin test "quoted in is not recognized as third word in case"
  script
    $SHELL -c 'case x "in" x) echo ok;; esac'
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted in is not recognized as third word in case"
```

#### Test: quoted do is not recognized as third word in for

If the third word in a `for` command is a quoted `"do"`, it is not recognized as a reserved word, resulting in a syntax error because the unquoted `in` or `do` reserved word is strictly required.

```
begin test "quoted do is not recognized as third word in for"
  script
    $SHELL -c 'for i "do" echo "$i"; done'
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "quoted do is not recognized as third word in for"
```

#### Test: reserved word is not recognized as an argument

Reserved words are not recognized when they appear in argument positions (i.e., not the first word of a command and not following a qualifying reserved word). They are simply treated as ordinary text.

```
begin test "reserved word is not recognized as an argument"
  script
    echo if while for
  expect
    stdout "if while for"
    stderr ""
    exit_code 0
end test "reserved word is not recognized as an argument"
```
