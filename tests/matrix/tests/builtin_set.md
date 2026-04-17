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

#### Test: set with no arguments lists shell variables

With no options or arguments, `set` shall write names and values of all
shell variables, each on a separate line in the format `name=value`.

```
begin test "set with no arguments lists shell variables"
  script
    MY_SET_VAR=hello_set
    set | grep -q '^MY_SET_VAR=hello_set$' && printf 'found\n'
  expect
    stdout "found"
    stderr ""
    exit_code 0
end test "set with no arguments lists shell variables"
```

#### Test: set variable listing follows collation sequence

The variable names shall be written in the collation sequence of the
current locale.

```
begin test "set variable listing follows collation sequence"
  script
    ZZ_coltest=z
    AA_coltest=a
    MM_coltest=m
    set | sed -E -n 's/^(AA_coltest|MM_coltest|ZZ_coltest)=.*/\1/p' | uniq
  expect
    stdout "AA_coltest\nMM_coltest\nZZ_coltest"
    stderr ""
    exit_code 0
end test "set variable listing follows collation sequence"
```

#### Test: set output is suitable for reinput

The value strings in `set` output shall be written with appropriate
quoting so the output is suitable for reinput to the shell.

```
begin test "set output is suitable for reinput"
  script
    my_quoted_var="has spaces and \"quotes\""
    line=$(set | grep '^my_quoted_var=')
    unset my_quoted_var
    eval "$line"
    printf '%s\n' "$my_quoted_var"
  expect
    stdout "has spaces and ""quotes"""
    stderr ""
    exit_code 0
end test "set output is suitable for reinput"
```

#### Test: set -- assigns positional parameters and updates sharp

When arguments follow `set --`, they shall be assigned to the
positional parameters in order and `$#` shall reflect the count.

```
begin test "set -- assigns positional parameters and updates sharp"
  script
    set -- a b c
    printf '%s %s %s %s\n' "$1" "$2" "$3" "$#"
  expect
    stdout "a b c 3"
    stderr ""
    exit_code 0
end test "set -- assigns positional parameters and updates sharp"
```

#### Test: set -- unsets old positional parameters first

All positional parameters shall be unset before any new values are
assigned, so old excess parameters do not persist.

```
begin test "set -- unsets old positional parameters first"
  script
    set -- old1 old2 old3
    set -- new1
    printf '1=%s 2=%s sharp=%s\n' "$1" "${2-unset}" "$#"
  expect
    stdout "1=new1 2=unset sharp=1"
    stderr ""
    exit_code 0
end test "set -- unsets old positional parameters first"
```

#### Test: set -- without arguments clears all positional parameters

`set --` with no arguments shall unset all positional parameters and
set `$#` to zero.

```
begin test "set -- without arguments clears all positional parameters"
  script
    set -- x y z
    set --
    printf '%s\n' "$#"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "set -- without arguments clears all positional parameters"
```

#### Test: set -- delimits arguments beginning with plus or minus

The `--` argument shall delimit arguments so that a first argument
beginning with `+` or `-` is treated as a positional parameter, not
as an option.

```
begin test "set -- delimits arguments beginning with plus or minus"
  script
    set -- -x +o foo
    printf '1=%s 2=%s 3=%s sharp=%s\n' "$1" "$2" "$3" "$#"
  expect
    stdout "1=\-x 2=\+o 3=foo sharp=3"
    stderr ""
    exit_code 0
end test "set -- delimits arguments beginning with plus or minus"
```

#### Test: options and positional parameters combined

Setting or unsetting attributes and positional parameters can be
combined in a single invocation of `set`.

```
begin test "options and positional parameters combined"
  script
    set -f a b c
    printf '1=%s sharp=%s glob=%s\n' "$1" "$#" "$(printf '%s\n' *.nonexistent)"
  expect
    stdout "1=a sharp=3 glob=\*.nonexistent"
    stderr ""
    exit_code 0
end test "options and positional parameters combined"
```

