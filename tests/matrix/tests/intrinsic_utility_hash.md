# Test Suite for Intrinsic Utility: hash

This test suite covers the **hash** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: hash](#utility-hash)

## utility: hash

#### NAME

> hash — remember or report utility locations

#### SYNOPSIS

> ```
> hash [utility...]
> hash -r
> ```

#### DESCRIPTION

> The *hash* utility shall affect the way the current shell environment remembers the locations of utilities found as described in [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution). Depending on the arguments specified, it shall add utility locations to its list of remembered locations or it shall purge the contents of the list. When no arguments are specified, it shall report on the contents of the list.
>
> Utilities provided as built-ins to the shell and functions shall not be reported by *hash*.

#### OPTIONS

> The *hash* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following option shall be supported:
>
> - **-r**: Forget all previously remembered utility locations.

#### OPERANDS

> The following operand shall be supported:
>
> - *utility*: The name of a utility to be searched for and added to the list of remembered locations. If the search does not find *utility*, it is unspecified whether or not this is treated as an error. If *utility* contains one or more `<slash>` characters, the results are unspecified.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *hash*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *PATH*: Determine the location of *utility*, as described in XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables).

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> The standard output of *hash* shall be used when no arguments are specified. Its format is unspecified, but includes the pathname of each utility in the list of remembered locations for the current shell environment. This list shall consist of those utilities named in previous *hash* invocations that have been invoked, and may contain those invoked and found through the normal command search process. This list shall be cleared when the contents of the *PATH* environment variable are changed.

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

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *hash* affects the current shell execution environment, it is always provided as a shell regular built-in. If it is called in a separate utility execution environment, such as one of the following:
>
> ```
> nohup hash -r
> find . -type f -exec hash {} +
> ```
>
> it does not affect the command search process of the caller's environment.
>
> The *hash* utility may be implemented as an alias—for example, [*alias*](docs/posix/md/utilities/alias.md) **-t -**, in which case utilities found through normal command search are not listed by the *hash* command.
>
> The effects of *hash* **-r** can also be achieved portably by resetting the value of *PATH ;* in the simplest form, this can be:
>
> ```
> PATH="$PATH"
> ```
>
> The use of *hash* with *utility* names is unnecessary for most applications, but may provide a performance improvement on a few implementations; normally, the hashing process is included by default.

#### EXAMPLES

> None.

#### RATIONALE

> None.

#### FUTURE DIRECTIONS

> If this utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used, implementations are encouraged to treat this as an error. A future version of this standard may require implementations to treat this as an error.

#### SEE ALSO

