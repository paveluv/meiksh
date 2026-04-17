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

#### Test: OPTIND initialized to 1

When the shell starts, `OPTIND` shall be initialized to `1`.

```
begin test "OPTIND initialized to 1"
  script
    printf '%s\n' "$OPTIND"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "OPTIND initialized to 1"
```

#### Test: getopts parses positional parameters by default

Without explicit `param` operands, `getopts` shall parse the current
positional parameters in the current shell execution environment.

```
begin test "getopts parses positional parameters by default"
  script
    OPTIND=1
    set -- -a -b
    result=
    while getopts ab name; do
      result="${result}${name}"
    done
    printf '%s\n' "$result"
  expect
    stdout "ab"
    stderr ""
    exit_code 0
end test "getopts parses positional parameters by default"
```

#### Test: param operands override positional parameters

If `param` operands are supplied, `getopts` shall parse those operands
instead of the shell's current positional parameters.

```
begin test "param operands override positional parameters"
  script
    set -- -a
    OPTIND=1
    result=
    while getopts xy name -x -y; do
      result="${result}${name}"
    done
    printf '%s:%s\n' "$result" "$OPTIND"
  expect
    stdout "xy:3"
    stderr ""
    exit_code 0
end test "param operands override positional parameters"
```

#### Test: option with separate argument sets OPTARG and OPTIND

When an option that requires an argument is followed by a separate
parameter, `getopts` shall set `name`, set `OPTARG` to just the argument
text, and advance `OPTIND` past both words.

```
begin test "option with separate argument sets OPTARG and OPTIND"
  script
    OPTIND=1
    set -- -f value operand
    getopts f: name
    status=$?
    printf 'name=%s optarg=%s optind=%s status=%s\n' "$name" "$OPTARG" "$OPTIND" "$status"
  expect
    stdout "name=f optarg=value optind=3 status=0"
    stderr ""
    exit_code 0
end test "option with separate argument sets OPTARG and OPTIND"
```

#### Test: option with attached argument sets OPTARG and OPTIND

If an option requiring an argument is written in the same word as its
argument, the following characters shall be treated as the argument and
`OPTIND` shall advance to the next parameter.

```
begin test "option with attached argument sets OPTARG and OPTIND"
  script
    OPTIND=1
    set -- -fvalue operand
    getopts f: name
    status=$?
    printf 'name=%s optarg=%s optind=%s status=%s\n' "$name" "$OPTARG" "$OPTIND" "$status"
  expect
    stdout "name=f optarg=value optind=2 status=0"
    stderr ""
    exit_code 0
end test "option with attached argument sets OPTARG and OPTIND"
```

#### Test: separate option-argument preserves embedded spaces

When an option-argument is supplied as a separate parameter, `OPTARG`
shall contain only the characters of that argument, including embedded
spaces.

```
begin test "separate option-argument preserves embedded spaces"
  script
    OPTIND=1
    set -- -f "a b" operand
    getopts f: name
    printf 'name=%s optarg=<%s> optind=%s\n' "$name" "$OPTARG" "$OPTIND"
  expect
    stdout "name=f optarg=<a b> optind=3"
    stderr ""
    exit_code 0
end test "separate option-argument preserves embedded spaces"
```

#### Test: option without argument unsets OPTARG

If the option found does not have an option-argument, `OPTARG` shall be
unset.

```
begin test "option without argument unsets OPTARG"
  script
    OPTIND=1
    OPTARG=stale
    set -- -a
    getopts a name
    printf '%s\n' "${OPTARG-unset}"
  expect
    stdout "unset"
    stderr ""
    exit_code 0
end test "option without argument unsets OPTARG"
```

#### Test: invalid option in normal mode reports diagnostic

For an unknown option in normal mode, `name` shall be `?`, `OPTARG`
shall be unset, the exit status shall remain `0`, and stderr shall
identify both the invoking program and the offending option character.