#### Test: plus form disables an option

Options specified with a leading `+` shall disable the corresponding
option that was previously enabled with `-`.

```
begin test "plus form disables an option"
  script
    set -f
    set +f
    touch _set_plusf_probe.txt
    printf '%s\n' _set_plusf_probe*
    rm -f _set_plusf_probe.txt
  expect
    stdout "_set_plusf_probe.txt"
    stderr ""
    exit_code 0
end test "plus form disables an option"
```

#### Test: set -a causes variables to be auto-exported

When `-a` is set, every subsequent variable assignment in the current
shell execution environment shall have the export attribute set.

```
begin test "set -a causes variables to be auto-exported"
  script
    set -a
    auto_exported=yes
    sh -c 'printf "%s\n" "$auto_exported"'
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "set -a causes variables to be auto-exported"
```

#### Test: set -a applies to side-effect assignments

The `-a` export attribute shall apply to all forms of assignment,
including those made as a side-effect of variable expansions.

```
begin test "set -a applies to side-effect assignments"
  script
    set -a
    : ${sideeffect_set_var=default_val}
    sh -c 'printf "%s\n" "$sideeffect_set_var"'
  expect
    stdout "default_val"
    stderr ""
    exit_code 0
end test "set -a applies to side-effect assignments"
```

#### Test: set -b is accepted

The `-b` (notify) option shall be supported. Setting and unsetting it
shall succeed without error.

```
begin test "set -b is accepted"
  script
    set -b
    set +b
    printf 'ok\n'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "set -b is accepted"
```

#### Test: set -m is accepted

The `-m` (monitor) option shall be supported. Setting it in a
non-interactive script shall not produce an error.

```
begin test "set -m is accepted"
  script
    set -m 2>/dev/null
    printf 'ok\n'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "set -m is accepted"
```

#### Test: set -C prevents overwriting existing regular files

When `-C` is set, the `>` redirection operator shall not overwrite
an existing regular file.

```
begin test "set -C prevents overwriting existing regular files"
  script
    tmp=$(mktemp)
    printf 'original\n' > "$tmp"
    set -C
    { printf 'overwritten\n' > "$tmp"; } 2>/dev/null
    cat "$tmp"
    rm -f "$tmp"
  expect
    stdout "original"
    stderr ""
    exit_code 0
end test "set -C prevents overwriting existing regular files"
```

#### Test: noclobber only protects regular files

The `-C` option prevents overwriting existing *regular files*; special
files such as `/dev/null` shall not be blocked.

```
begin test "noclobber only protects regular files"
  script
    set -C
    printf 'ok\n' > /dev/null
    printf 'status=%s\n' "$?"
  expect
    stdout "status=0"
    stderr ""
    exit_code 0
end test "noclobber only protects regular files"
```

#### Test: clobber operator overrides noclobber

The `>|` operator shall override the noclobber option for an individual
file.

```
begin test "clobber operator overrides noclobber"
  script
    tmp=$(mktemp)
    printf 'original\n' > "$tmp"
    set -C
    printf 'forced\n' >| "$tmp"
    cat "$tmp"
    rm -f "$tmp"
  expect
    stdout "forced"
    stderr ""
    exit_code 0
end test "clobber operator overrides noclobber"
```

#### Test: errexit exits on simple command failure

When `-e` is set, the shell shall exit immediately when any command
fails by returning a non-zero exit status.

```
begin test "errexit exits on simple command failure"
  script
    set -e
    printf 'start\n'
    false
    printf 'should not run\n'
  expect
    stdout "start"
    stderr ""
    exit_code !=0
end test "errexit exits on simple command failure"
```

#### Test: errexit preserves failing command exit status

When `-e` causes the shell to exit, it shall exit as if by executing
`exit` with no arguments, meaning the exit status is that of the
failing command.

