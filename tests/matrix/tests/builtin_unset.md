# Test Suite for 2.15 Special Built-In: unset

This test suite covers the **unset** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities unset](#215-special-built-in-utilities-unset)

## 2.15 Special Built-In Utilities unset

#### NAME

> unset — unset values and attributes of variables and functions

#### SYNOPSIS

> `unset [-fv] name...`

#### DESCRIPTION

> The [*unset*](#unset) utility shall unset each variable or function definition specified by *name* that does not have the *readonly* attribute and remove any attributes other than *readonly* that have been given to *name* (see [2.15 Special Built-In Utilities](#215-special-built-in-utilities) *export* and *readonly*).
>
> If **-v** is specified, *name* refers to a variable name and the shell shall unset it and remove it from the environment. Read-only variables cannot be unset.
>
> If **-f** is specified, *name* refers to a function and the shell shall unset the function definition.
>
> If neither **-f** nor **-v** is specified, *name* refers to a variable; if a variable by that name does not exist, it is unspecified whether a function by that name, if any, shall be unset.
>
> Unsetting a variable or function that was not previously set shall not be considered an error and does not cause the shell to abort.
>
> The [*unset*](#unset) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> Note that:
>
> ```
> VARIABLE=
> ```
>
> is not equivalent to an [*unset*](#unset) of **VARIABLE**; in the example, **VARIABLE** is set to `""`. Also, the variables that can be [*unset*](#unset) should not be misinterpreted to include the special parameters (see [2.5.2 Special Parameters](#252-special-parameters)).

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

> Not used.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> - 0: All *name* operands were successfully unset.
> - \>0: At least one *name* could not be unset.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> None.

#### EXAMPLES

> Unset *VISUAL* variable:
>
> ```
> unset -v VISUAL
> ```
>
> Unset the functions **foo** and **bar**:
>
> ```
> unset -f foo bar
> ```

#### RATIONALE

> Consideration was given to omitting the **-f** option in favor of an *unfunction* utility, but the standard developers decided to retain historical practice.
>
> The **-v** option was introduced because System V historically used one name space for both variables and functions. When [*unset*](#unset) is used without options, System V historically unset either a function or a variable, and there was no confusion about which one was intended. A portable POSIX application can use [*unset*](#unset) without an option to unset a variable, but not a function; the **-f** option must be used.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities)
>
> XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

#### Issue 6

> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.

#### Issue 8

> Austin Group Defect 1075 is applied, clarifying that [*unset*](#unset) removes attributes, other than *readonly*, from the variables it unsets.

*End of informative text.*

### Tests

#### Test: unset removes a variable

`unset` removes a variable from the shell environment.

```
begin test "unset removes a variable"
  script
    my_var="value"
    unset my_var
    echo "${my_var:-is_unset}"
  expect
    stdout "is_unset"
    stderr ""
    exit_code 0
end test "unset removes a variable"
```

#### Test: unset -f removes a function

`unset -f` removes a function definition.

```
begin test "unset -f removes a function"
  script
    my_func() { echo "running"; }
    unset -f my_func
    my_func 2>/dev/null || echo "not found"
  expect
    stdout "not found"
    stderr ""
    exit_code 0
end test "unset -f removes a function"
```

#### Test: unset without -f targets the variable only

With neither **-f** nor **-v**, the operand names a variable. Unsetting
that variable does not remove a function that shares the same name.

```
begin test "unset without -f targets the variable only"
  script
    dupname=var_value
    dupname() { echo func_body; }
    unset dupname
    printf '%s\n' "${dupname-unset}"
    dupname
  expect
    stdout "unset\nfunc_body"
    stderr ""
    exit_code 0
end test "unset without -f targets the variable only"
```

#### Test: unset of nonexistent variable is not an error

Unsetting a variable that was not previously set is not an error.

```
begin test "unset of nonexistent variable is not an error"
  script
    unset this_var_never_existed_xyz
    echo "ok $?"
  expect
    stdout "ok 0"
    stderr ""
    exit_code 0
end test "unset of nonexistent variable is not an error"
```

#### Test: unset -v explicitly targets a variable

The `-v` option names a variable operand; unsetting it removes the
variable from the shell and from the environment.

```
begin test "unset -v explicitly targets a variable"
  script
    export V_UNSET_V=1
    unset -v V_UNSET_V
    echo "${V_UNSET_V-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "unset -v explicitly targets a variable"
```

#### Test: unset removes variable from the environment

When an exported variable is unset, it is also removed from the
environment so child processes do not see it.

```
begin test "unset removes variable from the environment"
  script
    export UNSET_ENV_TEST=visible
    unset UNSET_ENV_TEST
    sh -c 'printf "%s\n" "${UNSET_ENV_TEST-gone}"'
  expect
    stdout "gone"
    stderr ""
    exit_code 0
end test "unset removes variable from the environment"
```

#### Test: unset fails on readonly variables

Read-only variables cannot be unset; the utility fails with non-zero
exit status.

```
begin test "unset fails on readonly variables"
  script
    readonly RO_UNSET=1
    unset RO_UNSET
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "unset fails on readonly variables"
```

#### Test: unset of readonly variable exits non-interactive shell

Since `unset` is a special built-in, a failure (such as attempting to unset
a readonly variable) is a special built-in utility error. Per
[2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors), a
non-interactive shell shall exit on a special built-in utility error.
Known `bash --posix` non-compliance #11: bash writes a diagnostic but
continues execution instead of exiting.

```
begin test "unset of readonly variable exits non-interactive shell"
  script
    readonly RO_VAR=1
    unset RO_VAR
    echo survived
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "unset of readonly variable exits non-interactive shell"
```
