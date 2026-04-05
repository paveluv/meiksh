# Test Suite for Intrinsic Utility: umask

This test suite covers the **umask** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: umask](#utility-umask)

## utility: umask

#### NAME

> umask — get or set the file mode creation mask

#### SYNOPSIS

> `umask [-S] [mask]`

#### DESCRIPTION

> The *umask* utility shall set the file mode creation mask of the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)) to the value specified by the *mask* operand. This mask shall affect the initial value of the file permission bits of subsequently created files. If *umask* is called in a subshell or separate utility execution environment, such as one of the following:
>
> ```
> (umask 002)
> nohup umask ...
> find . -exec umask ... \;
> ```
>
> it shall not affect the file mode creation mask of the caller's environment.
>
> If the *mask* operand is not specified, the *umask* utility shall write to standard output the value of the file mode creation mask of the invoking process.

#### OPTIONS

> The *umask* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following option shall be supported:
>
> - **-S**: Produce symbolic output.
>
> The default output style is unspecified, but shall be recognized on a subsequent invocation of *umask* on the same system as a *mask* operand to restore the previous file mode creation mask.

#### OPERANDS

> The following operand shall be supported:
>
> - *mask*: A string specifying the new file mode creation mask. The string is treated in the same way as the *mode* operand described in the EXTENDED DESCRIPTION section for [*chmod*](docs/posix/md/utilities/chmod.md). For a *symbolic_mode* value, the new value of the file mode creation mask shall be the logical complement of the file permission bits portion of the file mode specified by the *symbolic_mode* string. In a *symbolic_mode* value, the permissions *op* characters `'+'` and `'-'` shall be interpreted relative to the current file mode creation mask; `'+'` shall cause the bits for the indicated permissions to be cleared in the mask; `'-'` shall cause the bits for the indicated permissions to be set in the mask. The interpretation of *mode* values that specify file mode bits other than the file permission bits is unspecified. In the octal integer form of *mode*, the specified bits are set in the file mode creation mask. The file mode creation mask shall be set to the resulting numeric value. The default output of a prior invocation of *umask* on the same system with no operand also shall be recognized as a *mask* operand.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *umask*:
>
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> When the *mask* operand is not specified, the *umask* utility shall write a message to standard output that can later be used as a *umask* *mask* operand.
>
> If **-S** is specified, the message shall be in the following format:
>
> ```
> "u=%s,g=%s,o=%s\n", <owner permissions>, <group permissions>,
>     <other permissions>
> ```
>
> where the three values shall be combinations of letters from the set {*r*, *w*, *x*}; the presence of a letter shall indicate that the corresponding bit is clear in the file mode creation mask.
>
> If a *mask* operand is specified, there shall be no output written to standard output.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: The file mode creation mask was successfully changed, or no *mask* operand was supplied.
> - \>0: An error occurred.

#### CONSEQUENCES OF ERRORS

> Default.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *umask* affects the current shell execution environment, it is generally provided as a shell regular built-in.
>
> In contrast to the negative permission logic provided by the file mode creation mask and the octal number form of the *mask* argument, the symbolic form of the *mask* argument specifies those permissions that are left alone.

#### EXAMPLES

> ```
> umask a=rx,ug+w
> umask 002
> ```

#### RATIONALE

> Since *umask* affects the current shell execution environment, it is generally provided as a shell regular built-in. If it is called in a subshell or separate utility execution environment, such as one of the following:
>
> ```
> (umask 002)
> nohup umask ...
> find . -exec umask ... \;
> ```
>
> it does not affect the file mode creation mask of the environment of the caller.
>
> The description of the historical utility was modified to allow it to use the symbolic modes of [*chmod*](docs/posix/md/utilities/chmod.md). The **-s** option used in early proposals was changed to **-S** because **-s** could be confused with a *symbolic_mode* form of mask referring to the S_ISUID and S_ISGID bits.
>
> The default output style is unspecified to permit implementors to provide migration to the new symbolic style at the time most appropriate to their users. A **-o** flag to force octal mode output was omitted because the octal mode may not be sufficient to specify all of the information that may be present in the file mode creation mask when more secure file access permission checks are implemented.
>
> It has been suggested that trusted systems developers might appreciate ameliorating the requirement that the mode mask "affects" the file access permissions, since it seems access control lists might replace the mode mask to some degree. The wording has been changed to say that it affects the file permission bits, and it leaves the details of the behavior of how they affect the file access permissions to the description in the System Interfaces volume of POSIX.1-2024.

#### FUTURE DIRECTIONS

> None.

#### SEE ALSO