> [*2.9.1.4 Command Search and Execution*](docs/posix/md/utilities/V3_chap02.md#2914-command-search-and-execution)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 7

> The *hash* utility is moved from the XSI option to the Base.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0093 [241] is applied.

#### Issue 8

> Austin Group Defect 248 is applied, changing a command line in the APPLICATION USAGE section.
>
> Austin Group Defect 251 is applied, encouraging implementations to report an error if a utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used.
>
> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1063 is applied, clarifying that functions are not reported by *hash*, and that the list of remembered locations is cleared when the contents of *PATH* are changed.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1460 is applied, making it explicitly unspecified whether or not *hash* reports an error if it cannot find *utility*.

*End of informative text.*

### Tests

#### Test: hash -r forgets all remembered locations

`hash -r` clears the hash table.

```
begin test "hash -r forgets all remembered locations"
  script
    hash ls 2>/dev/null
    hash -r
    count=$(hash 2>/dev/null | wc -l)
    echo "$count"
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "hash -r forgets all remembered locations"
```

#### Test: hash a known utility succeeds

Hashing a known utility exits with code 0.

```
begin test "hash a known utility succeeds"
  script
    hash ls
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "hash a known utility succeeds"
```

#### Test: hash -r exits with code 0

Running `hash -r` with no remembered utilities should succeed and exit with status 0, since purging an already-empty table is valid.

```
begin test "hash -r exits with code 0"
  script
    hash -r
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "hash -r exits with code 0"
```

#### Test: hash output mentions hashed utility

After hashing `ls`, running `hash` with no arguments should produce output that includes the name `ls`, verifying that the utility was added to the remembered-locations list.

```
begin test "hash output mentions hashed utility"
  script
    hash -r
    hash ls 2>/dev/null
    hash 2>&1
  expect
    stdout "(.|\n)*ls(.|\n)*"
    stderr ""
    exit_code 0
end test "hash output mentions hashed utility"
```

#### Test: hash with utility argument adds it

When `hash` is invoked with a utility name operand (here `cat`), that utility should be added to the remembered-locations list and appear in subsequent `hash` output.

```
begin test "hash with utility argument adds it"
  script
    hash -r
    hash cat 2>/dev/null
    hash 2>&1
  expect
    stdout "(.|\n)*cat(.|\n)*"
    stderr ""
    exit_code 0
end test "hash with utility argument adds it"
```

#### Test: hash -r purges the list

After hashing a utility, `hash -r` should forget all previously remembered locations. A subsequent `hash` invocation should produce no output.

```
begin test "hash -r purges the list"
  script
    hash cat 2>/dev/null
    hash -r
    out=$(hash 2>&1)
    [ -z "$out" ] && echo purged || echo stillhas
  expect
    stdout "purged"
    stderr ""
    exit_code 0
end test "hash -r purges the list"
```

#### Test: hash reports when table has items

When the hash table contains entries, `hash` with no arguments should produce non-empty output reporting the remembered utility locations.

```
begin test "hash reports when table has items"
  script
    hash -r
    hash ls 2>/dev/null
    hash cat 2>/dev/null
    out=$(hash 2>&1)
    [ -n "$out" ] && echo reported || echo empty
  expect
    stdout "reported"
    stderr ""
    exit_code 0
end test "hash reports when table has items"
```

#### Test: hash produces no output when table is empty

After clearing the hash table with `hash -r`, running `hash` with no arguments should produce no output, since there are no remembered locations to report.

```
begin test "hash produces no output when table is empty"
  script
    hash -r
    out=$(hash 2>&1)
    [ -z "$out" ] && echo empty || echo notempty
  expect
    stdout "empty"
    stderr ""
    exit_code 0
end test "hash produces no output when table is empty"
```

#### Test: hash output contains paths for hashed utilities

The output of `hash` should include the pathname of each remembered utility. After hashing `ls`, the output should contain at least one slash (`/`), indicating an absolute path.

```
begin test "hash output contains paths for hashed utilities"
  script
    hash -r
    hash ls 2>/dev/null
    hash 2>&1
  expect
    stdout "(.|\n)*/(.|\n)*"
    stderr ""
    exit_code 0
end test "hash output contains paths for hashed utilities"
```

#### Test: function not in hash output

POSIX requires that functions shall not be reported by `hash`. After defining a function `myfunc`, the hash table output must not mention it.

```
begin test "function not in hash output"
  script
    myfunc() { echo hi; }
    hash -r
    hash 2>&1 | grep -c myfunc || true
  expect
    stdout "0"
    stderr ""
    exit_code 0
end test "function not in hash output"
```

#### Test: built-in cd not in hash output

POSIX requires that utilities provided as built-ins to the shell shall not be reported by `hash`. The built-in `cd` must not appear in the hash table output.

```
begin test "built-in cd not in hash output"
  script
    hash -r
    hash ls 2>/dev/null
    out=$(hash 2>&1)
    case "$out" in *cd*) echo found_cd;; *) echo no_cd;; esac
  expect
    stdout "no_cd"
    stderr ""
    exit_code 0
end test "built-in cd not in hash output"
```

#### Test: hashing a shell function does not add it

Explicitly hashing a name that resolves to a shell function should not add it to the remembered-locations list, since `hash` only tracks external utilities.

```
begin test "hashing a shell function does not add it"
  script
    hash -r
    testfn() { :; }
    hash testfn 2>/dev/null
    out=$(hash 2>&1)
    case "$out" in *testfn*) echo found;; *) echo notfound;; esac
  expect
    stdout "notfound"
    stderr ""
    exit_code 0
end test "hashing a shell function does not add it"
```

#### Test: hash remembers and lists utilities

After hashing `sh`, the `hash` command with no arguments should succeed (exit 0), confirming that the utility was remembered and the list can be reported.

```
begin test "hash remembers and lists utilities"
  script
    hash -r
    hash sh
    hash >/dev/null && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "hash remembers and lists utilities"
```

#### Test: hash output goes to stdout

Verifies that `hash` writes its remembered-locations report to standard output (not standard error), so that the shell continues executing normally after the `hash` invocation.

```
begin test "hash output goes to stdout"
  script
    hash -r
    hash echo 2>/dev/null
    echo 2>/dev/null
    hash 2>/dev/null
    echo pass
  expect
    stdout "(.|\n)*pass"
    stderr ""
    exit_code 0
end test "hash output goes to stdout"
```

#### Test: PATH change clears hash table

POSIX specifies that the remembered-locations list shall be cleared when the contents of PATH are changed. Re-assigning `PATH="$PATH"` should purge the hash table even though the value is the same.

```
begin test "PATH change clears hash table"
  script
    hash -r
    hash echo 2>/dev/null
    echo >/dev/null
    PATH="$PATH"
    hash 2>/dev/null | wc -l
  expect
    stdout " *0"
    stderr ""
    exit_code 0
end test "PATH change clears hash table"
```
