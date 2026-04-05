# Test Suite for Intrinsic Utility: getopts

This test suite covers the **getopts** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: getopts](#utility-getopts)

## utility: getopts

#### NAME

> getopts — parse utility options

#### SYNOPSIS

> `getopts optstring name [param...]`

#### DESCRIPTION

> The *getopts* utility shall retrieve options and option-arguments from a list of parameters. It shall support the Utility Syntax Guidelines 3 to 10, inclusive, described in XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> When the shell is first invoked, the shell variable *OPTIND* shall be initialized to 1. Each time *getopts* is invoked, it shall place the value of the next option found in the parameter list in the shell variable specified by the *name* operand and the shell variable *OPTIND* shall be set as follows:
>
> - When *getopts* successfully parses an option that takes an option-argument (that is, a character followed by `<colon>` in *optstring*, and exit status is 0), the value of *OPTIND* shall be the integer index of the next element of the parameter list (if any; see OPERANDS below) to be searched for an option character. Index 1 identifies the first element of the parameter list.
> - When *getopts* reports end of options (that is, when exit status is 1), the value of *OPTIND* shall be the integer index of the next element of the parameter list (if any).
> - In all other cases, the value of *OPTIND* is unspecified, but shall encode the information needed for the next invocation of *getopts* to resume parsing options after the option just parsed.
>
> When the option requires an option-argument, the *getopts* utility shall place it in the shell variable *OPTARG .* If no option was found, or if the option that was found does not have an option-argument, *OPTARG* shall be unset.
>
> If an option character not contained in the *optstring* operand is found where an option character is expected, the shell variable specified by *name* shall be set to the `<question-mark>` (`'?'`) character. In this case, if the first character in *optstring* is a `<colon>` (`':'`), the shell variable *OPTARG* shall be set to the option character found, but no output shall be written to standard error; otherwise, the shell variable *OPTARG* shall be unset and a diagnostic message shall be written to standard error. This condition shall be considered to be an error detected in the way arguments were presented to the invoking application, but shall not be an error in *getopts* processing.
>
> If an option-argument is missing:
>
> - If the first character of *optstring* is a `<colon>`, the shell variable specified by *name* shall be set to the `<colon>` character and the shell variable *OPTARG* shall be set to the option character found.
> - Otherwise, the shell variable specified by *name* shall be set to the `<question-mark>` character, the shell variable *OPTARG* shall be unset, and a diagnostic message shall be written to standard error. This condition shall be considered to be an error detected in the way arguments were presented to the invoking application, but shall not be an error in *getopts* processing; a diagnostic message shall be written as stated, but the exit status shall be zero.
>
> When the end of options is encountered, the *getopts* utility shall exit with a return value of one; the shell variable *OPTIND* shall be set to the index of the argument containing the first operand in the parameter list, or the value 1 plus the number of elements in the parameter list if there are no operands in the parameter list; the *name* variable shall be set to the `<question-mark>` character. Any of the following shall identify the end of options: the first `"--"` element of the parameter list that is not an option-argument, finding an element of the parameter list that is not an option-argument and does not begin with a `'-'`, or encountering an error.
>
> The shell variables *OPTIND* and *OPTARG* shall not be exported by default. An error in setting any of these variables (such as if *name* has previously been marked *readonly*) shall be considered an error of *getopts* processing, and shall result in a return value greater than one.
>
> The *getopts* utility can affect *OPTIND ,* *OPTARG ,* and the shell variable specified by the *name* operand, within the current shell execution environment; see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment).
>
> If the application sets *OPTIND* to the value 1, a new set of parameters can be used: either the current positional parameters or new *param* values. Any other attempt to invoke *getopts* multiple times in a single shell execution environment with parameters (positional parameters or *param* operands) that are not the same in all invocations, or with an *OPTIND* value modified by the application to be a value other than 1, produces unspecified results.

#### OPTIONS

> None.

#### OPERANDS

