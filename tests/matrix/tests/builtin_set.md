# Test Suite for 2.15 Special Built-In: set

This test suite covers the **set** special built-in utility
from Section 2.15 of the POSIX.1-2024 Shell Command Language specification.

## Table of contents

- [2.15 Special Built-In Utilities set](#215-special-built-in-utilities-set)

## 2.15 Special Built-In Utilities set

#### NAME

> set — set or unset options and positional parameters

#### SYNOPSIS

> ```
> set [-abCefhmnuvx] [-o option] [argument...]
> set [+abCefhmnuvx] [+o option] [argument...]
> set -- [argument...]
> set -o
> set +o
> ```

#### DESCRIPTION

> If no *option*s or *argument*s are specified, [*set*](#set) shall write the names and values of all shell variables in the collation sequence of the current locale. Each *name* shall start on a separate line, using the format:
>
> ```
> "%s=%s\n", <name>, <value>
> ```
>
> The *value* string shall be written with appropriate quoting; see the description of shell quoting in [2.2 Quoting](#22-quoting). The output shall be suitable for reinput to the shell, setting or resetting, as far as possible, the variables that are currently set; read-only variables cannot be reset.
>
> When options are specified, they shall set or unset attributes of the shell, as described below. When *argument*s are specified, they cause positional parameters to be set or unset, as described below. Setting or unsetting attributes and positional parameters are not necessarily related actions, but they can be combined in a single invocation of [*set*](#set).
>
> The [*set*](#set) special built-in shall support XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines) except that options can be specified with either a leading `<hyphen-minus>` (meaning enable the option) or `<plus-sign>` (meaning disable it) unless otherwise specified.
>
> Implementations shall support the options in the following list in both their `<hyphen-minus>` and `<plus-sign>` forms. These options can also be specified as options to [*sh*](docs/posix/md/utilities/sh.md).
>
> - **-a**: Set the *export* attribute for all variable assignments. When this option is on, whenever a value is assigned to a variable in the current shell execution environment, the *export* attribute shall be set for the variable. This applies to all forms of assignment, including those made as a side-effect of variable expansions or arithmetic expansions, and those made as a result of the operation of the [*cd*](docs/posix/md/utilities/cd.md), [*getopts*](docs/posix/md/utilities/getopts.md), or [*read*](docs/posix/md/utilities/read.md) utilities.
>
>     - **Note:** As discussed in [2.9.1 Simple Commands](#291-simple-commands), not all variable assignments happen in the current execution environment. When an assignment happens in a separate execution environment the *export* attribute is still set for the variable, but that does not affect the current execution environment.
> - **-b**: This option shall be supported if the implementation supports the User Portability Utilities option. When job control and **-b** are both enabled, the shell shall write asynchronous notifications of background job completions (including termination by a signal), and may write asynchronous notifications of background job suspensions. See [2.11 Job Control](#211-job-control) for details. When job control is disabled, the **-b** option shall have no effect. Asynchronous notification shall not be enabled by default.
> - **-C**: (Uppercase C.) Prevent existing regular files from being overwritten by the shell's `'>'` redirection operator (see [2.7.2 Redirecting Output](#272-redirecting-output)); the `">|"` redirection operator shall override this *noclobber* option for an individual file.
> - **-e**: When this option is on, when any command fails (for any of the reasons listed in [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) or by returning an exit status greater than zero), the shell immediately shall exit, as if by executing the [*exit*](#exit) special built-in utility with no arguments, with the following exceptions:
>
>     1. The failure of any individual command in a multi-command pipeline, or of any subshell environments in which command substitution was performed during word expansion, shall not cause the shell to exit. Only the failure of the pipeline itself shall be considered.
>     2. The **-e** setting shall be ignored when executing the compound list following the **while**, **until**, **if**, or **elif** reserved word, a pipeline beginning with the **!** reserved word, or any command of an AND-OR list other than the last.
>     3. If the exit status of a compound command other than a subshell command was the result of a failure while **-e** was being ignored, then **-e** shall not apply to this command.
>
>   This requirement applies to the shell environment and each subshell environment separately. For example, in:
>
>   ```
>   set -e; (false; echo one) | cat; echo two
>   ```
>
>   the [*false*](docs/posix/md/utilities/false.md) command causes the subshell to exit without executing `echo one`; however, `echo two` is executed because the exit status of the pipeline `(false; echo one) | cat` is zero.
>
>   In
>
>   ```
>   set -e; echo $(false; echo one) two
>   ```
>
>   the [*false*](docs/posix/md/utilities/false.md) command causes the subshell in which the command substitution is performed to exit without executing `echo one`; the exit status of the subshell is ignored and the shell then executes the word-expanded command `echo two`.
> - **-f**: The shell shall disable pathname expansion.
> - **-h**: Setting this option may speed up *PATH* searches (see XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables)). This option may be enabled by default.
> - **-m**: This option shall be supported if the implementation supports the User Portability Utilities option. When this option is enabled, the shell shall perform job control actions as described in [2.11 Job Control](#211-job-control). This option shall be enabled by default for interactive shells.
> - **-n**: The shell shall read commands but does not execute them; this can be used to check for shell script syntax errors. Interactive shells and subshells of interactive shells, recursively, may ignore this option.
> - **-o**: Write the current settings of the options to standard output in an unspecified format.
> - **+o**: Write the current option settings to standard output in a format that is suitable for reinput to the shell as commands that achieve the same options settings.
> - **-o***option*: Set various options, many of which shall be equivalent to the single option letters. The following values of *option* shall be supported:
>
>     - *allexport*: Equivalent to **-a**.
>     - *errexit*: Equivalent to **-e**.
>     - *ignoreeof*: Prevent an interactive shell from exiting on end-of-file. This setting prevents accidental logouts when `<control>`-D is entered. A user shall explicitly [*exit*](#exit) to leave the interactive shell. This option shall be supported if the system supports the User Portability Utilities option.
>     - *monitor*: Equivalent to **-m**. This option shall be supported if the system supports the User Portability Utilities option.
>     - *noclobber*: Equivalent to **-C** (uppercase C).
>     - *noglob*: Equivalent to **-f**.
>     - *noexec*: Equivalent to **-n**.
>     - *nolog*: Prevent the entry of function definitions into the command history; see [*Command History List*](docs/posix/md/utilities/sh.md#command-history-list). This option may have no effect; it is kept for compatibility with previous versions of the standard. This option shall be supported if the system supports the User Portability Utilities option.
>     - *notify*: Equivalent to **-b**.
>     - *nounset*: Equivalent to **-u**.
>     - *pipefail*: Derive the exit status of a pipeline from the exit statuses of all of the commands in the pipeline, not just the last (rightmost) command, as described in [2.9.2 Pipelines](#292-pipelines).
>     - *verbose*: Equivalent to **-v**.
>     - *vi*: Allow shell command line editing using the built-in [*vi*](docs/posix/md/utilities/vi.md) editor. Enabling [*vi*](docs/posix/md/utilities/vi.md) mode shall disable any other command line editing mode provided as an implementation extension. This option shall be supported if the system supports the User Portability Utilities option. It need not be possible to set [*vi*](docs/posix/md/utilities/vi.md) mode on for certain block-mode terminals.
>     - *xtrace*: Equivalent to **-x**.
> - **-u**: When the shell tries to expand, in a parameter expansion or an arithmetic expansion, an unset parameter other than the `'@'` and `'*'` special parameters, it shall write a message to standard error and the expansion shall fail with the consequences specified in [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors).
> - **-v**: The shell shall write its input to standard error as it is read.
> - **-x**: The shell shall write to standard error a trace for each command after it expands the command and before it executes it. It is unspecified whether the command that turns tracing off is traced.
>
> The default for all these options shall be off (unset) unless stated otherwise in the description of the option or unless the shell was invoked with them on; see [*sh*](docs/posix/md/utilities/sh.md).
>
> The remaining arguments shall be assigned in order to the positional parameters. The special parameter `'#'` shall be set to reflect the number of positional parameters. All positional parameters shall be unset before any new values are assigned.
>
> If the first argument is `'-'`, the results are unspecified.
>
> The special argument `"--"` immediately following the [*set*](#set) command name can be used to delimit the arguments if the first argument begins with `'+'` or `'-'`, or to prevent inadvertent listing of all shell variables when there are no arguments. The command [*set*](#set) **--** without *argument* shall unset all positional parameters and set the special parameter `'#'` to zero.

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
> - \>0: An invalid option was specified, or an error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> Application writers should avoid relying on [*set*](#set) **-e** within functions. For example, in the following script:
>
> ```
> set -e
> start() {
>     some_server
>     echo some_server started successfully
> }
> start || echo >&2 some_server failed
> ```
>
> the **-e** setting is ignored within the function body (because the function is a command in an AND-OR list other than the last). Therefore, if `some_server` fails, the function carries on to echo `"some_server started successfully"`, and the exit status of the function is zero (which means `"some_server failed"` is not output).
>
> Use of [*set*](#set) **-n** causes the shell to parse the rest of the script without executing any commands, meaning that [*set*](#set) **+n** cannot be used to undo the effect. Syntax checking is more commonly done via `sh` `-n` *script_name*.

#### EXAMPLES

> Write out all variables and their values:
>
> ```
> set
> ```
>
> Set $1, $2, and $3 and set `"$#"` to 3:
>
> ```
> set c a b
> ```
>
> Turn on the **-x** and **-v** options:
>
> ```
> set -xv
> ```
>
> Unset all positional parameters:
>
> ```
> set --
> ```
>
> Set $1 to the value of *x*, even if it begins with `'-'` or `'+'`:
>
> ```
> set -- "$x"
> ```
>
> Set the positional parameters to the expansion of *x*, even if *x* expands with a leading `'-'` or `'+'`:
>
> ```
> set -- $x
> ```

#### RATIONALE

> The [*set*](#set) -- form is listed specifically in the SYNOPSIS even though this usage is implied by the Utility Syntax Guidelines. The explanation of this feature removes any ambiguity about whether the [*set*](#set) -- form might be misinterpreted as being equivalent to [*set*](#set) without any options or arguments. The functionality of this form has been adopted from the KornShell. In System V, [*set*](#set) -- only unsets parameters if there is at least one argument; the only way to unset all parameters is to use [*shift*](#shift). Using the KornShell version should not affect System V scripts because there should be no reason to issue it without arguments deliberately; if it were issued as, for example:
>
> ```
> set -- "$@"
> ```
>
> and there were in fact no arguments resulting from `"$@"`, unsetting the parameters would have no result.
>
> The [*set*](#set) + form in early proposals was omitted as being an unnecessary duplication of [*set*](#set) alone and not widespread historical practice.
>
> The *noclobber* option was changed to allow [*set*](#set) **-C** as well as the [*set*](#set) **-o** *noclobber* option. The single-letter version was added so that the historical `"$-"` paradigm would not be broken; see [2.5.2 Special Parameters](#252-special-parameters).
>
> The description of the **-e** option is intended to match the behavior of the 1988 version of the KornShell.
>
> The **-h** option is related to command name hashing. See [*hash*](docs/posix/md/utilities/hash.md). The normative description is deliberately vague because the way this option works varies between shell implementations.
>
> Earlier versions of this standard specified **-h** as a way to locate and remember utilities to be invoked by functions as those functions are defined (the utilities are normally located when the function is executed). However, this did not match existing practice in most shells.
>
> The following [*set*](#set) options were omitted intentionally with the following rationale:
>
> - **-k**: The **-k** option was originally added by the author of the Bourne shell to make it easier for users of pre-release versions of the shell. In early versions of the Bourne shell the construct [*set*](#set) *name*=*value* had to be used to assign values to shell variables. The problem with **-k** is that the behavior affects parsing, virtually precluding writing any compilers. To explain the behavior of **-k**, it is necessary to describe the parsing algorithm, which is implementation-defined. For example:
>
>   ```
>   set -k; echo name=value
>   ```
>
>   and:
>
>   ```
>   set -k
>   echo name=value
>   ```
>
>   behave differently. The interaction with functions is even more complex. What is more, the **-k** option is never needed, since the command line could have been reordered.
> - **-t**: The **-t** option is hard to specify and almost never used. The only known use could be done with here-documents. Moreover, the behavior with *ksh* and [*sh*](docs/posix/md/utilities/sh.md) differs. The reference page says that it exits after reading and executing one command. What is one command? If the input is *date*;*date*, [*sh*](docs/posix/md/utilities/sh.md) executes both [*date*](docs/posix/md/utilities/date.md) commands while *ksh* does only the first.
>
> Consideration was given to rewriting [*set*](#set) to simplify its confusing syntax. A specific suggestion was that the [*unset*](#unset) utility should be used to unset options instead of using the non-[*getopt*()](docs/posix/md/functions/getopt.md)-able +*option* syntax. However, the conclusion was reached that the historical practice of using +*option* was satisfactory and that there was no compelling reason to modify such widespread historical practice.
>
> The **-o** option was adopted from the KornShell to address user needs. In addition to its generally friendly interface, **-o** is needed to provide the [*vi*](docs/posix/md/utilities/vi.md) command line editing mode, for which historical practice yields no single-letter option name. (Although it might have been possible to invent such a letter, it was recognized that other editing modes would be developed and **-o** provides ample name space for describing such extensions.)
>
> Historical implementations are inconsistent in the format used for **-o** option status reporting. The **+o** format without an option-argument was added to allow portable access to the options that can be saved and then later restored using, for instance, a dot script.
>
> Historically, [*sh*](docs/posix/md/utilities/sh.md) did trace the command [*set*](#set) **+x**, but *ksh* did not.
>
> The *ignoreeof* setting prevents accidental logouts when the end-of-file character (typically `<control>`-D) is entered. A user shall explicitly [*exit*](#exit) to leave the interactive shell.
>
> The [*set*](#set) **-m** option was added to apply only to the UPE because it applies primarily to interactive use, not shell script applications.
>
> The ability to do asynchronous notification became available in the 1988 version of the KornShell. To have it occur, the user had to issue the command:
>
> ```
> trap "jobs -n" CLD
> ```
>
> The C shell provides two different levels of an asynchronous notification capability. The environment variable *notify* is analogous to what is done in [*set*](#set) **-b** or [*set*](#set) **-o** *notify*. When set, it notifies the user immediately of background job completions. When unset, this capability is turned off.
>
> The other notification ability comes through the built-in utility *notify*. The syntax is:
>
> ```
> notify [%job ... ]
> ```
>
> By issuing *notify* with no operands, it causes the C shell to notify the user asynchronously when the state of the current job changes. If given operands, *notify* asynchronously informs the user of changes in the states of the specified jobs.
>
> To add asynchronous notification to the POSIX shell, neither the KornShell extensions to [*trap*](#trap), nor the C shell *notify* environment variable seemed appropriate (*notify* is not a proper POSIX environment variable name).
>
> The [*set*](#set) **-b** option was selected as a compromise.
>
> The *notify* built-in was considered to have more functionality than was required for simple asynchronous notification.
>
> Historically, some shells applied the **-u** option to all parameters including `$@` and `$*`. The standard developers felt that this was a misfeature since it is normal and common for `$@` and `$*` to be used in shell scripts regardless of whether they were passed any arguments. Treating these uses as an error when no arguments are passed reduces the value of **-u** for its intended purpose of finding spelling mistakes in variable names and uses of unset positional parameters.

#### FUTURE DIRECTIONS

> A future version of this standard may remove the **-o** *nolog* option.

#### SEE ALSO

> [2.15 Special Built-In Utilities](#215-special-built-in-utilities), [*hash*](docs/posix/md/utilities/hash.md)
>
> XBD [*4.26 Variable Assignment*](docs/posix/md/basedefs/V1_chap04.md#426-variable-assignment), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

#### Issue 6

> The obsolescent [*set*](#set) command name followed by `'-'` has been removed.
>
> The following new requirements on POSIX implementations derive from alignment with the Single UNIX Specification:
>
> - The *nolog* option is added to [*set*](#set) **-o**.
>
> IEEE PASC Interpretation 1003.2 #167 is applied, clarifying that the options default also takes into account the description of the option.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/5 is applied so that the reference page sections use terms as described in the Utility Description Defaults ( [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults)). No change in behavior is intended.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/8 is applied, changing the square brackets in the example in RATIONALE to be in bold, which is the typeface used for optional items.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #027 is applied, clarifying the behavior if the first argument is `'-'`.
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> XSI shading is removed from the **-h** functionality.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0046 [52], XCU/TC1-2008/0047 [155,280], XCU/TC1-2008/0048 [52], XCU/TC1-2008/0049 [52], and XCU/TC1-2008/0050 [155,430] are applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0053 [584], XCU/TC2-2008/0054 [717], XCU/TC2-2008/0055 [717], and XCU/TC2-2008/0056 [960] are applied.

#### Issue 8

> Austin Group Defect 559 is applied, changing the description of the **-u** option.
>
> Austin Group Defect 789 is applied, adding **-o** *pipefail*.
>
> Austin Group Defect 981 is applied, changing the description of the **-o** *nolog* option and the FUTURE DIRECTIONS section.
>
> Austin Group Defects 1009 and 1555 are applied, changing the description of the **-a** option.
>
> Austin Group Defect 1016 is applied, changing the description of the **-C** option.
>
> Austin Group Defect 1055 is applied, adding a paragraph about the **-n** option to the APPLICATION USAGE section.
>
> Austin Group Defect 1063 is applied, changing the description of the **-h** option.
>
> Austin Group Defect 1150 is applied, changing the description of the **-e** option.
>
> Austin Group Defect 1207 is applied, clarifying which option-arguments of the **-o** option are related to the User Portability Utilities option.
>
> Austin Group Defect 1254 is applied, changing the descriptions of the **-b** and **-m** options.
>
> Austin Group Defect 1384 is applied, allowing subshells of interactive shells to ignore the **-n** option.

*End of informative text.*

### Tests

#### Test: set lists all shell variables

`set` with no options or arguments writes all shell variable names
and values.

```
begin test "set lists all shell variables"
  script
    MY_TEST_VAR="hello_set"
    set | grep -q "^MY_TEST_VAR=hello_set$" && echo "found"
  expect
    stdout "found"
    stderr ""
    exit_code 0
end test "set lists all shell variables"
```

#### Test: set -- assigns positional parameters

`set --` followed by arguments assigns positional parameters.

```
begin test "set -- assigns positional parameters"
  script
    set -- a b c
    echo "$1 $2 $3"
  expect
    stdout "a b c"
    stderr ""
    exit_code 0
end test "set -- assigns positional parameters"
```

#### Test: set -- without args clears positional parameters

`set --` without arguments unsets all positional parameters and
sets `$#` to zero.

```
begin test "set -- without args clears positional parameters"
  script
    set -- x y z
    set --
    echo "$#"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "set -- without args clears positional parameters"
```

#### Test: options default to off (-e is off)

The default for all set options is off unless stated otherwise.

```
begin test "options default to off (-e is off)"
  script
    false
    echo "survived"
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "options default to off (-e is off)"
```

#### Test: set output is suitable for reinput with quoting

The output of `set` (listing variables) is formatted so it can be
used as shell input to recreate the same variable assignments, even
when values contain spaces and quotes.

```
begin test "set output is suitable for reinput with quoting"
  script
    my_weird_var="space and literal and \"quotes\""
    output=$(set | grep "^my_weird_var=")
    unset my_weird_var
    eval "$output"
    echo "$my_weird_var"
  expect
    stdout "space and literal and ""quotes"""
    stderr ""
    exit_code 0
end test "set output is suitable for reinput with quoting"
```

#### Test: set -a causes variables to be auto-exported

The `-a` (allexport) option causes all subsequent variable
assignments to be automatically exported to the environment.

```
begin test "set -a causes variables to be auto-exported"
  script
    set -a
    auto_exported="yes"
    env | grep -q "^auto_exported=yes$" && echo "exported"
  expect
    stdout "exported"
    stderr ""
    exit_code 0
end test "set -a causes variables to be auto-exported"
```

#### Test: set -b is accepted without error

The `-b` (notify) option causes the shell to report background
job completions immediately. Setting and unsetting it must succeed.

```
begin test "set -b is accepted without error"
  script
    set -b
    set +b
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "set -b is accepted without error"
```

#### Test: noclobber prevents overwrite with >

When the `-C` (noclobber) option is set, redirection with `>`
shall fail if the target file already exists.

```
begin test "noclobber prevents overwrite with >"
  script
    _f=_noclobber_test
    echo original > $_f
    set -C
    echo overwritten > $_f 2>/dev/null
    cat $_f
  expect
    stdout "original"
    stderr ".+"
    exit_code 0
end test "noclobber prevents overwrite with >"
```

#### Test: >| overrides noclobber

The `>|` operator forces output redirection even when the
noclobber option is set.

```
begin test ">| overrides noclobber"
  script
    _f=_noclobber_test2
    echo original > $_f
    set -C
    echo forced >| $_f
    cat $_f
  expect
    stdout "forced"
    stderr ""
    exit_code 0
end test ">| overrides noclobber"
```

#### Test: errexit exits on command failure

The `-e` (errexit) option causes the shell to exit immediately
when a simple command fails (returns non-zero).

```
begin test "errexit exits on command failure"
  script
    set -e
    echo "start"
    false
    echo "should not run"
  expect
    stdout "start"
    stderr ""
    exit_code !=0
end test "errexit exits on command failure"
```

#### Test: errexit ignores failures in if/while/until and AND-OR lists

With `-e` set, commands that are part of the condition in `if`,
`while`, `until`, or AND-OR lists do not cause the shell to exit
on failure.

```
begin test "errexit ignores failures in if/while/until and AND-OR lists"
  script
    set -e
    if false; then
      echo "no"
    fi
    false || true
    true && false || true
    while false; do
      echo "no"
    done
    echo "survived"
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "errexit ignores failures in if/while/until and AND-OR lists"
```

#### Test: errexit does not exit on ! pipeline

The `!` reserved word negates the exit status of a pipeline. With
`-e` set, `! false` succeeds and the shell does not exit.

```
begin test "errexit does not exit on ! pipeline"
  script
    set -e
    ! false
    echo "survived_not"
  expect
    stdout "survived_not"
    stderr ""
    exit_code 0
end test "errexit does not exit on ! pipeline"
```

#### Test: errexit triggers on pipeline failure (last command)

With `-e` set, the exit status of a pipeline is the exit status
of the last command; if it fails, the shell exits.

```
begin test "errexit triggers on pipeline failure (last command)"
  script
    set -e
    echo ok | false
    echo should_not_appear
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code !=0
end test "errexit triggers on pipeline failure (last command)"
```

#### Test: set -f disables pathname expansion

The `-f` (noglob) option disables pathname expansion, so pattern
characters are treated literally.

```
begin test "set -f disables pathname expansion"
  script
    set -f
    touch tmp_set_f.txt
    echo tmp_set_*.txt
  expect
    stdout "tmp_set_\*.txt"
    stderr ""
    exit_code 0
end test "set -f disables pathname expansion"
```

#### Test: set -m is accepted without error

The `-m` (monitor) option enables job control. Setting it must
not produce an error.

```
begin test "set -m is accepted without error"
  script
    set -m 2>/dev/null
    true
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "set -m is accepted without error"
```

#### Test: set -n suppresses command execution

The `-n` (noexec) option causes the shell to read commands but
not execute them. No output is produced from commands in the
script.

```
begin test "set -n suppresses command execution"
  script
    set -n
    echo "should not run"
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "set -n suppresses command execution"
```

#### Test: nounset exits on unset variable expansion

The `-u` (nounset) option causes the shell to write a diagnostic
message and exit when an unset variable is expanded.

```
begin test "nounset exits on unset variable expansion"
  script
    set -u
    echo "start"
    echo "${this_var_is_definitely_unset}"
    echo "should not run"
  expect
    stdout "start"
    stderr ".+"
    exit_code !=0
end test "nounset exits on unset variable expansion"
```

#### Test: nounset does not trigger on $@ and $*

With `-u` set, expanding `$@` or `$*` when there are no
positional parameters is not an error.

```
begin test "nounset does not trigger on $@ and $*"
  script
    set -u
    for i in "$@"; do
      echo $i
    done
    echo "survived"
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "nounset does not trigger on $@ and $*"
```

#### Test: set -v echoes input to stderr

The `-v` (verbose) option causes the shell to write each input
line to standard error as it is read.

```
begin test "set -v echoes input to stderr"
  script
    set -v
    echo "testing_verbose"
  expect
    stdout "(.|\n)*"
    stderr "(.|\n)*echo.*testing_verbose(.|\n)*"
    exit_code 0
end test "set -v echoes input to stderr"
```

#### Test: set -x traces expanded commands to stderr

The `-x` (xtrace) option causes the shell to write each command
and its arguments to standard error after expansion.

```
begin test "set -x traces expanded commands to stderr"
  script
    set -x
    echo "testing_xtrace"
  expect
    stdout "(.|\n)*"
    stderr "(.|\n)*echo testing_xtrace(.|\n)*"
    exit_code 0
end test "set -x traces expanded commands to stderr"
```

#### Test: set -o succeeds

`set -o` without an option-argument writes the current setting of
all options to standard output.

```
begin test "set -o succeeds"
  script
    set -o 2>/dev/null
    true
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "set -o succeeds"
```

#### Test: set -o allexport exports all assignments

The `allexport` option (equivalent to `-a`) causes all subsequent
variable assignments to be marked for export.

```
begin test "set -o allexport exports all assignments"
  script
    set -o allexport
    ALLEXP_VAR=allexp_val
    env | grep ALLEXP_VAR
  expect
    stdout ".*ALLEXP_VAR=allexp_val.*"
    stderr ""
    exit_code 0
end test "set -o allexport exports all assignments"
```