```
begin test "invalid option in normal mode reports diagnostic"
  script
    tmp=$(mktemp)
    prog=${0##*/}
    OPTIND=1
    set -- -z
    getopts ab name 2>"$tmp"
    status=$?
    if [ "$name" = "?" ] &&
       [ "${OPTARG+set}" != set ] &&
       [ "$status" -eq 0 ] &&
       grep -Fq "$prog" "$tmp" &&
       grep -Fq "z" "$tmp"; then
      echo ok
    else
      echo fail
    fi
    rm -f "$tmp"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "invalid option in normal mode reports diagnostic"
```

#### Test: invalid option in silent mode suppresses diagnostic

If `optstring` starts with `:`, an unknown option shall still return
status `0`, but `name` shall be `?`, `OPTARG` shall contain the
offending character, and no diagnostic shall be written to stderr.

```
begin test "invalid option in silent mode suppresses diagnostic"
  script
    tmp=$(mktemp)
    OPTIND=1
    set -- -z
    getopts :ab name 2>"$tmp"
    status=$?
    printf 'name=%s optarg=%s status=%s stderr_bytes=%s\n' "$name" "$OPTARG" "$status" "$(wc -c <"$tmp" | tr -d ' ')"
    rm -f "$tmp"
  expect
    stdout "name=\? optarg=z status=0 stderr_bytes=0"
    stderr ""
    exit_code 0
end test "invalid option in silent mode suppresses diagnostic"
```

#### Test: missing argument in normal mode reports diagnostic

If a required option-argument is missing in normal mode, `name` shall be
`?`, `OPTARG` shall be unset, the exit status shall remain `0`, and
stderr shall identify both the invoking program and the option whose
argument is missing.

```
begin test "missing argument in normal mode reports diagnostic"
  script
    tmp=$(mktemp)
    prog=${0##*/}
    OPTIND=1
    set -- -f
    getopts f: name 2>"$tmp"
    status=$?
    if [ "$name" = "?" ] &&
       [ "${OPTARG+set}" != set ] &&
       [ "$status" -eq 0 ] &&
       grep -Fq "$prog" "$tmp" &&
       grep -Fq "f" "$tmp"; then
      echo ok
    else
      echo fail
    fi
    rm -f "$tmp"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "missing argument in normal mode reports diagnostic"
```

#### Test: missing argument in silent mode sets colon and OPTARG

If `optstring` starts with `:` and a required option-argument is
missing, `name` shall be `:`, `OPTARG` shall be the option character,
and no diagnostic shall be written to stderr.

```
begin test "missing argument in silent mode sets colon and OPTARG"
  script
    tmp=$(mktemp)
    OPTIND=1
    set -- -f
    getopts :f: name 2>"$tmp"
    status=$?
    printf 'name=%s optarg=%s status=%s stderr_bytes=%s\n' "$name" "$OPTARG" "$status" "$(wc -c <"$tmp" | tr -d ' ')"
    rm -f "$tmp"
  expect
    stdout "name=: optarg=f status=0 stderr_bytes=0"
    stderr ""
    exit_code 0
end test "missing argument in silent mode sets colon and OPTARG"
```

#### Test: getopts resumes parsing after invalid option

After an unknown option, `OPTIND` is unspecified but shall still encode
enough state for the next `getopts` call to resume after the option just
parsed.

```
begin test "getopts resumes parsing after invalid option"
  script
    OPTIND=1
    set -- -z -a
    getopts ab name 2>/dev/null
    getopts ab name 2>/dev/null
    status=$?
    printf 'name=%s status=%s\n' "$name" "$status"
  expect
    stdout "name=a status=0"
    stderr ""
    exit_code 0
end test "getopts resumes parsing after invalid option"
```

#### Test: first operand ends option processing

Encountering a non-option operand shall end option processing, return
status `1`, set `name` to `?`, leave `OPTARG` unset, and leave `OPTIND`
pointing at that operand.

```
begin test "first operand ends option processing"
  script
    OPTIND=1
    OPTARG=stale
    set -- -a operand -b
    getopts a name
    getopts a name
    status=$?
    printf 'name=%s optind=%s optarg=%s status=%s\n' "$name" "$OPTIND" "${OPTARG-unset}" "$status"
  expect
    stdout "name=\? optind=2 optarg=unset status=1"
    stderr ""
    exit_code 0
end test "first operand ends option processing"
```

