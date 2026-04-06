# Test Suite for 2.15 Special Built-In: eval

This test suite covers the **eval** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities eval](#215-special-built-in-utilities-eval)

## 2.15 Special Built-In Utilities eval

#### NAME

> eval — construct command by concatenating arguments

#### SYNOPSIS

> `eval [argument...]`

#### DESCRIPTION

> The [*eval*](#eval) utility shall construct a command string by concatenating *argument*s together, separating each with a `<space>` character. The constructed command string shall be tokenized (see [2.3 Token Recognition](#23-token-recognition)), parsed (see [2.10 Shell Grammar](#210-shell-grammar)), and executed by the shell in the current environment. It is unspecified whether the commands are parsed and executed as a *program* (as for a shell script) or are parsed as a single *compound_list* that is executed after the entire constructed command string has been parsed.

#### OPTIONS

> None.

#### OPERANDS

> See the DESCRIPTION.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> None.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> Not used.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> If there are no *argument*s, or only null *argument*s, [*eval*](#eval) shall return a zero exit status; otherwise, it shall return the exit status of the command defined by the string of concatenated *argument*s separated by `<space>` characters, or a non-zero exit status if the concatenation could not be parsed as a command and the shell is interactive (and therefore did not abort).

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> Since [*eval*](#eval) is not required to recognize the `"--"` end of options delimiter, in cases where the argument(s) to [*eval*](#eval) might begin with `'-'` it is recommended that the first argument is prefixed by a string that will not alter the commands to be executed, such as a `<space>` character:
>
> ```
> eval " $commands"
> ```
>
> or:
>
> ```
> eval " $(some_command)"
> ```

#### EXAMPLES

> ```
> foo=10 x=foo
> y='$'$x
> echo $y
>
> $foo
>
> eval y='$'$x
> echo $y
>
> 10
> ```

#### RATIONALE

> This standard allows, but does not require, [*eval*](#eval) to recognize `"--"`. Although this means applications cannot use `"--"` to protect against options supported as an extension (or errors reported for unsupported options), the nature of the [*eval*](#eval) utility is such that other means can be used to provide this protection (see APPLICATION USAGE above).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0040 [114], XCU/TC1-2008/0041 [163], and XCU/TC1-2008/0042 [163] are applied.

#### Issue 8

> Austin Group Defect 953 is applied, clarifying how the commands in the constructed command string are parsed.

*End of informative text.*

### Tests

#### Test: eval concatenates and executes arguments

`eval` joins arguments with spaces and executes the resulting string.

```
begin test "eval concatenates and executes arguments"
  script
    foo="bar"
    eval echo "$foo" "and" "baz"
  expect
    stdout "bar and baz"
    stderr ""
    exit_code 0
end test "eval concatenates and executes arguments"
```

#### Test: eval with no arguments returns 0

When eval has no arguments it returns zero.

```
begin test "eval with no arguments returns 0"
  script
    eval
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "eval with no arguments returns 0"
```

#### Test: eval with empty string returns 0

When eval has only null arguments it returns zero.

```
begin test "eval with empty string returns 0"
  script
    eval ""
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "eval with empty string returns 0"
```

#### Test: eval exit status follows executed command

Otherwise, eval returns the exit status of the command formed from its
arguments.

```
begin test "eval exit status follows executed command"
  script
    eval false
    echo "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "eval exit status follows executed command"
```