```
begin test "errexit preserves failing command exit status"
  script
    set -e
    (exit 42)
    printf 'should not run\n'
  expect
    stdout ""
    stderr ""
    exit_code 42
end test "errexit preserves failing command exit status"
```

#### Test: errexit ignored in if, elif, while, and until conditions

The `-e` setting shall be ignored when executing the compound list
following `if`, `elif`, `while`, or `until`.

```
begin test "errexit ignored in if, elif, while, and until conditions"
  script
    set -e
    if false; then echo no; fi
    if true; then true; elif false; then echo no; fi
    while false; do echo no; done
    until true; do echo no; done
    printf 'survived\n'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "errexit ignored in if, elif, while, and until conditions"
```

#### Test: errexit ignored in negated pipeline

The `-e` setting shall be ignored for a pipeline beginning with the
`!` reserved word.

```
begin test "errexit ignored in negated pipeline"
  script
    set -e
    ! false
    printf 'survived\n'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "errexit ignored in negated pipeline"
```

#### Test: errexit ignored in non-last AND-OR list commands

The `-e` setting shall be ignored for any command of an AND-OR list
other than the last.

```
begin test "errexit ignored in non-last AND-OR list commands"
  script
    set -e
    false || true
    true && false || true
    printf 'survived\n'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "errexit ignored in non-last AND-OR list commands"
```

#### Test: errexit triggers on pipeline failure

The failure of the pipeline itself (not individual commands in a
multi-command pipeline) shall be considered under `-e`.

```
begin test "errexit triggers on pipeline failure"
  script
    set -e
    printf 'ok\n' | false
    printf 'should not run\n'
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "errexit triggers on pipeline failure"
```

#### Test: errexit applies separately to subshell environments

The `-e` requirement applies to each subshell environment separately.
A `false` in a subshell exits that subshell, but the pipeline exit
status determines the parent shell's fate.

```
begin test "errexit applies separately to subshell environments"
  script
    set -e
    (false; echo one) | cat
    printf 'two\n'
  expect
    stdout "two"
    stderr ""
    exit_code 0
end test "errexit applies separately to subshell environments"
```

#### Test: errexit ignores command substitution subshell failure

Subshell environments created for command substitution during word
expansion are exempt from causing the parent shell to exit under `-e`.

```
begin test "errexit ignores command substitution subshell failure"
  script
    set -e
    printf '%s two\n' "$(false; echo one)"
  expect
    stdout " two"
    stderr ""
    exit_code 0
end test "errexit ignores command substitution subshell failure"
```

#### Test: errexit exception 3 for compound command in ignored context

If the exit status of a compound command (other than a subshell) was the
result of a failure while `-e` was being ignored, `-e` shall not apply
to that command.

```
begin test "errexit exception 3 for compound command in ignored context"
  script
    set -e
    f() { false; printf 'inner\n'; }
    f || printf 'caught\n'
    printf 'after\n'
  expect
    stdout "inner\nafter"
    stderr ""
    exit_code 0
end test "errexit exception 3 for compound command in ignored context"
```

#### Test: set -f disables pathname expansion

When `-f` is set, the shell shall disable pathname expansion.

```
begin test "set -f disables pathname expansion"
  script
    set -f
    printf '%s\n' tmp_set_f_*
  expect
    stdout "tmp_set_f_\*"
    stderr ""
    exit_code 0
end test "set -f disables pathname expansion"
```

#### Test: set -n suppresses command execution

When `-n` is set, the shell shall read commands but not execute them.

```
begin test "set -n suppresses command execution"
  script
    set -n
    printf 'should not run\n'
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "set -n suppresses command execution"
```

#### Test: nounset fails on unset parameter expansion

When `-u` is set, expanding an unset parameter in a parameter expansion
shall write a diagnostic to stderr and the expansion shall fail.

