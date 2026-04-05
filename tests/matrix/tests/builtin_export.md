# Test Suite for 2.15 Special Built-In: export

This test suite covers the **export** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities export](#215-special-built-in-utilities-export)

## 2.15 Special Built-In Utilities export

#### NAME

> export — set the export attribute for variables

#### SYNOPSIS

> ```
> export name[=word]...
> export -p
> ```

#### DESCRIPTION

> The shell shall give the [*export*](#export) attribute to the variables corresponding to the specified *name*s, which shall cause them to be in the environment of subsequently executed commands. If the name of a variable is followed by =*word*, then the value of that variable shall be set to *word*.
>
> The [*export*](#export) special built-in shall be a declaration utility. Therefore, if *export* is recognized as the command name of a simple command, then subsequent words of the form *name*=*word* shall be expanded in an assignment context. See [2.9.1.1 Order of Processing](#2911-order-of-processing).
>
> The [*export*](#export) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> When **-p** is specified, [*export*](#export) shall write to the standard output the names and values of all exported variables, in the following format:
>
> ```
> "export %s=%s\n", <name>, <value>
> ```
>
> if *name* is set, and:
>
> ```
> "export %s\n", <name>
> ```
>
> if *name* is unset.
>
> The shell shall format the output, including the proper use of quoting, so that it is suitable for reinput to the shell as commands that achieve the same exporting results, except:
>
> 1. Read-only variables with values cannot be reset.
> 2. Variables that were unset at the time they were output need not be reset to the unset state if a value is assigned to the variable between the time the state was saved and the time at which the saved output is reinput to the shell.
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
> - \>0: At least one operand could not be processed as requested, such as a *name* operand that could not be exported or an attempt to modify a *readonly* variable using a *name*=*word* operand, or the **-p** option was specified and a write error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> Note that, unless *X* was previously marked readonly, the value of `"$?"` after:
>
> ```
> export X=$(false)
> ```
>
> will be 0 (because [*export*](#export) successfully set *X* to the empty string) and that execution continues, even if [*set*](#set) **-e** is in effect. In order to detect command substitution failures, a user must separate the assignment from the export, as in:
>
> ```
> X=$(false)
> export X
> ```
>
> In shells that support extended assignment syntax, for example to allow an array to be populated with a single assignment, such extensions can typically only be used in assignments specified as arguments to [*export*](#export) if the command word is literally *export*, and not if it is some other word that expands to *export*. For example:
>
> ```
> # Shells that support array assignment as an extension generally
> # support this:
> export x=(1 2 3); echo ${x[0]}  # outputs 1
> # But generally do not support this:
> e=export; $e x=(1 2 3); echo ${x[0]}  # syntax error
> ```

#### EXAMPLES

> Export *PWD* and *HOME* variables:
>
> ```
> export PWD HOME
> ```
>
> Set and export the *PATH* variable:
>
> ```
> export PATH="/local/bin:$PATH"
> ```
>
> Save and restore all exported variables:
>
> ```
> export -p > temp-file
> unset a lot of variables
>
> ... processing
>
> . ./temp-file
> ```
>
> **Note:** If LANG, LC_CTYPE or LC_ALL are left altered or unset in the above example prior to sourcing `temp-file`, the results may be undefined.

#### RATIONALE

> Some historical shells use the no-argument case as the functional equivalent of what is required here with **-p**. This feature was left unspecified because it is not historical practice in all shells, and some scripts may rely on the now-unspecified results on their implementations. Attempts to specify the **-p** output as the default case were unsuccessful in achieving consensus. The **-p** option was added to allow portable access to the values that can be saved and then later restored using; for example, a [*dot*](#dot) script.
>
> Some implementations extend the shell's assignment syntax, for example to allow an array to be populated with a single assignment, and in order for such an extension to be usable in assignments specified as arguments to [*export*](#export) these shells have *export* as a separate token in their grammar. This standard only permits an extension of this nature when the input to the shell would contain a syntax error according to the standard grammar. Note that although *export* can be a separate token in the shell's grammar, it cannot be a reserved word since *export* is a candidate for alias substitution whereas reserved words are not (see [2.3.1 Alias Substitution](#231-alias-substitution)).

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
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/6 is applied, adding the following text to the end of the first paragraph of the DESCRIPTION: "If the name of a variable is followed by =*word*, then the value of that variable shall be set to *word*.". The reason for this change is that the SYNOPSIS for [*export*](#export) includes:
>
> ```
> export name[=word]...
> ```
>
> but the meaning of the optional "=*word*" is never explained in the text.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0043 [352] is applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0051 [654] and XCU/TC2-2008/0052 [960] are applied.

#### Issue 8

> Austin Group Defect 351 is applied, requiring [*export*](#export) to be a declaration utility.
>
> Austin Group Defect 367 is applied, changing the EXIT STATUS section.
>
> Austin Group Defect 1258 is applied, changing the EXAMPLES section.
>
> Austin Group Defect 1393 is applied, changing the APPLICATION USAGE and RATIONALE sections.

*End of informative text.*

### Tests

#### Test: export makes variable available to child processes

`export` gives variables the export attribute so they appear in the
environment of subsequently executed commands.

```
begin test "export makes variable available to child processes"
  script
    foo="bar"
    export foo baz="qux"
    env | grep -E "^(foo|baz)=" | sort
  expect
    stdout "baz=qux\nfoo=bar"
    stderr ""
    exit_code 0
end test "export makes variable available to child processes"
```

#### Test: export -p generates eval-able output

`export -p` produces output suitable for reinput to the shell.

```
begin test "export -p generates eval-able output"
  script
    export EXPORTED_VAR="val with spaces"
    output=$(export -p | grep "EXPORTED_VAR=")
    unset EXPORTED_VAR
    eval "$output"
    echo "$EXPORTED_VAR"
  expect
    stdout "val with spaces"
    stderr ""
    exit_code 0
end test "export -p generates eval-able output"
```

#### Test: export as declaration utility

`export` is a declaration utility: `name=word` arguments undergo
assignment context expansion.

```
begin test "export as declaration utility"
  script
    export decl_var="~"
    echo "$decl_var"
  expect
    stdout ".*"
    stderr ""
    exit_code 0
end test "export as declaration utility"
```

#### Test: export -p shows exported variable

`export -p` lists all exported variables in a format suitable for
reinput. A variable that has been exported must appear in the
output.

```
begin test "export -p shows exported variable"
  script
    MYEXPORTVAR=hello
    export MYEXPORTVAR
    export -p | grep MYEXPORTVAR
  expect
    stdout ".*export.*MYEXPORTVAR.*hello.*"
    stderr ""
    exit_code 0
end test "export -p shows exported variable"
```

#### Test: export -p handles spaces in values

The `export -p` output must correctly quote values that contain
spaces so they can be used as shell input.

```
begin test "export -p handles spaces in values"
  script
    TESTV="has spaces"
    export TESTV
    export -p | grep TESTV
  expect
    stdout ".*export.*TESTV.*"
    stderr ""
    exit_code 0
end test "export -p handles spaces in values"
```