> The following operands shall be supported:
>
> - *optstring*: A string containing the option characters recognized by the utility invoking *getopts*. If a character is followed by a `<colon>`, the option shall be expected to have an argument, which should be supplied as a separate argument. Applications should specify an option character and its option-argument as separate arguments, but *getopts* shall interpret the characters following an option character requiring arguments as an argument whether or not this is done. An explicit null option-argument need not be recognized if it is not supplied as a separate argument when *getopts* is invoked. (See also the [*getopt*()](docs/posix/md/functions/getopt.md) function defined in the System Interfaces volume of POSIX.1-2024.) The characters `<question-mark>` and `<colon>` shall not be used as option characters by an application. The use of other option characters that are not alphanumeric produces unspecified results. Whether or not the option-argument is supplied as a separate argument from the option character, the value in *OPTARG* shall only be the characters of the option-argument. The first character in *optstring* determines how *getopts* behaves if an option character is not known or an option-argument is missing.
> - *name*: The name of a shell variable that shall be set by the *getopts* utility to the option character that was found.
>
> By default, the list of parameters parsed by the *getopts* utility shall be the positional parameters currently set in the invoking shell environment (`"$@"`). If *param* operands are given, they shall be parsed instead of the positional parameters. Note that the next element of the parameter list need not exist; in this case, *OPTIND* will be set to `$#+1` or the number of *param* operands plus 1.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *getopts*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments and input files).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *OPTIND*: This variable shall be used by the *getopts* utility as the index of the next argument to be processed.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> Not used.

#### STDERR