```
begin test "nounset fails on unset parameter expansion"
  script
    set -u
    printf 'start\n'
    printf '%s\n' "${this_is_definitely_unset}"
    printf 'should not run\n'
  expect
    stdout "start"
    stderr ".+"
    exit_code !=0
end test "nounset fails on unset parameter expansion"
```

#### Test: nounset fails on unset arithmetic expansion

The `-u` option shall also apply to arithmetic expansions that reference
unset parameters.

```
begin test "nounset fails on unset arithmetic expansion"
  script
    set -u
    printf 'start\n'
    : $((unset_arith_var_xyz + 1))
    printf 'should not run\n'
  expect
    stdout "start"
    stderr ".+"
    exit_code !=0
end test "nounset fails on unset arithmetic expansion"
```

#### Test: nounset does not trigger on at and star

The `@` and `*` special parameters are explicitly excluded from the
`-u` check even when no positional parameters are set.

```
begin test "nounset does not trigger on at and star"
  script
    set --
    set -u
    for i in "$@"; do printf '%s\n' "$i"; done
    printf '%s\n' "$*"
    printf 'survived\n'
  expect
    stdout "\nsurvived"
    stderr ""
    exit_code 0
end test "nounset does not trigger on at and star"
```

#### Test: set -v writes input to stderr

When `-v` is set, the shell shall write its input to standard error as
it is read.

```
begin test "set -v writes input to stderr"
  script
    set -v
    printf 'testing_verbose\n'
  expect
    stdout "testing_verbose"
    stderr "(.|\n)*printf.*testing_verbose(.|\n)*"
    exit_code 0
end test "set -v writes input to stderr"
```

#### Test: set -x traces expanded commands to stderr

When `-x` is set, the shell shall write to stderr a trace for each
command after it expands the command and before it executes it. The
trace shall show the expanded form, not the unexpanded source.

```
begin test "set -x traces expanded commands to stderr"
  script
    myvar=expanded_val
    set -x
    printf '%s\n' "$myvar"
  expect
    stdout "expanded_val"
    stderr "(.|\n)*printf.*expanded_val(.|\n)*"
    exit_code 0
end test "set -x traces expanded commands to stderr"
```

#### Test: set -o writes current option settings

`set -o` without an option-argument shall write the current settings of
all options to standard output.

```
begin test "set -o writes current option settings"
  script
    set -o | grep -q 'errexit' && printf 'found\n'
  expect
    stdout "found"
    stderr ""
    exit_code 0
end test "set -o writes current option settings"
```

#### Test: set +o writes restorable option settings

`set +o` shall write the current option settings in a format suitable
for reinput to the shell to achieve the same settings. A save/restore
roundtrip shall undo multiple option changes.

```
begin test "set +o writes restorable option settings"
  script
    saved=$(set +o)
    set -e -f -u
    eval "$saved"
    false
    touch _set_restore_x1.tmp
    printf '%s\n' _set_restore_x1*
    rm -f _set_restore_x1.tmp
    printf 'survived\n'
  expect
    stdout "_set_restore_x1.tmp\nsurvived"
    stderr ""
    exit_code 0
end test "set +o writes restorable option settings"
```

#### Test: -o allexport is equivalent to -a

`set -o allexport` shall be equivalent to `set -a`.

```
begin test "-o allexport is equivalent to -a"
  script
    set -o allexport
    olong_exp_var=yes
    sh -c 'printf "%s\n" "$olong_exp_var"'
  expect
    stdout "yes"
    stderr ""
    exit_code 0
end test "-o allexport is equivalent to -a"
```

#### Test: -o errexit is equivalent to -e

`set -o errexit` shall be equivalent to `set -e`.

```
begin test "-o errexit is equivalent to -e"
  script
    set -o errexit
    printf 'start\n'
    false
    printf 'should not run\n'
  expect
    stdout "start"
    stderr ""
    exit_code !=0
end test "-o errexit is equivalent to -e"
```