> [*2. Shell Command Language*](docs/posix/md/utilities/V3_chap02.md#2-shell-command-language), [*chmod*](docs/posix/md/utilities/chmod.md#tag_20_17)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*umask*()](docs/posix/md/functions/umask.md#tag_17_645)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> The following new requirements on POSIX implementations derive from alignment with the Single UNIX Specification:
>
> - The octal mode is supported.
>
> IEEE Std 1003.1-2001/Cor 2-2004, item XCU/TC2/D6/34 is applied, making a correction to the RATIONALE.

#### Issue 7

> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0197 [584] is applied.

#### Issue 8

> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*

*End of informative text.*

### Tests

#### Test: umask set and query octal value

`umask` with an octal operand sets the file mode creation mask;
without operands it writes the current mask.

```
begin test "umask set and query octal value"
  script
    umask 0022
    umask
  expect
    stdout "0*022"
    stderr ""
    exit_code 0
end test "umask set and query octal value"
```

#### Test: umask -S prints symbolic mode

`umask -S` writes the file mode creation mask in symbolic form.

```
begin test "umask -S prints symbolic mode"
  script
    umask 0022
    umask -S
  expect
    stdout "u=rwx,g=rx,o=rx"
    stderr ""
    exit_code 0
end test "umask -S prints symbolic mode"
```

#### Test: umask affects file permissions

The file mode creation mask restricts permissions on new files.

```
begin test "umask affects file permissions"
  script
    umask 0077
    touch tmp_umask_test
    ls -l tmp_umask_test | cut -c1-10
  expect
    stdout "-rw-------"
    stderr ""
    exit_code 0
end test "umask affects file permissions"
```

#### Test: umask symbolic mode a=rx

The mask operand may use symbolic notation like `a=rx`. This sets permission bits for all classes to read+execute, which corresponds to an octal mask of 222 (write bits masked for user, group, and other).

```
begin test "umask symbolic mode a=rx"
  script
    umask 022
    umask a=rx
    val=$(umask)
    echo "$val" | grep -q "222" && echo "pass" || echo "fail: $val"
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "umask symbolic mode a=rx"
```

#### Test: umask symbolic relative g-w

In symbolic mode the `-` operator sets bits in the mask (removes permissions). Starting from 022, `g-w` means "remove group write permission", but group write is already masked, so the effective mask remains the same and `-S` should still show `u=rwx,g=rx,o=rx`.

```
begin test "umask symbolic relative g-w"
  script
    umask 022
    umask g-w
    val=$(umask -S)
    echo "$val"
  expect
    stdout "u=rwx,g=rx,o=rx"
    stderr ""
    exit_code 0
end test "umask symbolic relative g-w"
```

#### Test: umask 0022 exits successfully

Setting the file mode creation mask with an octal operand must succeed with exit status 0 and produce no output on stdout or stderr.

```
begin test "umask 0022 exits successfully"
  script
    umask 0022
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "umask 0022 exits successfully"
```

#### Test: umask 0022 reports correct value

After setting the mask to 0022, invoking `umask` with no operand must print a value that matches `022`, confirming the mask was stored correctly.

```
begin test "umask 0022 reports correct value"
  script
    umask 0022
    umask
  expect
    stdout "0*022"
    stderr ""
    exit_code 0
end test "umask 0022 reports correct value"
```

#### Test: umask with no operand produces output

When no mask operand is specified, `umask` must write the current file mode creation mask to standard output. This verifies that at least some non-empty output is produced.

```
begin test "umask with no operand produces output"
  script
    umask 0027
    umask
  expect
    stdout ".+"
    stderr ""
    exit_code 0
end test "umask with no operand produces output"
```

#### Test: umask with no operand reflects set value

After setting the mask to 0027, querying with `umask` (no operand) must output a value matching `027`, proving the query reflects the most recently set mask.

```
begin test "umask with no operand reflects set value"
  script
    umask 0027
    umask
  expect
    stdout "0*027"
    stderr ""
    exit_code 0
end test "umask with no operand reflects set value"
```

#### Test: umask set exits 0

Successfully changing the file mode creation mask must return exit status 0, with no output on stdout or stderr.

```
begin test "umask set exits 0"
  script
    umask 0022
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "umask set exits 0"
```

#### Test: umask query exits 0

Invoking `umask` with no operand to query the current mask must exit with status 0, as specified by POSIX for a successful query.

```
begin test "umask query exits 0"
  script
    umask
  expect
    stdout "(.|\n)*"
    stderr ""
    exit_code 0
end test "umask query exits 0"
```

#### Test: umask octal operand 0037

Setting the mask to 0037 and then querying it must report `037`, verifying that an arbitrary octal operand is accepted and stored correctly.

```
begin test "umask octal operand 0037"
  script
    umask 0037
    umask
  expect
    stdout "0*037"
    stderr ""
    exit_code 0
end test "umask octal operand 0037"
```

#### Test: umask symbolic operand u=rwx,g=rx,o=

A symbolic operand like `u=rwx,g=rx,o=` sets the mask so that user keeps all permissions, group keeps read+execute, and other gets nothing. The resulting octal mask should be 027.

```
begin test "umask symbolic operand u=rwx,g=rx,o="
  script
    umask u=rwx,g=rx,o=
    umask
  expect
    stdout "0*027"
    stderr ""
    exit_code 0
end test "umask symbolic operand u=rwx,g=rx,o="
```

#### Test: umask -S produces symbolic output

The `-S` option causes `umask` to produce symbolic output in the format `u=...,g=...,o=...`. This verifies that all three class fields appear in the output.

```
begin test "umask -S produces symbolic output"
  script
    umask 0022
    umask -S
  expect
    stdout ".*u=.*g=.*o=.*"
    stderr ""
    exit_code 0
end test "umask -S produces symbolic output"
```

#### Test: umask -S reflects mask 0077

With mask 0077 (group and other fully masked), `umask -S` must show that the user retains full permissions (`u=rwx`) while group and other permissions are restricted accordingly.

```
begin test "umask -S reflects mask 0077"
  script
    umask 0077
    umask -S
  expect
    stdout ".*u=rwx.*g=.*o=.*"
    stderr ""
    exit_code 0
end test "umask -S reflects mask 0077"
```

#### Test: umask -S with mask 0000 shows all permissions

A mask of 0000 means no permission bits are masked. The symbolic output must therefore show full permissions for every class: `u=rwx,g=rwx,o=rwx`.

```
begin test "umask -S with mask 0000 shows all permissions"
  script
    umask 0000
    umask -S
  expect
    stdout ".*u=rwx.*g=rwx.*o=rwx.*"
    stderr ""
    exit_code 0
end test "umask -S with mask 0000 shows all permissions"
```

#### Test: subshell umask does not affect parent

POSIX requires that a `umask` executed in a subshell does not affect the file mode creation mask of the caller's environment. After a subshell changes the mask, the parent's mask must remain unchanged.

```
begin test "subshell umask does not affect parent"
  script
    umask 0022
    (umask 0077)
    umask
  expect
    stdout "0*022"
    stderr ""
    exit_code 0
end test "subshell umask does not affect parent"
```

#### Test: subshell and parent masks are independent

A subshell gets its own copy of the file mode creation mask. The subshell mask and the parent mask must be reported independently, each reflecting only their own `umask` calls.

```
begin test "subshell and parent masks are independent"
  script
    umask 0022
    (umask 0077; echo inner=$(umask))
    echo outer=$(umask)
  expect
    stdout ".*inner=.*077.*\n.*outer=.*022.*"
    stderr ""
    exit_code 0
end test "subshell and parent masks are independent"
```

#### Test: umask default output round-trip

POSIX requires that the default output of `umask` (no `-S`) can be used as a mask operand on a subsequent invocation to restore the same mask. This test sets a mask, captures the output, clears the mask, restores it, and checks it matches.

```
begin test "umask default output round-trip"
  script
    umask 0037
    _saved=$(umask)
    umask 0000
    umask "$_saved"
    _restored=$(umask)
    [ "$_saved" = "$_restored" ] && echo roundtrip_ok || echo MISMATCH
  expect
    stdout "roundtrip_ok"
    stderr ""
    exit_code 0
end test "umask default output round-trip"
```

#### Test: umask default output as mask operand round-trip

Same round-trip guarantee as the previous test but with a different mask value (0055), confirming the default output format is always reusable as an operand.

```
begin test "umask default output as mask operand round-trip"
  script
    umask 0055
    _saved=$(umask)
    umask 0000
    umask "$_saved"
    _restored=$(umask)
    [ "$_saved" = "$_restored" ] && echo roundtrip_ok || echo MISMATCH
  expect
    stdout "roundtrip_ok"
    stderr ""
    exit_code 0
end test "umask default output as mask operand round-trip"
```

#### Test: umask with octal operand produces no stdout

POSIX specifies that when a mask operand is provided, there shall be no output written to standard output. This verifies that `umask 0022` produces empty stdout.

```
begin test "umask with octal operand produces no stdout"
  script
    umask 0022
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "umask with octal operand produces no stdout"
```

#### Test: umask with symbolic operand produces no stdout

Likewise, when a symbolic mask operand is provided, `umask` must produce no standard output. This checks the symbolic-operand case.

```
begin test "umask with symbolic operand produces no stdout"
  script
    umask u=rwx,g=rx,o=rx
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "umask with symbolic operand produces no stdout"
```
