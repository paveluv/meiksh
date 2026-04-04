# Test Suite for 2.15 Special Built-In: readonly

This test suite covers the **readonly** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities readonly](#215-special-built-in-utilities-readonly)

## 2.15 Special Built-In Utilities readonly

#### NAME

> readonly — set the readonly attribute for variables

#### SYNOPSIS

> ```
> readonly name[=word]...
> readonly -p
> ```

#### DESCRIPTION

> The variables whose *name*s are specified shall be given the [*readonly*](#readonly) attribute. The values of variables with the [*readonly*](#readonly) attribute cannot be changed by subsequent assignment or use of the [*export*](#export), [*getopts*](docs/posix/md/utilities/getopts.md), [*readonly*](#readonly), or [*read*](docs/posix/md/utilities/read.md) utilities, nor can those variables be unset by the [*unset*](#unset) utility. As described in XBD [*8.1 Environment Variable Definition*](docs/posix/md/basedefs/V1_chap08.md#81-environment-variable-definition), conforming applications shall not request to mark a variable as *readonly* if it is documented as being manipulated by a shell built-in utility, as it may render those utilities unable to complete successfully. If the name of a variable is followed by =*word*, then the value of that variable shall be set to *word*.
>
> The [*readonly*](#readonly) special built-in shall be a declaration utility. Therefore, if *readonly* is recognized as the command name of a simple command, then subsequent words of the form *name*=*word* shall be expanded in an assignment context. See [2.9.1.1 Order of Processing](#2911-order-of-processing).
>
> The [*readonly*](#readonly) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> When **-p** is specified, [*readonly*](#readonly) writes to the standard output the names and values of all read-only variables, in the following format:
>
> ```
> "readonly %s=%s\n", <name>, <value>
> ```
>
> if *name* is set, and
>
> ```
> "readonly %s\n", <name>
> ```
>
> if *name* is unset.
>
> The shell shall format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same value and *readonly* attribute-setting results in a shell execution environment in which:
>
> 1. Variables with values at the time they were output do not have the *readonly* attribute set.
> 2. Variables that were unset at the time they were output do not have a value at the time at which the saved output is reinput to the shell.
>
> When no arguments are given, the results are unspecified.

#### OPTIONS

> See the DESCRIPTION.

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

> See the DESCRIPTION.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> - 0: Successful completion.
> - \>0: At least one operand could not be processed as requested, such as a *name* operand that could not be marked *readonly* or an attempt to modify an already *readonly* variable using a *name*=*word* operand, or the **-p** option was specified and a write error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> In shells that support extended assignment syntax, for example to allow an array to be populated with a single assignment, such extensions can typically only be used in assignments specified as arguments to [*readonly*](#readonly) if the command word is literally *readonly*, and not if it is some other word that expands to *readonly*. For example:
>
> ```
> # Shells that support array assignment as an extension generally
> # support this:
> readonly x=(1 2 3); echo ${x[0]}  # outputs 1
> # But generally do not support this:
> r=readonly; $r x=(1 2 3); echo ${x[0]}  # syntax error
> ```

#### EXAMPLES

> ```
> readonly HOME
> ```

#### RATIONALE

> Some historical shells preserve the *readonly* attribute across separate invocations. This volume of POSIX.1-2024 allows this behavior, but does not require it.
>
> The **-p** option allows portable access to the values that can be saved and then later restored using, for example, a [*dot*](#dot) script. Also see the RATIONALE for [export](#tag_19_23) for a description of the no-argument and **-p** output cases and a related example.
>
> Read-only functions were considered, but they were omitted as not being historical practice or particularly useful. Furthermore, functions must not be read-only across invocations to preclude "spoofing" (spoofing is the term for the practice of creating a program that acts like a well-known utility with the intent of subverting the real intent of the user) of administrative or security-relevant (or security-conscious) shell scripts.
>
> Attempts to set the *readonly* attribute on certain variables, such as *PWD ,* may have surprising results. Either [*readonly*](#readonly) will reject the attempt, or the attempt will succeed but the shell will continue to alter the contents of *PWD* during the [*cd*](docs/posix/md/utilities/cd.md) utility, or the attempt will succeed and render the [*cd*](docs/posix/md/utilities/cd.md) utility inoperative (since it must not change directories if it cannot also update *PWD ).*
>
> Some implementations extend the shell's assignment syntax, for example to allow an array to be populated with a single assignment, and in order for such an extension to be usable in assignments specified as arguments to [*readonly*](#readonly) these shells have *readonly* as a separate token in their grammar. This standard only permits an extension of this nature when the input to the shell would contain a syntax error according to the standard grammar. Note that although *readonly* can be a separate token in the shell's grammar, it cannot be a reserved word since *readonly* is a candidate for alias substitution whereas reserved words are not (see [2.3.1 Alias Substitution](#231-alias-substitution)).

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.9.1.1 Order of Processing](#2911-order-of-processing), [2.15 Special Built-In Utilities](#215-special-built-in-utilities)
>
> XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

#### Issue 6

> IEEE PASC Interpretation 1003.2 #203 is applied, clarifying the format when a variable is unset.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/7 is applied, adding the following text to the end of the first paragraph of the DESCRIPTION: "If the name of a variable is followed by =*word*, then the value of that variable shall be set to *word*.". The reason for this change is that the SYNOPSIS for [*readonly*](#readonly) includes:
>
> ```
> readonly name[=word]...
> ```
>
> but the meaning of the optional "=*word*" is never explained in the text.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0052 [960] is applied.

#### Issue 8

> Austin Group Defect 351 is applied, requiring [*readonly*](#readonly) to be a declaration utility.
>
> Austin Group Defect 367 is applied, clarifying that the values of *readonly* variables cannot be changed by subsequent use of the [*export*](#export), [*getopts*](docs/posix/md/utilities/getopts.md), [*readonly*](#readonly), or [*read*](docs/posix/md/utilities/read.md) utilities, and changing the EXIT STATUS, EXAMPLES and RATIONALE sections.
>
> Austin Group Defect 1393 is applied, changing the APPLICATION USAGE and RATIONALE sections.

*End of informative text.*

### Tests

#### Test: readonly prevents assignment

A variable with the readonly attribute cannot be reassigned.

```
begin test "readonly prevents assignment"
  script
    readonly RO_VAR="protected"
    RO_VAR="mutated"
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "readonly prevents assignment"
```

#### Test: readonly -p generates eval-able output

`readonly -p` produces output suitable for reinput to the shell.

```
begin test "readonly -p generates eval-able output"
  script
    readonly RO_VAR="protected"
    output=$(readonly -p | grep "RO_VAR=")
    echo "$output; echo \"$RO_VAR\"" > tmp_ro.sh
    $SHELL tmp_ro.sh
  expect
    stdout "protected"
    stderr ""
    exit_code 0
end test "readonly -p generates eval-able output"
```