#### Test: -o noclobber is equivalent to -C

`set -o noclobber` shall be equivalent to `set -C`.

```
begin test "-o noclobber is equivalent to -C"
  script
    tmp=$(mktemp)
    printf 'original\n' > "$tmp"
    set -o noclobber
    { printf 'overwritten\n' > "$tmp"; } 2>/dev/null
    cat "$tmp"
    rm -f "$tmp"
  expect
    stdout "original"
    stderr ""
    exit_code 0
end test "-o noclobber is equivalent to -C"
```

#### Test: -o noglob is equivalent to -f

`set -o noglob` shall be equivalent to `set -f`.

```
begin test "-o noglob is equivalent to -f"
  script
    set -o noglob
    printf '%s\n' noglob_probe_*
  expect
    stdout "noglob_probe_\*"
    stderr ""
    exit_code 0
end test "-o noglob is equivalent to -f"
```

#### Test: -o noexec is equivalent to -n

`set -o noexec` shall be equivalent to `set -n`.

```
begin test "-o noexec is equivalent to -n"
  script
    set -o noexec
    printf 'should not run\n'
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "-o noexec is equivalent to -n"
```

#### Test: -o nounset is equivalent to -u

`set -o nounset` shall be equivalent to `set -u`.

```
begin test "-o nounset is equivalent to -u"
  script
    set -o nounset
    printf '%s\n' "${definitely_unset_olong}"
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "-o nounset is equivalent to -u"
```

#### Test: -o verbose is equivalent to -v

`set -o verbose` shall be equivalent to `set -v`.

```
begin test "-o verbose is equivalent to -v"
  script
    set -o verbose
    printf 'verbose_probe\n'
  expect
    stdout "verbose_probe"
    stderr "(.|\n)*printf.*verbose_probe(.|\n)*"
    exit_code 0
end test "-o verbose is equivalent to -v"
```

#### Test: -o xtrace is equivalent to -x

`set -o xtrace` shall be equivalent to `set -x`.

```
begin test "-o xtrace is equivalent to -x"
  script
    set -o xtrace
    printf 'xtrace_probe\n'
  expect
    stdout "xtrace_probe"
    stderr "(.|\n)*printf.*xtrace_probe(.|\n)*"
    exit_code 0
end test "-o xtrace is equivalent to -x"
```

#### Test: -o pipefail derives exit status from all pipeline commands

When `pipefail` is set, the exit status of a pipeline shall be derived
from the exit statuses of all commands in the pipeline, not just the
last.

```
begin test "-o pipefail derives exit status from all pipeline commands"
  script
    set -o pipefail
    false | true
    printf '%s\n' "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "-o pipefail derives exit status from all pipeline commands"
```

#### Test: +o option-name disables a long-name option

The `+o` form with an option name shall disable that option.

```
begin test "+o option-name disables a long-name option"
  script
    set -o errexit
    set +o errexit
    false
    printf 'survived\n'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "+o option-name disables a long-name option"
```

#### Test: options default to off

The default for all options shall be off unless stated otherwise in the
description of the option.

```
begin test "options default to off"
  script
    false
    printf 'survived\n'
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "options default to off"
```

#### Test: set -h is accepted

The `-h` option is reserved for speeding up PATH lookups; the shell must
accept turning it on and off without error.

```
begin test "set -h is accepted"
  script
    set -h
    set +h
    printf 'ok\n'
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "set -h is accepted"
```

#### Test: invalid option produces non-zero exit status

An invalid option shall cause `set` to exit with a non-zero status.
Because `set` is a special built-in, an option error may cause the
enclosing script to exit, so we test in a subshell.

```
begin test "invalid option produces non-zero exit status"
  script
    (set -Z) 2>/dev/null
    printf '%s\n' "$?"
  expect
    stdout "[1-9][0-9]*"
    stderr ""
    exit_code 0
end test "invalid option produces non-zero exit status"
```