#### Test: double dash ends option processing

The first `--` that is not itself an option-argument shall end option
processing, and `OPTIND` shall point to the next operand after it.

```
begin test "double dash ends option processing"
  script
    OPTIND=1
    OPTARG=stale
    set -- -a -- operand -b
    getopts a name
    getopts a name
    status=$?
    saved_optind=$OPTIND
    shift $((saved_optind - 1))
    printf 'name=%s optind=%s optarg=%s status=%s first=%s\n' "$name" "$saved_optind" "${OPTARG-unset}" "$status" "$1"
  expect
    stdout "name=\? optind=3 optarg=unset status=1 first=operand"
    stderr ""
    exit_code 0
end test "double dash ends option processing"
```

#### Test: end of options with no operands sets OPTIND past list

If there are no operands after option processing ends, `getopts` shall
return `1`, set `name` to `?`, unset `OPTARG`, and set `OPTIND` to one
past the parameter list.

```
begin test "end of options with no operands sets OPTIND past list"
  script
    OPTIND=1
    OPTARG=stale
    set -- -a -b
    getopts ab name
    getopts ab name
    getopts ab name
    status=$?
    printf 'name=%s optind=%s optarg=%s status=%s\n' "$name" "$OPTIND" "${OPTARG-unset}" "$status"
  expect
    stdout "name=\? optind=3 optarg=unset status=1"
    stderr ""
    exit_code 0
end test "end of options with no operands sets OPTIND past list"
```

#### Test: empty parameter list reports end of options

If the parameter list is empty, `getopts` shall immediately report end of
options with status `1`, set `name` to `?`, leave `OPTARG` unset, and
set `OPTIND` to `1`.

```
begin test "empty parameter list reports end of options"
  script
    OPTIND=1
    OPTARG=stale
    set --
    getopts a name
    status=$?
    printf 'name=%s optind=%s optarg=%s status=%s\n' "$name" "$OPTIND" "${OPTARG-unset}" "$status"
  expect
    stdout "name=\? optind=1 optarg=unset status=1"
    stderr ""
    exit_code 0
end test "empty parameter list reports end of options"
```

#### Test: OPTIND and OPTARG are not exported by default

`OPTIND` and `OPTARG` shall not be exported to child processes unless
the application explicitly exports them.

```
begin test "OPTIND and OPTARG are not exported by default"
  script
    OPTIND=1
    OPTARG=example
    env | grep -Eq '^(OPTIND|OPTARG)='
    printf '%s\n' "$?"
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "OPTIND and OPTARG are not exported by default"
```

#### Test: setting OPTIND to 1 allows reparsing new parameters

If the application resets `OPTIND` to `1`, `getopts` shall allow a new
parameter list to be parsed in the same shell execution environment.

```
begin test "setting OPTIND to 1 allows reparsing new parameters"
  script
    OPTIND=1
    set -- -a -b
    result=
    while getopts ab name; do
      result="${result}${name}"
    done
    OPTIND=1
    set -- -x -y
    while getopts xy name; do
      result="${result}${name}"
    done
    printf '%s\n' "$result"
  expect
    stdout "abxy"
    stderr ""
    exit_code 0
end test "setting OPTIND to 1 allows reparsing new parameters"
```

#### Test: combined short options are parsed separately

Grouped single-letter options after one hyphen shall be returned one at
a time across successive `getopts` calls.

```
begin test "combined short options are parsed separately"
  script
    OPTIND=1
    set -- -abc
    result=
    while getopts abc name; do
      result="${result}${name}"
    done
    printf '%s\n' "$result"
  expect
    stdout "abc"
    stderr ""
    exit_code 0
end test "combined short options are parsed separately"
```

#### Test: grouped options may end with option requiring argument

When grouped options end with one that requires an argument, earlier
options shall still be parsed normally and the final option shall consume
its argument.

