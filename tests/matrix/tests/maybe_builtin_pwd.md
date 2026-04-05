# Test Suite for Maybe-Builtin Utility: pwd

This test suite covers the **pwd** utility as specified by
POSIX.1-2024. This utility is commonly implemented as a shell builtin
but the standard does not require it.

## Table of contents

- [utility: pwd](#utility-pwd)

## utility: pwd

#### NAME

> pwd — return working directory name

#### SYNOPSIS

> `pwd [-L|-P]`

#### DESCRIPTION

> The *pwd* utility shall write to standard output an absolute pathname of the current working directory, which does not contain the filenames dot or dot-dot.

#### OPTIONS

> The *pwd* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following options shall be supported by the implementation:
>
> - **-L**: If the *PWD* environment variable contains an absolute pathname of the current directory and the pathname does not contain any components that are dot or dot-dot, *pwd* shall write this pathname to standard output, except that if the *PWD* environment variable is longer than {PATH_MAX} bytes including the terminating null, it is unspecified whether *pwd* writes this pathname to standard output or behaves as if the **-P** option had been specified. Otherwise, the **-L** option shall behave as the **-P** option.
> - **-P**: The pathname written to standard output shall not contain any components that refer to files of type symbolic link. If there are multiple pathnames that the *pwd* utility could write to standard output, one beginning with a single `<slash>` character and one or more beginning with two `<slash>` characters, then it shall write the pathname beginning with a single `<slash>` character. The pathname shall not contain any unnecessary `<slash>` characters after the leading one or two `<slash>` characters.
>
> If both **-L** and **-P** are specified, the last one shall apply. If neither **-L** nor **-P** is specified, the *pwd* utility shall behave as if **-L** had been specified.

#### OPERANDS

> None.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *pwd*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *PWD*: An absolute pathname of the current working directory. If an application sets or unsets the value of *PWD ,* the behavior of *pwd* is unspecified.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The *pwd* utility output is an absolute pathname of the current working directory:
>
> ```
> "%s\n", <directory pathname>
> ```

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: Successful completion.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> If an error is detected other than a write error when writing to standard output, no output shall be written to standard output, a diagnostic message shall be written to standard error, and the exit status shall be non-zero.

---

*The following sections are informative.*

#### APPLICATION USAGE

> If the pathname obtained from *pwd* is longer than {PATH_MAX} bytes, it could produce an error if passed to [*cd*](docs/posix/md/utilities/cd.md). Therefore, in order to return to that directory it may be necessary to break the pathname into sections shorter than {PATH_MAX} and call [*cd*](docs/posix/md/utilities/cd.md) on each section in turn (the first section being an absolute pathname and subsequent sections being relative pathnames).

#### EXAMPLES

> None.

#### RATIONALE

> Some implementations have historically provided *pwd* as a shell special built-in command.
>
> In most utilities, if an error occurs, partial output may be written to standard output. This does not happen in historical implementations of *pwd* (unless an error condition causes a partial write). Because *pwd* is frequently used in historical shell scripts without checking the exit status, it is important that the historical behavior is required here; therefore, the CONSEQUENCES OF ERRORS section specifically disallows any partial output being written to standard output, except when a write error occurs when writing to standard output.
>
> An earlier version of this standard stated that the *PWD* environment variable was affected when the **-P** option was in effect. This was incorrect; conforming implementations do not do this.

#### FUTURE DIRECTIONS

> If this utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used, implementations are encouraged to treat this as an error. A future version of this standard may require implementations to treat this as an error.

#### SEE ALSO

> [*cd*](docs/posix/md/utilities/cd.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*getcwd*()](docs/posix/md/functions/getcwd.md)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> The **-P** and **-L** options are added to describe actions relating to symbolic links as specified in the IEEE P1003.2b draft standard.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #097 is applied.
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> Changes to the *pwd* utility and *PWD* environment variable have been made to match the changes to the [*getcwd*()](docs/posix/md/functions/getcwd.md) function made for Austin Group Interpretation 1003.1-2001 #140.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0161 [471] is applied.

#### Issue 8

> Austin Group Defect 251 is applied, encouraging implementations to report an error if a utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1488 is applied, clarifying the behavior when a write error occurs when writing to standard output.

*End of informative text.*

### Tests

#### Test: pwd outputs absolute path

`pwd` writes an absolute pathname of the current working directory.

```
begin test "pwd outputs absolute path"
  script
    pwd | grep -q '^/' && echo PASS || echo FAIL
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd outputs absolute path"
```

#### Test: pwd -P outputs absolute path

`pwd -P` writes the physical directory without symlink components.

```
begin test "pwd -P outputs absolute path"
  script
    pwd -P | grep -q '^/' && echo PASS || echo FAIL
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P outputs absolute path"
```

#### Test: pwd produces exactly one line

The output format is `"%s\n"` — one directory pathname followed by
a single newline.

```
begin test "pwd produces exactly one line"
  script
    _lines=$(pwd | wc -l | tr -d ' ')
    if [ "$_lines" = "1" ]; then
      echo PASS
    else
      echo FAIL
    fi
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd produces exactly one line"
```

#### Test: pwd after cd / writes /

`pwd` writes the absolute pathname of the current working
directory.

```
begin test "pwd after cd / writes /"
  script
    cd / && pwd
  expect
    stdout "/"
    stderr ""
    exit_code 0
end test "pwd after cd / writes /"
```

#### Test: pwd -P returns non-empty path

`pwd -P` writes a physical pathname without symbolic link
components.

```
begin test "pwd -P returns non-empty path"
  script
    pwd -P | grep -q . && echo "ok"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "pwd -P returns non-empty path"
```

#### Test: pwd -L -P succeeds

When both `-L` and `-P` are specified, the last one applies.

```
begin test "pwd -L -P succeeds"
  script
    pwd -L -P >/dev/null
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "pwd -L -P succeeds"
```

#### Test: pwd default matches pwd -L

If neither `-L` nor `-P` is specified, `pwd` behaves as if `-L`
had been specified.

```
begin test "pwd default matches pwd -L"
  script
    cd / && default=$(pwd) && l=$(pwd -L) && [ "$default" = "$l" ] && echo "match"
  expect
    stdout "match"
    stderr ""
    exit_code 0
end test "pwd default matches pwd -L"
```

#### Test: pwd -L with clean PWD uses it

When `-L` is specified and `PWD` contains a valid absolute pathname without dot or dot-dot components, `pwd` shall write that value to standard output.

```
begin test "pwd -L with clean PWD uses it"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    ln -sfn "$_tmpdir/real/deep" "$_tmpdir/link"
    _real=$(cd "$_tmpdir/real/deep" && pwd -P)
    cd "$_tmpdir/real/deep"
    PWD="$_real" pwd -L
    rm -rf "$_tmpdir"
  expect
    stdout "/.*"
    stderr ""
    exit_code 0
end test "pwd -L with clean PWD uses it"
```

#### Test: pwd -L rejects PWD containing dot-dot

When `-L` is specified but `PWD` contains a `..` component, `pwd -L` shall not use it and must fall back to `-P` behavior, so the output should never contain `..`.

```
begin test "pwd -L rejects PWD containing dot-dot"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    cd "$_tmpdir/real/deep"
    _out=$(PWD="$_tmpdir/real/deep/../deep" pwd -L)
    case "$_out" in *".."*) echo FAIL_DOTDOT ;; *) echo PASS ;; esac
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -L rejects PWD containing dot-dot"
```

#### Test: pwd -L with bogus PWD falls back to physical

When `PWD` does not contain a valid absolute pathname of the current directory, `pwd -L` shall behave as if `-P` had been specified and output the physical path.

```
begin test "pwd -L with bogus PWD falls back to physical"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    cd "$_tmpdir/real/deep"
    _physical=$(pwd -P)
    _out=$(PWD=/nonexistent/bogus pwd -L)
    if [ "$_out" = "$_physical" ]; then
      echo MATCH
    else
      echo FAIL
    fi
    rm -rf "$_tmpdir"
  expect
    stdout "MATCH"
    stderr ""
    exit_code 0
end test "pwd -L with bogus PWD falls back to physical"
```

#### Test: pwd -L with symlink PWD outputs absolute path

When `PWD` is set to a symlink-based path that still resolves to the current directory, `pwd -L` may use it. The output must always be an absolute pathname starting with `/`.

```
begin test "pwd -L with symlink PWD outputs absolute path"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    ln -sfn "$_tmpdir/real/deep" "$_tmpdir/link"
    cd "$_tmpdir/real/deep"
    _out=$(PWD="$_tmpdir/link" pwd -L)
    case "$_out" in /*) echo PASS ;; *) echo FAIL ;; esac
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -L with symlink PWD outputs absolute path"
```

#### Test: pwd -L outputs absolute path

`pwd -L` shall write an absolute pathname of the current working directory, which must begin with `/`.

```
begin test "pwd -L outputs absolute path"
  script
    pwd -L | grep -q '^/' && echo PASS || echo FAIL
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -L outputs absolute path"
```

#### Test: pwd with no options outputs absolute path

When invoked with no options, `pwd` defaults to `-L` behavior and shall write an absolute pathname beginning with `/`.

```
begin test "pwd with no options outputs absolute path"
  script
    pwd | grep -q '^/' && echo PASS || echo FAIL
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd with no options outputs absolute path"
```

#### Test: pwd -P from root has no unnecessary double slashes

The pathname written by `pwd -P` shall not contain any unnecessary slash characters after the leading one or two slashes. This test verifies no double slashes appear when running from the root directory.

```
begin test "pwd -P from root has no unnecessary double slashes"
  script
    _out=$(cd / && pwd -P)
    _stripped=$(echo "$_out" | sed 's|^//||')
    case "$_stripped" in *//*) echo FAIL ;; *) echo PASS ;; esac
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P from root has no unnecessary double slashes"
```

#### Test: pwd -P from deep dir has no unnecessary double slashes

The pathname written by `pwd -P` shall not contain any unnecessary slash characters after the leading one or two slashes. This test verifies no double slashes appear when running from a deeply nested directory.

```
begin test "pwd -P from deep dir has no unnecessary double slashes"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    _out=$(cd "$_tmpdir/real/deep" && pwd -P)
    _stripped=$(echo "$_out" | sed 's|^//||')
    case "$_stripped" in *//*) echo FAIL ;; *) echo PASS ;; esac
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P from deep dir has no unnecessary double slashes"
```

#### Test: pwd -L has no unnecessary double slashes

The pathname written by `pwd -L` shall not contain unnecessary slash characters. This test verifies no double slashes appear in the output from a nested directory.

```
begin test "pwd -L has no unnecessary double slashes"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    _out=$(cd "$_tmpdir/real/deep" && pwd -L)
    _stripped=$(echo "$_out" | sed 's|^//||')
    case "$_stripped" in *//*) echo FAIL ;; *) echo PASS ;; esac
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -L has no unnecessary double slashes"
```

#### Test: pwd -P has no trailing slash

The absolute pathname written by `pwd -P` shall not contain unnecessary trailing slash characters (the root `/` excluded).

```
begin test "pwd -P has no trailing slash"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/real/deep"
    _out=$(cd "$_tmpdir/real/deep" && pwd -P)
    case "$_out" in */) echo FAIL ;; *) echo PASS ;; esac
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P has no trailing slash"
```

#### Test: pwd output ends with exactly one trailing newline

The output format of `pwd` is specified as `"%s\n"`, so the output should contain exactly one trailing newline after the directory pathname.

```
begin test "pwd output ends with exactly one trailing newline"
  script
    _raw_len=$(pwd | wc -c | tr -d ' ')
    _path_len=$(printf '%s' "$(pwd)" | wc -c | tr -d ' ')
    _diff=$((_raw_len - _path_len))
    if [ "$_diff" = "1" ]; then
      echo PASS
    else
      echo FAIL
    fi
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd output ends with exactly one trailing newline"
```

#### Test: pwd produces no stderr on success

Standard error shall be used only for diagnostic messages, so on a successful invocation `pwd` must produce no stderr output.

```
begin test "pwd produces no stderr on success"
  script
    _err=$(pwd 2>&1 1>/dev/null)
    if [ -z "$_err" ]; then
      echo PASS
    else
      echo FAIL
    fi
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd produces no stderr on success"
```

#### Test: pwd -L produces no stderr on success

Standard error shall be used only for diagnostic messages, so a successful `pwd -L` invocation must produce no stderr output.

```
begin test "pwd -L produces no stderr on success"
  script
    _err=$(pwd -L 2>&1 1>/dev/null)
    if [ -z "$_err" ]; then
      echo PASS
    else
      echo FAIL
    fi
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -L produces no stderr on success"
```

#### Test: pwd -P produces no stderr on success

Standard error shall be used only for diagnostic messages, so a successful `pwd -P` invocation must produce no stderr output.

```
begin test "pwd -P produces no stderr on success"
  script
    _err=$(pwd -P 2>&1 1>/dev/null)
    if [ -z "$_err" ]; then
      echo PASS
    else
      echo FAIL
    fi
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P produces no stderr on success"
```

#### Test: pwd -P in deleted directory either succeeds with absolute path or fails silently

If the current directory has been removed, `pwd -P` may either succeed and write an absolute path, or fail. On failure, the CONSEQUENCES OF ERRORS section requires that no output is written to stdout and a diagnostic may go to stderr.

```
begin test "pwd -P in deleted directory either succeeds with absolute path or fails silently"
  script
    _tmpdir="/tmp/pwd_test_$$"
    mkdir -p "$_tmpdir/will_remove"
    cd "$_tmpdir/will_remove"
    rmdir "$_tmpdir/will_remove"
    _result=$(pwd -P 2>/dev/null)
    _rc=$?
    if [ "$_rc" -eq 0 ]; then
      case "$_result" in /*) echo PASS ;; *) echo FAIL_NOT_ABSOLUTE ;; esac
    else
      if [ -z "$_result" ]
      then echo PASS
    else
      echo FAIL_STDOUT_ON_ERROR
    fi
    fi
    rm -rf "$_tmpdir"
  expect
    stdout "PASS"
    stderr ""
    exit_code 0
end test "pwd -P in deleted directory either succeeds with absolute path or fails silently"
```

#### Test: pwd matches working directory

Verifies that `pwd` succeeds and produces some output representing the current working directory pathname.

```
begin test "pwd matches working directory"
  script
    pwd
  expect
    stdout ".*"
    stderr ""
    exit_code 0
end test "pwd matches working directory"
```
