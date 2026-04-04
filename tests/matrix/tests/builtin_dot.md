# Test Suite for 2.15 Special Built-In: dot

This test suite covers the **dot** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities dot](#215-special-built-in-utilities-dot)

## 2.15 Special Built-In Utilities dot

#### NAME

> dot — execute commands in the current environment

#### SYNOPSIS

> `. file`

#### DESCRIPTION

> The shell shall tokenize (see [2.3 Token Recognition](#23-token-recognition)) the contents of the *file*, parse the tokens (see [2.10 Shell Grammar](#210-shell-grammar)), and execute the resulting commands in the current environment. It is unspecified whether the commands are parsed and executed as a *program* (as for a shell script) or are parsed as a single *compound_list* that is executed after the entire file has been parsed.
>
> If *file* does not contain a `<slash>`, the shell shall use the search path specified by *PATH* to find the directory containing *file*. Unlike normal command search, however, the file searched for by the [*dot*](#dot) utility need not be executable. If no readable file is found, a non-interactive shell shall abort; an interactive shell shall write a diagnostic message to standard error.
>
> The [*dot*](#dot) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines), except for Guidelines 1 and 2.

#### OPTIONS

> None.

#### OPERANDS

> See the DESCRIPTION.

#### STDIN

> Not used.

#### INPUT FILES

> See the DESCRIPTION.

#### ENVIRONMENT VARIABLES

> See the DESCRIPTION.

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

> If no readable file was found or if the commands in the file could not be parsed, and the shell is interactive (and therefore does not abort; see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)), the exit status shall be non-zero. Otherwise, return the value of the last command executed, or a zero exit status if no command is executed.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> ```
> cat foobar
>
> foo=hello bar=world
>
> . ./foobar
> echo $foo $bar
>
> hello world
> ```

#### RATIONALE

> Some older implementations searched the current directory for the *file*, even if the value of *PATH* disallowed it. This behavior was omitted from this volume of POSIX.1-2024 due to concerns about introducing the susceptibility to trojan horses that the user might be trying to avoid by leaving **dot** out of *PATH .*
>
> The KornShell version of [*dot*](#dot) takes optional arguments that are set to the positional parameters. This is a valid extension that allows a [*dot*](#dot) script to behave identically to a function.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities), [return](#tag_19_25)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-164 is applied.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0038 [114] and XCU/TC1-2008/0039 [214] are applied.

#### Issue 8

> Austin Group Defect 252 is applied, adding a requirement for [*dot*](#dot) to support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines) (except for Guidelines 1 and 2, since the utility's name is `'.'`).
>
> Austin Group Defect 953 is applied, clarifying how the commands in the *file* are parsed.
>
> Austin Group Defect 1265 is applied, updating the DESCRIPTION to align with the changes made to [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) between Issue 6 and Issue 7.

*End of informative text.*

### Tests

#### Test: dot sources file in current directory

The dot utility executes file contents in the current environment.

```
begin test "dot sources file in current directory"
  script
    echo 'export SOURCED_VAR=hello' > tmp_source.sh
    . ./tmp_source.sh
    echo "$SOURCED_VAR"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "dot sources file in current directory"
```

#### Test: dot sources file via PATH resolution

When `file` has no slash, dot searches PATH for it.

```
begin test "dot sources file via PATH resolution"
  script
    mkdir -p tmp_bin
    echo 'export SOURCED_VAR=path_resolved' > tmp_bin/tmp_source.sh
    PATH="$PWD/tmp_bin:$PATH" . tmp_source.sh
    echo "$SOURCED_VAR"
  expect
    stdout "path_resolved"
    stderr ""
    exit_code 0
end test "dot sources file via PATH resolution"
```