```
begin test "grouped options may end with option requiring argument"
  script
    OPTIND=1
    set -- -abf file.txt
    result=
    arg=
    while getopts abf: name; do
      result="${result}${name}"
      if [ "$name" = "f" ]; then
        arg=$OPTARG
      fi
    done
    printf '%s:%s\n' "$result" "$arg"
  expect
    stdout "abf:file.txt"
    stderr ""
    exit_code 0
end test "grouped options may end with option requiring argument"
```

#### Test: getopts uses function positional parameters

Inside a shell function, `getopts` shall parse that function's current
positional parameters by default.

```
begin test "getopts uses function positional parameters"
  script
    f() {
      OPTIND=1
      result=
      while getopts xy name; do
        result="${result}${name}"
      done
      printf '%s\n' "$result"
    }
    set -- -a
    f -x -y
  expect
    stdout "xy"
    stderr ""
    exit_code 0
end test "getopts uses function positional parameters"
```

#### Test: explicit null option-argument as separate parameter is preserved

An explicit null string supplied as a separate option-argument shall be
accepted as the option-argument value.

```
begin test "explicit null option-argument as separate parameter is preserved"
  script
    OPTIND=1
    set -- -f ""
    getopts f: name
    printf 'name=%s optarg=<%s>\n' "$name" "$OPTARG"
  expect
    stdout "name=f optarg=<>"
    stderr ""
    exit_code 0
end test "explicit null option-argument as separate parameter is preserved"
```

#### Test: readonly name variable causes processing error

If `getopts` cannot assign to the variable named by `name`, that is a
`getopts` processing error and shall produce a return value greater than
`1`.

```
begin test "readonly name variable causes processing error"
  script
    OPTIND=1
    readonly name
    set -- -a
    getopts a name 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code >1
end test "readonly name variable causes processing error"
```

#### Test: readonly OPTIND causes processing error

If `getopts` cannot update `OPTIND`, that is also a processing error and
shall produce a return value greater than `1`. This test asserts the
POSIX requirement directly; `bash --posix` currently returns `0` here.

```
begin test "readonly OPTIND causes processing error"
  script
    OPTIND=1
    readonly OPTIND
    set -- -a
    getopts a name 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code >1
end test "readonly OPTIND causes processing error"
```

#### Test: readonly OPTARG causes processing error

If `getopts` cannot assign `OPTARG` for an option that requires an
argument, that too shall be a processing error with a return value
greater than `1`. This test asserts the POSIX requirement directly;
`bash --posix` currently returns `0` here.

```
begin test "readonly OPTARG causes processing error"
  script
    OPTIND=1
    readonly OPTARG
    set -- -f value
    getopts f: name 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code >1
end test "readonly OPTARG causes processing error"
```

#### Test: getopts option argument with multi-byte characters in C locale

In the C locale, option arguments containing high bytes are passed through
verbatim. Each byte is an independent character.

```
begin test "getopts option argument with multi-byte characters in C locale"
  setenv "LC_ALL" "C"
  script
    OPTIND=1
    set -- -f "$(printf 'a\303\251b')"
    getopts f: name
    printf '%s' "$OPTARG" | od -An -t x1 | tr -d ' \n'
  expect
    stdout "61c3a962"
    stderr ""
    exit_code 0
end test "getopts option argument with multi-byte characters in C locale"
```

#### Test: getopts option argument with multi-byte characters in UTF-8

In C.UTF-8, option arguments containing multi-byte characters are preserved
intact. The argument `a\303\251b` contains three characters: `a`, U+00E9, `b`.

```
begin test "getopts option argument with multi-byte characters in UTF-8"
  setenv "LC_ALL" "C.UTF-8"
  script
    OPTIND=1
    set -- -f "$(printf 'a\303\251b')"
    getopts f: name
    printf '%s' "$OPTARG" | od -An -t x1 | tr -d ' \n'
  expect
    stdout "61c3a962"
    stderr ""
    exit_code 0
end test "getopts option argument with multi-byte characters in UTF-8"
```