> Whenever an error is detected and the first character in the *optstring* operand is not a `<colon>` (`':'`), a diagnostic message shall be written to standard error with the following information in an unspecified format:
>
> - The invoking program name shall be identified in the message. The invoking program name shall be the value of the shell special parameter 0 (see [*2.5.2 Special Parameters*](docs/posix/md/utilities/V3_chap02.md#252-special-parameters)) at the time the *getopts* utility is invoked. A name equivalent to: may be used.
>   ```
>   basename "$0"
>   ```
> - If an option is found that was not specified in *optstring*, this error is identified and the invalid option character shall be identified in the message.
> - If an option requiring an option-argument is found, but an option-argument is not found, this error shall be identified and the invalid option character shall be identified in the message.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: An option, specified or unspecified by *optstring*, was found.
> - 1: The end of options was encountered.
> - \>1: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *getopts* affects the current shell execution environment, it is generally provided as a shell regular built-in. If it is called in a subshell or separate utility execution environment, such as one of the following:
>
> ```
> (getopts abc value "$@")
> nohup getopts ...
> find . -exec getopts ... \;
> ```
>
> it does not affect the shell variables in the caller's environment.
>
> Note that shell functions share *OPTIND* with the calling shell even though the positional parameters are changed. If the calling shell and any of its functions uses *getopts* to parse arguments, the results are unspecified.

#### EXAMPLES

> The following example script parses and displays its arguments:
>
> ```
> aflag=
> bflag=
> while getopts ab: name
> do
>     case $name in
>     a)    aflag=1;;
>     b)    bflag=1
>           bval="$OPTARG";;
>     ?)   printf "Usage: %s: [-a] [-b value] args\n" $0
>           exit 2;;
>     esac
> done
> if [ -n "$aflag" ]; then
>     printf "Option -a specified\n"
> fi
> if [ -n "$bflag" ]; then
>     printf 'Option -b "%s" specified\n' "$bval"
> fi
> shift $(($OPTIND - 1))
> printf "Remaining arguments are: %s\n" "$*"
> ```

#### RATIONALE

> ```
> >
> "%s: option requires an argument -- %c\n", <
> ```

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2.5.2 Special Parameters*](docs/posix/md/utilities/V3_chap02.md#252-special-parameters)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*getopt*()](docs/posix/md/functions/getopt.md)

#### CHANGE HISTORY

> First released in Issue 4.

#### Issue 6

> The normative text is reworded to avoid use of the term "must" for application requirements.

#### Issue 7

> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0092 [159] is applied.

#### Issue 8

> Austin Group Defect 191 is applied, adding a paragraph about leading `<plus-sign>` to the RATIONALE section.
>
> Austin Group Defect 367 is applied, requiring that *getopts* distinguishes between encountering the end of options and an error occurring, setting its exit status to one and greater than one, respectively.
>
> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1442 is applied, changing the EXAMPLES section.
>
> Austin Group Defect 1784 is applied, clarifying several aspects of *getopts* behavior and changing the value of *OPTIND* to be unspecified in some circumstances.

*End of informative text.*

### Tests

#### Test: getopts retrieves options from parameter list

`getopts` retrieves options and option-arguments from a list of
parameters.

```
begin test "getopts retrieves options from parameter list"
  script
    $SHELL -c 'OPTIND=1; set -- -a; getopts a name; echo "$name"'
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "getopts retrieves options from parameter list"
```

#### Test: getopts loop extracts each option in turn

Repeated calls to `getopts` extract successive options.

```
begin test "getopts loop extracts each option in turn"
  script
    $SHELL -c 'OPTIND=1; set -- -a -b -c; result=""; while getopts abc name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "getopts loop extracts each option in turn"
```

#### Test: OPTARG set to option-argument value

When an option requires an argument, OPTARG is set to the
option-argument.

```
begin test "OPTARG set to option-argument value"
  script
    $SHELL -c 'OPTIND=1; set -- -f myfile; getopts f: name; echo "$OPTARG"'
  expect
    stdout "myfile"
    stderr ""
    exit_code 0
end test "OPTARG set to option-argument value"
```

#### Test: invalid option sets name to question mark

When an option character not in optstring is found, `name` is set
to `?`.

```
begin test "invalid option sets name to question mark"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts ab name 2>/dev/null; echo "$name"'
  expect
    stdout "[?]"
    stderr ""
    exit_code 0
end test "invalid option sets name to question mark"
```

#### Test: getopts returns non-zero when options exhausted

`getopts` returns a non-zero value when all options have been
processed.

```
begin test "getopts returns non-zero when options exhausted"
  script
    $SHELL -c 'OPTIND=1; set -- -a; getopts a name; getopts a name; echo "$?"'
  expect
    stdout "[1-9].*"
    stderr ""
    exit_code 0
end test "getopts returns non-zero when options exhausted"
```

#### Test: getopts sets name to option character

Verifies that `getopts` stores the matched option letter in the specified shell variable. When `-a` is passed and `a` is in the optstring, `name` must be set to `a`.

```
begin test "getopts sets name to option character"
  script
    $SHELL -c 'OPTIND=1; getopts ab: name -a; echo "$name"'
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "getopts sets name to option character"
```

#### Test: option with colon expects argument

Verifies that when an option character is followed by a colon in the optstring, `getopts` treats it as requiring an argument and still sets `name` to the option character itself.

```
begin test "option with colon expects argument"
  script
    $SHELL -c 'OPTIND=1; set -- -f myfile; getopts f: name; echo "$name"'
  expect
    stdout "f"
    stderr ""
    exit_code 0
end test "option with colon expects argument"
```

#### Test: OPTARG set to option-argument value (separate word)

Verifies that when an option-argument is given as a separate word (e.g. `-f myfile.txt`), `OPTARG` is set to that argument value.

```
begin test "OPTARG set to option-argument value (separate word)"
  script
    $SHELL -c 'OPTIND=1; set -- -f myfile.txt; getopts f: name; echo "$OPTARG"'
  expect
    stdout "myfile.txt"
    stderr ""
    exit_code 0
end test "OPTARG set to option-argument value (separate word)"
```

#### Test: OPTARG set to option-argument value (concatenated)

Verifies that `getopts` interprets the characters immediately following an option letter as its argument when concatenated in a single word (e.g. `-fmyfile.txt`), setting `OPTARG` accordingly.

```
begin test "OPTARG set to option-argument value (concatenated)"
  script
    $SHELL -c 'OPTIND=1; set -- -fmyfile.txt; getopts f: name; echo "$OPTARG"'
  expect
    stdout "myfile.txt"
    stderr ""
    exit_code 0
end test "OPTARG set to option-argument value (concatenated)"
```

#### Test: OPTIND initialized to 1

Verifies that when the shell is first invoked, `OPTIND` is initialized to 1, as required by POSIX.

```
begin test "OPTIND initialized to 1"
  script
    $SHELL -c 'echo "$OPTIND"'
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "OPTIND initialized to 1"
```

#### Test: OPTIND updated after first option

Verifies that after `getopts` successfully parses one option from a two-option list, `OPTIND` advances to 2, pointing to the next element to be processed.

```
begin test "OPTIND updated after first option"
  script
    $SHELL -c 'OPTIND=1; set -- -a -b; getopts ab name; echo "$OPTIND"'
  expect
    stdout "2"
    stderr ""
    exit_code 0
end test "OPTIND updated after first option"
```

#### Test: OPTIND updated after two options

Verifies that after `getopts` processes both options in a two-option list, `OPTIND` advances to 3, pointing past all parsed arguments.

```
begin test "OPTIND updated after two options"
  script
    $SHELL -c 'OPTIND=1; set -- -a -b; getopts ab name; getopts ab name; echo "$OPTIND"'
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "OPTIND updated after two options"
```

#### Test: OPTIND after option with separate argument

Verifies that when an option takes an argument as a separate word (e.g. `-f val`), `OPTIND` advances past both the option and its argument, resulting in a value of 3.

```
begin test "OPTIND after option with separate argument"
  script
    $SHELL -c 'OPTIND=1; set -- -f val; getopts f: name; echo "$OPTIND"'
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "OPTIND after option with separate argument"
```

#### Test: OPTIND and OPTARG are not exported by default

Verifies that `OPTIND` and `OPTARG` are not present in the exported environment by default, as POSIX requires these variables to not be exported unless explicitly done so.

```
begin test "OPTIND and OPTARG are not exported by default"
  script
    $SHELL -c 'env | grep -c "^OPTIND\|^OPTARG"; echo $?'
  expect
    stdout "0\n1"
    stderr ""
    exit_code 0
end test "OPTIND and OPTARG are not exported by default"
```

#### Test: invalid option sets name to ?

Verifies that when an unrecognized option character is encountered (without silent mode), `name` is set to `?` and a diagnostic message is written to stderr.

```
begin test "invalid option sets name to ?"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts ab name; echo "$name"'
  expect
    stdout "\?"
    stderr ".+"
    exit_code 0
end test "invalid option sets name to ?"
```

#### Test: invalid option in silent mode sets OPTARG to offending char

Verifies that in silent mode (optstring begins with `:`), when an unrecognized option is encountered, `OPTARG` is set to the offending option character and no diagnostic is written to stderr.

```
begin test "invalid option in silent mode sets OPTARG to offending char"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts :ab name; echo "$OPTARG"'
  expect
    stdout "z"
    stderr ""
    exit_code 0
end test "invalid option in silent mode sets OPTARG to offending char"
```

#### Test: missing argument sets name to ?

Verifies that when an option requiring an argument is given without one (non-silent mode), `name` is set to `?` and a diagnostic message is written to stderr.

```
begin test "missing argument sets name to ?"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts f: name; echo "$name"'
  expect
    stdout "\?"
    stderr ".+"
    exit_code 0
end test "missing argument sets name to ?"
```

#### Test: missing argument in silent mode sets name to colon

Verifies that in silent mode (optstring begins with `:`), when a required option-argument is missing, `name` is set to `:` and `OPTARG` is set to the option character that was missing its argument.

```
begin test "missing argument in silent mode sets name to colon"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts :f: name; echo "$name:$OPTARG"'
  expect
    stdout "::f"
    stderr ""
    exit_code 0
end test "missing argument in silent mode sets name to colon"
```

#### Test: invalid option still returns exit status 0

Verifies that encountering an unrecognized option is not treated as an error in `getopts` processing itself — the exit status remains 0, since the error is in how arguments were presented to the application, not in `getopts`.

```
begin test "invalid option still returns exit status 0"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts ab name 2>/dev/null; echo $?'
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "invalid option still returns exit status 0"
```

#### Test: missing argument still returns exit status 0

Verifies that a missing option-argument is not a `getopts` processing error — the exit status is still 0. POSIX considers this an error in the way arguments were presented, not in `getopts` itself.

```
begin test "missing argument still returns exit status 0"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts f: name 2>/dev/null; echo $?'
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "missing argument still returns exit status 0"
```

#### Test: OPTIND reset allows reparsing

Verifies that setting `OPTIND` back to 1 allows a new set of parameters to be parsed from scratch, as POSIX permits restarting option processing this way.

```
begin test "OPTIND reset allows reparsing"
  script
    $SHELL -c 'OPTIND=1; set -- -a -b; result=""; while getopts ab name; do result="${result}${name}"; done; OPTIND=1; set -- -x -y; while getopts xy name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "abxy"
    stderr ""
    exit_code 0
end test "OPTIND reset allows reparsing"
```

#### Test: -- terminates option processing

Verifies that encountering `--` in the parameter list ends option processing, so subsequent arguments (even those beginning with `-`) are treated as operands.

```
begin test "-- terminates option processing"
  script
    $SHELL -c 'OPTIND=1; result=""; set -- -a -- -b; while getopts ab name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "-- terminates option processing"
```

#### Test: OPTIND points past -- to operand

Verifies that after option processing is terminated by `--`, `OPTIND` points to the first operand following `--`, so that `shift $((OPTIND - 1))` correctly removes all options and the `--` delimiter.

```
begin test "OPTIND points past -- to operand"
  script
    $SHELL -c 'OPTIND=1; set -- -a -- operand; while getopts a name; do :; done; shift $((OPTIND - 1)); echo "$1"'
  expect
    stdout "operand"
    stderr ""
    exit_code 0
end test "OPTIND points past -- to operand"
```

#### Test: getopts returns 0 on successful parse

Verifies that `getopts` returns exit status 0 when it successfully finds an option in the parameter list.

```
begin test "getopts returns 0 on successful parse"
  script
    $SHELL -c 'OPTIND=1; set -- -a; getopts a name'
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "getopts returns 0 on successful parse"
```

#### Test: getopts returns non-zero with no options

Verifies that `getopts` returns a non-zero exit status (1) when the parameter list is empty and there are no options to process, signaling end-of-options.

```
begin test "getopts returns non-zero with no options"
  script
    $SHELL -c 'OPTIND=1; set -- ; getopts a name'
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "getopts returns non-zero with no options"
```

#### Test: name set to ? when options exhausted

Verifies that when all options have been consumed, `getopts` sets `name` to `?` to indicate end-of-options, as required by POSIX.

```
begin test "name set to ? when options exhausted"
  script
    $SHELL -c 'OPTIND=1; set -- -a; getopts a name; getopts a name; echo "$name"'
  expect
    stdout "\?"
    stderr ""
    exit_code 0
end test "name set to ? when options exhausted"
```

#### Test: unknown option in silent mode: name=? OPTARG=char

Verifies the combined behavior in silent mode for an unknown option: `name` is set to `?` and `OPTARG` is set to the unrecognized option character, allowing the application to handle the error programmatically.

```
begin test "unknown option in silent mode: name=? OPTARG=char"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts :ab name; echo "$name:$OPTARG"'
  expect
    stdout "\?:z"
    stderr ""
    exit_code 0
end test "unknown option in silent mode: name=? OPTARG=char"
```

#### Test: no stderr for unknown option in silent mode

Verifies that when the optstring begins with `:` (silent mode), no diagnostic message is written to stderr for an unrecognized option — the application is expected to handle the error itself.

```
begin test "no stderr for unknown option in silent mode"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts :ab name' 2>&1
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "no stderr for unknown option in silent mode"
```

#### Test: missing argument in silent mode: name=: OPTARG=char

Verifies the combined silent-mode behavior when a required argument is missing: `name` is set to `:` and `OPTARG` is set to the option character whose argument was absent.

```
begin test "missing argument in silent mode: name=: OPTARG=char"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts :f: name; echo "$name:$OPTARG"'
  expect
    stdout "::f"
    stderr ""
    exit_code 0
end test "missing argument in silent mode: name=: OPTARG=char"
```

#### Test: no stderr for missing argument in silent mode

Verifies that in silent mode, no diagnostic message is written to stderr when a required option-argument is missing. Silent mode suppresses all stderr diagnostics from `getopts`.

```
begin test "no stderr for missing argument in silent mode"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts :f: name' 2>&1
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "no stderr for missing argument in silent mode"
```

#### Test: diagnostic message written to stderr for unknown option

Verifies that in normal (non-silent) mode, `getopts` writes a diagnostic message to stderr when an option character not in the optstring is encountered.

```
begin test "diagnostic message written to stderr for unknown option"
  script
    $SHELL -c 'OPTIND=1; set -- -z; getopts ab name'
  expect
    stdout ""
    stderr ".+"
    exit_code 0
end test "diagnostic message written to stderr for unknown option"
```

#### Test: diagnostic message written to stderr for missing argument

Verifies that in normal (non-silent) mode, `getopts` writes a diagnostic message to stderr when an option that requires an argument is given without one.

```
begin test "diagnostic message written to stderr for missing argument"
  script
    $SHELL -c 'OPTIND=1; set -- -f; getopts f: name'
  expect
    stdout ""
    stderr ".+"
    exit_code 0
end test "diagnostic message written to stderr for missing argument"
```

#### Test: OPTIND=1 reset parses new parameters

Verifies that assigning `OPTIND=1` between two separate `getopts` invocations with different positional parameters allows each set to be parsed independently.

```
begin test "OPTIND=1 reset parses new parameters"
  script
    $SHELL -c 'OPTIND=1; set -- -x; getopts x name; r1="$name"; OPTIND=1; set -- -y; getopts y name; r2="$name"; echo "${r1}:${r2}"'
  expect
    stdout "x:y"
    stderr ""
    exit_code 0
end test "OPTIND=1 reset parses new parameters"
```

#### Test: combined options after single hyphen

Verifies that multiple option characters grouped after a single hyphen (e.g. `-abc`) are each parsed individually by successive `getopts` calls, per Utility Syntax Guideline 5.

```
begin test "combined options after single hyphen"
  script
    $SHELL -c 'OPTIND=1; result=""; set -- -abc; while getopts abc name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "combined options after single hyphen"
```

#### Test: combined options where last takes argument

Verifies that when options are grouped and the last one requires an argument (e.g. `-abf file.txt`), all preceding options are parsed normally and the final option receives its argument from the next word.

```
begin test "combined options where last takes argument"
  script
    $SHELL -c 'OPTIND=1; result=""; arg=""; set -- -abf file.txt; while getopts abf: name; do result="${result}${name}"; if [ "$name" = "f" ]; then arg="$OPTARG"; fi; done; echo "${result}:${arg}"'
  expect
    stdout "abf:file.txt"
    stderr ""
    exit_code 0
end test "combined options where last takes argument"
```

#### Test: OPTARG unset for options without arguments

Verifies that `OPTARG` is unset after parsing an option that does not take an argument, even if `OPTARG` previously held a value.

```
begin test "OPTARG unset for options without arguments"
  script
    $SHELL -c 'OPTIND=1; OPTARG="stale"; set -- -a; getopts a name; echo "${OPTARG:-UNSET}"'
  expect
    stdout "UNSET"
    stderr ""
    exit_code 0
end test "OPTARG unset for options without arguments"
```

#### Test: getopts uses positional parameters by default

Verifies that when no `param` operands are given, `getopts` parses the current positional parameters (`"$@"`) by default.

```
begin test "getopts uses positional parameters by default"
  script
    $SHELL -c 'set -- -a -b; OPTIND=1; result=""; while getopts ab name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "ab"
    stderr ""
    exit_code 0
end test "getopts uses positional parameters by default"
```

#### Test: getopts uses function positional parameters

Verifies that inside a shell function, `getopts` operates on the function's own positional parameters rather than the script-level ones.

```
begin test "getopts uses function positional parameters"
  script
    $SHELL -c 'f() { OPTIND=1; result=""; while getopts xy name; do result="${result}${name}"; done; echo "$result"; }; f -x -y'
  expect
    stdout "xy"
    stderr ""
    exit_code 0
end test "getopts uses function positional parameters"
```

#### Test: param operands override positional parameters

Verifies that when explicit `param` operands are supplied after `name` on the `getopts` command line, they are parsed instead of the current positional parameters.

```
begin test "param operands override positional parameters"
  script
    $SHELL -c 'set -- -a; OPTIND=1; getopts x name -x; echo "$name"'
  expect
    stdout "x"
    stderr ""
    exit_code 0
end test "param operands override positional parameters"
```

#### Test: non-option argument stops parsing

Verifies that encountering a parameter that does not begin with `-` (and is not an option-argument) terminates option processing, even if further option-like arguments follow.

```
begin test "non-option argument stops parsing"
  script
    $SHELL -c 'OPTIND=1; result=""; set -- -a operand -b; while getopts ab name; do result="${result}${name}"; done; echo "$result"'
  expect
    stdout "a"
    stderr ""
    exit_code 0
end test "non-option argument stops parsing"
```

#### Test: explicit null option-argument as separate word

Verifies that an explicit null string (`""`) supplied as a separate argument is accepted as a valid option-argument, with `OPTARG` set to the empty string.

```
begin test "explicit null option-argument as separate word"
  script
    $SHELL -c 'OPTIND=1; set -- -f ""; getopts f: name; echo "name=$name OPTARG=<$OPTARG>"'
  expect
    stdout "name=f OPTARG=<>"
    stderr ""
    exit_code 0
end test "explicit null option-argument as separate word"
```

#### Test: readonly name variable causes return value greater than one

Verifies that if the `name` variable is marked readonly, `getopts` cannot assign to it and treats this as a processing error, returning an exit status greater than one.

```
begin test "readonly name variable causes return value greater than one"
  script
    $SHELL -c 'OPTIND=1; readonly name; set -- -a; getopts a name 2>/dev/null; echo $?'
  expect
    stdout "[2-9][0-9]*"
    stderr ""
    exit_code 0
end test "readonly name variable causes return value greater than one"
```

#### Test: OPTIND set to 1 + count when no operands remain

Verifies that when all parameters are options and none are operands, `OPTIND` is set to `$# + 1` (one past the last parameter) after options are exhausted.

```
begin test "OPTIND set to 1 + count when no operands remain"
  script
    $SHELL -c 'OPTIND=1; set -- -a -b; while getopts ab name; do :; done; echo "$OPTIND"'
  expect
    stdout "3"
    stderr ""
    exit_code 0
end test "OPTIND set to 1 + count when no operands remain"
```

#### Test: getopts basic option parsing

Verifies the core `getopts` loop: parsing `-a` (no argument), then `-b foo` (option with required argument via `b:` in optstring), then detecting end-of-options. OPTIND advances correctly to point past the consumed arguments.

```
begin test "getopts basic option parsing"
  script
    set -- -a -b foo bar
    getopts "ab:" opt
    echo "$opt"
    getopts "ab:" opt
    echo "$opt $OPTARG"
    getopts "ab:" opt
    echo "$?"
    echo "$OPTIND"
  expect
    stdout "a\nb foo\n1\n4"
    stderr ""
    exit_code 0
end test "getopts basic option parsing"
```

#### Test: getopts silent mode error handling

When optstring begins with `:` (silent mode), `getopts` suppresses its own error messages. For an unrecognized option, `name` is set to `?` and OPTARG to the offending character. For a missing option-argument, `name` is set to `:` and OPTARG to the option character.

```
begin test "getopts silent mode error handling"
  script
    set -- -x -b
    getopts ":ab:" opt
    echo "$opt $OPTARG"
    getopts ":ab:" opt
    echo "$opt $OPTARG"
  expect
    stdout "\? x\n: b"
    stderr ""
    exit_code 0
end test "getopts silent mode error handling"
```

#### Test: getopts verbose mode missing option-argument

In verbose mode (no leading `:` in optstring), when an option that requires an argument is given without one, `getopts` sets `name` to `?` and OPTARG is unset. A diagnostic message is written to stderr.

```
begin test "getopts verbose mode missing option-argument"
  script
    set -- -b
    getopts "ab:" opt 2>/dev/null
    echo "$opt ${OPTARG:-unset}"
  expect
    stdout "\? unset"
    stderr ""
    exit_code 0
end test "getopts verbose mode missing option-argument"
```
