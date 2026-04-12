# Test Suite for Intrinsic Utility: cd

This test suite covers the **cd** intrinsic utility
as specified by POSIX.1-2024 (Section 1.7).

## Table of contents

- [utility: cd](#utility-cd)

## utility: cd

#### NAME

> cd — change the working directory

#### SYNOPSIS

> ```
> cd [-L] [directory]
> cd -P [-e] [directory]
> ```

#### DESCRIPTION

> The *cd* utility shall change the working directory of the current shell execution environment (see [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment)) by executing the following steps in sequence. (In the following steps, the symbol **curpath** represents an intermediate value used to simplify the description of the algorithm used by *cd*. There is no requirement that **curpath** be made visible to the application.)
>
> 1. If no *directory* operand is given and the *HOME* environment variable is empty or undefined, the default behavior is implementation-defined and no further steps shall be taken.
> 2. If no *directory* operand is given and the *HOME* environment variable is set to a non-empty value, the *cd* utility shall behave as if the directory named in the *HOME* environment variable was specified as the *directory* operand.
> 3. If the *directory* operand begins with a `<slash>` character, set **curpath** to the operand and proceed to step 7.
> 4. If the first component of the *directory* operand is dot or dot-dot, proceed to step 6.
> 5. Starting with the first pathname in the `<colon>`-separated pathnames of *CDPATH* (see the ENVIRONMENT VARIABLES section) if the pathname is non-null, test if the concatenation of that pathname, a `<slash>` character if that pathname did not end with a `<slash>` character, and the *directory* operand names a directory. If the pathname is null, test if the concatenation of dot, a `<slash>` character, and the operand names a directory. In either case, if the resulting string names an existing directory, set **curpath** to that string and proceed to step 7. Otherwise, repeat this step with the next pathname in *CDPATH* until all pathnames have been tested.
> 6. Set **curpath** to the *directory* operand.
> 7. If the **-P** option is in effect, proceed to step 10. If **curpath** does not begin with a `<slash>` character, set **curpath** to the string formed by the concatenation of the value of *PWD ,* a `<slash>` character if the value of *PWD* did not end with a `<slash>` character, and **curpath**.
> 8. The **curpath** value shall then be converted to canonical form as follows, considering each component from beginning to end, in sequence:
>     1. Dot components and any `<slash>` characters that separate them from the next component shall be deleted.
>     2. For each dot-dot component, if there is a preceding component and it is neither root nor dot-dot, then:
>           1. If the preceding component does not refer (in the context of pathname resolution with symbolic links followed) to a directory, then the *cd* utility shall display an appropriate error message and no further steps shall be taken.
>           2. The preceding component, all `<slash>` characters separating the preceding component from dot-dot, dot-dot, and all `<slash>` characters separating dot-dot from the following component (if any) shall be deleted.
>     3. An implementation may further simplify **curpath** by removing any trailing `<slash>` characters that are not also leading `<slash>` characters, replacing multiple non-leading consecutive `<slash>` characters with a single `<slash>`, and replacing three or more leading `<slash>` characters with a single `<slash>`. If, as a result of this canonicalization, the **curpath** variable is null, no further steps shall be taken.
> 9. If **curpath** is longer than {PATH_MAX} bytes (including the terminating null) and the *directory* operand was not longer than {PATH_MAX} bytes (including the terminating null), then **curpath** shall be converted from an absolute pathname to an equivalent relative pathname if possible. This conversion shall always be considered possible if the value of *PWD ,* with a trailing `<slash>` added if it does not already have one, is an initial substring of **curpath**. Whether or not it is considered possible under other circumstances is unspecified. Implementations may also apply this conversion if **curpath** is not longer than {PATH_MAX} bytes or the *directory* operand was longer than {PATH_MAX} bytes.
> 10. The *cd* utility shall then perform actions equivalent to the [*chdir*()](docs/posix/md/functions/chdir.md) function called with **curpath** as the *path* argument. If these actions fail for any reason, the *cd* utility shall display an appropriate error message and the remainder of this step shall not be executed. If the **-P** option is not in effect, the *PWD* environment variable shall be set to the value that **curpath** had on entry to step 9 (i.e., before conversion to a relative pathname). If the **-P** option is in effect, the *PWD* environment variable shall be set to the string that would be output by [*pwd*](docs/posix/md/utilities/pwd.md) **-P**. If there is insufficient permission on the new directory, or on any parent of that directory, to determine the current working directory, the value of the *PWD* environment variable is unspecified. If both the **-e** and the **-P** options are in effect and *cd* is unable to determine the pathname of the current working directory, *cd* shall complete successfully but return a non-zero exit status.
>
> If, during the execution of the above steps, the *PWD* environment variable is set, the *OLDPWD* shell variable shall also be set to the value of the old working directory (that is the current working directory immediately prior to the call to *cd*). It is unspecified whether, when setting *OLDPWD ,* the shell also causes it to be exported if it was not already.

#### OPTIONS

> The *cd* utility shall conform to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines).
>
> The following options shall be supported by the implementation:
>
> - **-e**: If the **-P** option is in effect, the current working directory is successfully changed, and the correct value of the *PWD* environment variable cannot be determined, exit with exit status 1.
> - **-L**: Handle the operand dot-dot logically; symbolic link components shall not be resolved before dot-dot components are processed (see steps 8. and 9. in the DESCRIPTION).
> - **-P**: Handle the operand dot-dot physically; symbolic link components shall be resolved before dot-dot components are processed (see step 7. in the DESCRIPTION).
>
> If both **-L** and **-P** options are specified, the last of these options shall be used and all others ignored. If neither **-L** nor **-P** is specified, the operand shall be handled dot-dot logically; see the DESCRIPTION.

#### OPERANDS

> The following operands shall be supported:
>
> - *directory*: An absolute or relative pathname of the directory that shall become the new working directory. The interpretation of a relative pathname by *cd* depends on the **-L** option and the *CDPATH* and *PWD* environment variables. If *directory* is an empty string, *cd* shall write a diagnostic message to standard error and exit with non-zero status. If *directory* consists of a single `'-'` (`<hyphen-minus>`) character, the *cd* utility shall behave as if *directory* contained the value of the *OLDPWD* environment variable, except that after it sets the value of *PWD* it shall write the new value to standard output. The behavior is unspecified if *OLDPWD* does not start with a `<slash>` character.

#### STDIN

> Not used.

#### INPUT FILES

> None.

#### ENVIRONMENT VARIABLES

> The following environment variables shall affect the execution of *cd*:
>
> - *CDPATH*: A `<colon>`-separated list of pathnames that refer to directories. The *cd* utility shall use this list in its attempt to change the directory, as described in the DESCRIPTION. An empty string in place of a directory pathname represents the current directory. If *CDPATH* is not set, it shall be treated as if it were an empty string.
> - *HOME*: The name of the directory, used when no *directory* operand is specified.
> - *LANG*: Provide a default value for the internationalization variables that are unset or null. (See XBD [*8.2 Internationalization Variables*](docs/posix/md/basedefs/V1_chap08.md#82-internationalization-variables) for the precedence of internationalization variables used to determine the values of locale categories.)
> - *LC_ALL*: If set to a non-empty string value, override the values of all the other internationalization variables.
> - *LC_CTYPE*: Determine the locale for the interpretation of sequences of bytes of text data as characters (for example, single-byte as opposed to multi-byte characters in arguments).
> - *LC_MESSAGES*: Determine the locale that should be used to affect the format and contents of diagnostic messages written to standard error.
> - *NLSPATH*: Determine the location of messages objects and message catalogs.
> - *OLDPWD*: A pathname of the previous working directory, used when the operand is `'-'`. If an application sets or unsets the value of *OLDPWD ,* the behavior of *cd* with a `'-'` operand is unspecified.
> - *PWD*: This variable shall be set as specified in the DESCRIPTION. If an application sets or unsets the value of *PWD ,* the behavior of *cd* is unspecified.

#### ASYNCHRONOUS EVENTS

> Default.

#### STDOUT

> If a non-empty directory name from *CDPATH* is used, or if the operand `'-'` is used, and the absolute pathname of the new working directory can be determined, that pathname shall be written to the standard output as follows:
>
> ```
> "%s\n", <new directory>
> ```
>
> If an absolute pathname of the new current working directory cannot be determined, it is unspecified whether nothing is written to the standard output or the value of **curpath** used in step 10, followed by a `<newline>`, is written to the standard output.
>
> If a non-empty directory name from *CDPATH* is not used, and the directory argument is not `'-'`, there shall be no output.

#### STDERR

> The standard error shall be used only for diagnostic messages.

#### OUTPUT FILES

> None.

#### EXTENDED DESCRIPTION

> None.

#### EXIT STATUS

> The following exit values shall be returned:
>
> - 0: The current working directory was successfully changed and the value of the *PWD* environment variable was set correctly.
> - 0: The current working directory was successfully changed, the **-e** option is not in effect, the **-P** option is in effect, and the correct value of the *PWD* environment variable could not be determined.
> - \>0: Either the **-e** option or the **-P** option is not in effect, and an error occurred.
> - 1: The current working directory was successfully changed, both the **-e** and the **-P** options are in effect, and the correct value of the *PWD* environment variable could not be determined.
> - \>1: Both the **-e** and the **-P** options are in effect, and an error occurred.

#### CONSEQUENCES OF ERRORS

> The working directory shall remain unchanged.

---

*The following sections are informative.*

#### APPLICATION USAGE

> This utility is required to be intrinsic. See [*1.7 Intrinsic Utilities*](docs/posix/md/utilities/V3_chap01.md#17-intrinsic-utilities) for details.
>
> Since *cd* affects the current shell execution environment, it is always provided as a shell regular built-in. If it is called in a subshell or separate utility execution environment, such as one of the following:
>
> ```
> (cd /tmp)
> nohup cd
> find . -exec cd {} \;
> ```
>
> it does not affect the working directory of the caller's environment.
>
> The user must have execute (search) permission in *directory* in order to change to it.
>
> Since *cd* treats the operand `'-'` as a special case, applications should not pass arbitrary values as the operand. For example, instead of:
>
> ```
> CDPATH= cd -P -- "$dir"
> ```
>
> applications should use the following:
>
> ```
> case $dir in
> (/*) cd -P "$dir";;
> ("") echo >&2 directory is an empty string; exit 1;;
> (*) CDPATH= cd -P "./$dir";;
> esac
> ```
>
> If an absolute pathname of the new current working directory cannot be determined, and a non-empty directory name from *CDPATH* is used, *cd* may write a pathname to standard output that is not an absolute pathname.

#### EXAMPLES

> The following template can be used to perform processing in the directory specified by *location* and end up in the current working directory in use before the first *cd* command was issued:
>
> ```
> cd location
> if [ $? -ne 0 ]
> then
>     print error message
>     exit 1
> fi
> ... do whatever is desired as long as the OLDPWD environment variable
>     is not modified
> cd -
> ```

#### RATIONALE

> The use of the *CDPATH* was introduced in the System V shell. Its use is analogous to the use of the *PATH* variable in the shell. The BSD C shell used a shell parameter *cdpath* for this purpose.
>
> A common extension when *HOME* is undefined is to get the login directory from the user database for the invoking user. This does not occur on System V implementations.
>
> Some historical shells, such as the KornShell, took special actions when the directory name contained a dot-dot component, selecting the logical parent of the directory, rather than the actual parent directory; that is, it moved up one level toward the `'/'` in the pathname, remembering what the user typed, rather than performing the equivalent of:
>
> ```
> chdir("..");
> ```
>
> In such a shell, the following commands would not necessarily produce equivalent output for all directories:
>
> ```
> cd .. && ls      ls ..
> ```
>
> This behavior is now the default. It is not consistent with the definition of dot-dot in most historical practice; that is, while this behavior has been optionally available in the KornShell, other shells have historically not supported this functionality. The logical pathname is stored in the *PWD* environment variable when the *cd* utility completes and this value is used to construct the next directory name if *cd* is invoked with the **-L** option.
>
> When the **-P** option is in effect, the correct value of the *PWD* environment variable cannot be determined on some systems, but still results in a zero exit status. The value of *PWD* doesn't matter to some shell scripts and in those cases this is not a problem. In other cases, especially with multiple calls to *cd*, the values of *PWD* and *OLDPWD* are important but the standard provided no easy way to know that this was the case. The **-e** option has been added, even though this was not historic practice, to give script writers a reliable way to know when the value of *PWD* is not reliable.

#### FUTURE DIRECTIONS

> If this utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used, implementations are encouraged to treat this as an error. A future version of this standard may require implementations to treat this as an error.

#### SEE ALSO

> [*2.13 Shell Execution Environment*](docs/posix/md/utilities/V3_chap02.md#213-shell-execution-environment), [*pwd*](docs/posix/md/utilities/pwd.md)
>
> XBD [*8. Environment Variables*](docs/posix/md/basedefs/V1_chap08.md#8-environment-variables), [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines)
>
> XSH [*chdir*()](docs/posix/md/functions/chdir.md)

#### CHANGE HISTORY

> First released in Issue 2.

#### Issue 6

> The following new requirements on POSIX implementations derive from alignment with the Single UNIX Specification:
>
> - The *cd* **-** operand, *PWD ,* and *OLDPWD* are added.
>
> The **-L** and **-P** options are added to align with the IEEE P1003.2b draft standard. This also includes the introduction of a new description to include the effect of these options.
>
> IEEE Std 1003.1-2001/Cor 1-2002, item XCU/TC1/D6/14 is applied, changing the SYNOPSIS to make it clear that the **-L** and **-P** options are mutually-exclusive.

#### Issue 7

> Austin Group Interpretation 1003.1-2001 #037 is applied.
>
> Austin Group Interpretation 1003.1-2001 #199 is applied, clarifying how the *cd* utility handles concatenation of two pathnames when the first pathname ends in a `<slash>` character.
>
> SD5-XCU-ERN-97 is applied, updating the SYNOPSIS.
>
> Step 7 of the processing performed by *cd* is revised to refer to **curpath** instead of "the operand".
>
> Changes to the [*pwd*](docs/posix/md/utilities/pwd.md) utility and *PWD* environment variable have been made to match the changes to the [*getcwd*()](docs/posix/md/functions/getcwd.md) function made for Austin Group Interpretation 1003.1-2001 #140.
>
> POSIX.1-2008, Technical Corrigendum 1, XCU/TC1-2008/0076 [230], XCU/TC1-2008/0077 [240], XCU/TC1-2008/0078 [240], and XCU/TC1-2008/0079 [123] are applied.
>
> POSIX.1-2008, Technical Corrigendum 2, XCU/TC2-2008/0074 [584] is applied.

#### Issue 8

> Austin Group Defect 251 is applied, encouraging implementations to report an error if a utility is directed to display a pathname that contains any bytes that have the encoded value of a `<newline>` character when `<newline>` is a terminator or separator in the output format being used.
>
> Austin Group Defect 253 is applied, adding the **-e** option.
>
> Austin Group Defect 854 is applied, adding a note to the APPLICATION USAGE section that this utility is required to be intrinsic.
>
> Austin Group Defect 1045 is applied, clarifying the behavior when the *directory* operand is `'-'`.
>
> Austin Group Defect 1047 is applied, requiring *cd* to treat an empty *directory* operand as an error
>
> Austin Group Defect 1122 is applied, changing the description of *NLSPATH .*
>
> Austin Group Defect 1527 is applied, clarifying the behavior when an absolute pathname of the new current working directory cannot be determined.
>
> Austin Group Defect 1601 is applied, clarifying that when setting *OLDPWD ,* the shell need not cause it to be exported if it was not already.

*End of informative text.*

### Tests

#### Test: cd into directory and cd - toggle

`cd` changes the working directory; `cd -` returns to the previous
directory and prints the old directory name.

```
begin test "cd into directory and cd - toggle"
  script
    orig=$PWD
    mkdir -p tmp_cd_dir
    cd tmp_cd_dir
    new=$PWD
    cd - >/dev/null
    echo "$PWD"
    test "$PWD" = "$orig" && echo "back"
  expect
    stdout ".*\nback"
    stderr ""
    exit_code 0
end test "cd into directory and cd - toggle"
```

#### Test: cd default is logical (symlink)

By default `cd` uses logical pathname traversal (`-L`), preserving
symbolic link components.

```
begin test "cd default is logical (symlink)"
  script
    mkdir -p tmp_cd_real
    ln -sf tmp_cd_real tmp_cd_link
    cd tmp_cd_link
    case "$PWD" in *tmp_cd_link*) echo "logical" ;; *) echo "physical" ;; esac
  expect
    stdout "logical"
    stderr ""
    exit_code 0
end test "cd default is logical (symlink)"
```

#### Test: cd -P is physical (symlink)

`cd -P` resolves symbolic links to the physical directory.

```
begin test "cd -P is physical (symlink)"
  script
    mkdir -p tmp_cd_real2
    ln -sf tmp_cd_real2 tmp_cd_link2
    cd -P tmp_cd_link2
    case "$PWD" in *tmp_cd_link2*) echo "logical" ;; *) echo "physical" ;; esac
  expect
    stdout "physical"
    stderr ""
    exit_code 0
end test "cd -P is physical (symlink)"
```

#### Test: cd with no args goes to HOME

When no operand is given, `cd` changes to the directory named by HOME.

```
begin test "cd with no args goes to HOME"
  script
    mkdir -p tmp_cd_home
    HOME=$PWD/tmp_cd_home
    cd
    test "$PWD" = "$HOME" && echo "home"
  expect
    stdout "home"
    stderr ""
    exit_code 0
end test "cd with no args goes to HOME"
```

#### Test: cd to nonexistent path fails

`cd` to a nonexistent directory fails with a non-zero exit status.

```
begin test "cd to nonexistent path fails"
  script
    cd /nonexistent_dir_xyz 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "cd to nonexistent path fails"
```

#### Test: cd - output prints old directory

When `cd -` is used, the shell prints the absolute pathname of the
previous working directory to standard output.

```
begin test "cd - output prints old directory"
  script
    mkdir -p abc
    old=$PWD
    cd abc
    cd - | grep -q "^$old" && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "cd - output prints old directory"
```

#### Test: cd - with unset OLDPWD fails

When OLDPWD is unset, `cd -` has no previous directory to return to
and must fail with a non-zero exit status.

```
begin test "cd - with unset OLDPWD fails"
  script
    unset OLDPWD
    cd - 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "cd - with unset OLDPWD fails"
```

#### Test: cd conforms to utility syntax guidelines

Verifies that `cd` accepts a plain directory operand and successfully
changes into it, consistent with the POSIX utility syntax guidelines.

```
begin test "cd conforms to utility syntax guidelines"
  script
    mkdir -p syndir
    cd syndir
    echo "$PWD" | grep -q syndir && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "cd conforms to utility syntax guidelines"
```

#### Test: cd -L -P last option wins

When both `-L` and `-P` are specified, the last one takes precedence.
Here `-P` is last, so symlinks are resolved to the physical path.

```
begin test "cd -L -P last option wins"
  script
    mkdir -p combo_real
    ln -s combo_real link_combo
    cd -L -P link_combo
    case "$PWD" in */combo_real) echo pass ;; *) echo fail ;; esac
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "cd -L -P last option wins"
```

#### Test: cd -eP to valid directory succeeds

When `-e` and `-P` are both in effect and the target directory is valid,
`cd` succeeds with exit status zero because PWD can be determined.

```
begin test "cd -eP to valid directory succeeds"
  script
    mkdir -p edir
    cd -eP edir
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "cd -eP to valid directory succeeds"
```

#### Test: CDPATH search finds directory

CDPATH provides a colon-separated list of directories to search when the
operand is a relative path; `cd` should locate the target under CDPATH.

```
begin test "CDPATH search finds directory"
  script
    mkdir -p /tmp/_cd_epty_cdpath/searchdir/target
    CDPATH=/tmp/_cd_epty_cdpath/searchdir
    export CDPATH
    cd target >/dev/null
    case "$PWD" in */target) echo pass ;; *) echo fail ;; esac
    rm -rf /tmp/_cd_epty_cdpath
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "CDPATH search finds directory"
```

#### Test: cd without CDPATH produces no stdout

When CDPATH is not set and the operand is not `-`, `cd` must not write
anything to standard output.

```
begin test "cd without CDPATH produces no stdout"
  script
    mkdir -p local_only_cd
    unset CDPATH
    output=$(cd local_only_cd 2>&1)
    [ -z "$output" ] && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "cd without CDPATH produces no stdout"
```

#### Test: single hyphen-minus operand treated as first operand

A single `-` operand tells `cd` to change to OLDPWD, the previous
working directory, and the shell should end up back where it was before
the most recent directory change.

```
begin test "single hyphen-minus operand treated as first operand"
  script
    mkdir -p hyp_test
    cd hyp_test
    first=$PWD
    cd ..
    cd - >/dev/null
    [ "$PWD" = "$first" ] && echo pass || echo fail
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "single hyphen-minus operand treated as first operand"
```

#### Test: cd -L keeps logical symlink path in PWD

With `-L` (logical mode), `cd` preserves symbolic link names in PWD
rather than resolving them to the physical directory.

```
begin test "cd -L keeps logical symlink path in PWD"
  script
    mkdir -p real_target/sub
    ln -s real_target link_l
    cd -L link_l
    case "$PWD" in */link_l) echo pass_pwd_logical ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_pwd_logical"
    stderr ""
    exit_code 0
end test "cd -L keeps logical symlink path in PWD"
```

#### Test: cd -L into symlink subdirectory

When `cd -L` navigates through a symlink into a subdirectory, PWD
should retain the logical (symlink-based) path, not the resolved one.

```
begin test "cd -L into symlink subdirectory"
  script
    mkdir -p real_target2/sub
    ln -s real_target2 link_l2
    cd -L link_l2/sub
    case "$PWD" in */link_l2/sub) echo pass_logical_sub ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_logical_sub"
    stderr ""
    exit_code 0
end test "cd -L into symlink subdirectory"
```

#### Test: cd -L .. goes to logical parent

In logical mode, `cd -L ..` moves to the logical parent of the current
path (removing the last path component from PWD) rather than performing
a physical `chdir("..")`.

```
begin test "cd -L .. goes to logical parent"
  script
    mkdir -p real_deep/child
    ln -s real_deep link_deep
    cd -L link_deep/child
    cd -L ..
    case "$PWD" in */link_deep) echo pass_logical_dotdot ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_logical_dotdot"
    stderr ""
    exit_code 0
end test "cd -L .. goes to logical parent"
```

#### Test: cd -P resolves symlink to real path

With `-P` (physical mode), `cd` resolves all symbolic links and sets
PWD to the real, physical directory path.

```
begin test "cd -P resolves symlink to real path"
  script
    mkdir -p phys_real/inner
    ln -s phys_real link_p
    cd -P link_p
    case "$PWD" in */phys_real) echo pass_physical ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_physical"
    stderr ""
    exit_code 0
end test "cd -P resolves symlink to real path"
```

#### Test: cd -P .. resolves real path first

With `-P`, symlinks are resolved before `..` is processed, so `cd -P ..`
ascends to the physical parent of the resolved directory, not the
logical parent.

```
begin test "cd -P .. resolves real path first"
  script
    mkdir -p phys_real2/inner2
    ln -s phys_real2/inner2 link_p2
    cd -P link_p2
    cd -P ..
    case "$PWD" in */phys_real2) echo pass_physical_dotdot ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_physical_dotdot"
    stderr ""
    exit_code 0
end test "cd -P .. resolves real path first"
```

#### Test: -L -P last option P wins

When `-L` and `-P` are given together, the last one specified takes
precedence. Here `-P` appears last, so symlinks are physically resolved.

```
begin test "-L -P last option P wins"
  script
    mkdir -p combo_real
    ln -s combo_real link_combo
    cd -L -P link_combo
    case "$PWD" in */combo_real) echo pass_last_P ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_last_P"
    stderr ""
    exit_code 0
end test "-L -P last option P wins"
```

#### Test: -P -L last option L wins

When `-P` and `-L` are given together, the last one specified takes
precedence. Here `-L` appears last, so symlink names are preserved in
PWD.

```
begin test "-P -L last option L wins"
  script
    mkdir -p combo_real2
    ln -s combo_real2 link_combo2
    cd -P -L link_combo2
    case "$PWD" in */link_combo2) echo pass_last_L ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_last_L"
    stderr ""
    exit_code 0
end test "-P -L last option L wins"
```

#### Test: CDPATH finds target directory

When CDPATH is set, `cd` searches its entries for a matching subdirectory
and changes into the first match found.

```
begin test "CDPATH finds target directory"
  script
    mkdir -p /tmp/_cd_test_cdpath/searchdir/target_dir
    CDPATH=/tmp/_cd_test_cdpath/searchdir
    export CDPATH
    cd target_dir >/dev/null
    case "$PWD" in */target_dir) echo pass_cdpath ;; *) echo "fail: PWD=$PWD" ;; esac
    rm -rf /tmp/_cd_test_cdpath
  expect
    stdout "pass_cdpath"
    stderr ""
    exit_code 0
end test "CDPATH finds target directory"
```

#### Test: CDPATH with multiple colon-separated entries

CDPATH entries are colon-separated and searched in order; `cd` should
find the target in a later entry when the earlier entries do not contain
it.

```
begin test "CDPATH with multiple colon-separated entries"
  script
    mkdir -p /tmp/_cd_test_cp2/a /tmp/_cd_test_cp2/b/found_here
    CDPATH=/tmp/_cd_test_cp2/a:/tmp/_cd_test_cp2/b
    export CDPATH
    cd found_here >/dev/null
    case "$PWD" in */found_here) echo pass_cdpath_multi ;; *) echo "fail: PWD=$PWD" ;; esac
    rm -rf /tmp/_cd_test_cp2
  expect
    stdout "pass_cdpath_multi"
    stderr ""
    exit_code 0
end test "CDPATH with multiple colon-separated entries"
```

#### Test: cd without CDPATH produces no output

When CDPATH is unset and the operand is not `-`, a successful `cd` must
produce no output on either stdout or stderr.

```
begin test "cd without CDPATH produces no output"
  script
    mkdir -p local_only
    unset CDPATH
    output=$(cd local_only 2>&1)
    if [ -z "$output" ]; then
      echo pass_no_output
    else
      echo "fail: output=$output"
    fi
  expect
    stdout "pass_no_output"
    stderr ""
    exit_code 0
end test "cd without CDPATH produces no output"
```

#### Test: CDPATH match prints new directory to stdout

When a non-empty CDPATH entry is used to resolve the target, `cd` must
print the absolute pathname of the new working directory to stdout.

```
begin test "CDPATH match prints new directory to stdout"
  script
    mkdir -p /tmp/_cd_test_cp3/cdout_dir
    CDPATH=/tmp/_cd_test_cp3
    export CDPATH
    output=$(cd cdout_dir)
    case "$output" in */cdout_dir) echo pass_cdpath_output ;; *) echo "fail: output=$output" ;; esac
    rm -rf /tmp/_cd_test_cp3
  expect
    stdout "pass_cdpath_output"
    stderr ""
    exit_code 0
end test "CDPATH match prints new directory to stdout"
```

#### Test: cd - returns to previous directory

The `-` operand makes `cd` change back to the directory stored in
OLDPWD, effectively toggling between the current and previous directory.

```
begin test "cd - returns to previous directory"
  script
    first=$PWD
    mkdir -p cd_dash_test
    cd cd_dash_test
    second=$PWD
    cd - >/dev/null
    if [ "$PWD" = "$first" ]; then
      echo pass_cd_dash
    else
      echo "fail: PWD=$PWD expected=$first"
    fi
  expect
    stdout "pass_cd_dash"
    stderr ""
    exit_code 0
end test "cd - returns to previous directory"
```

#### Test: cd - updates OLDPWD

Each successful `cd` must set OLDPWD to the working directory that was
in effect before the change, so that `cd -` can return to it.

```
begin test "cd - updates OLDPWD"
  script
    mkdir -p dash_old1 dash_old2
    cd dash_old1
    cd ../dash_old2
    cd - >/dev/null
    case "$OLDPWD" in */dash_old2) echo pass_oldpwd_updated ;; *) echo "fail: OLDPWD=$OLDPWD" ;; esac
  expect
    stdout "pass_oldpwd_updated"
    stderr ""
    exit_code 0
end test "cd - updates OLDPWD"
```

#### Test: cd with unset HOME fails

When HOME is unset and no operand is given, `cd` has no target directory
and must fail with a non-zero exit status.

```
begin test "cd with unset HOME fails"
  script
    unset HOME
    cd 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "cd with unset HOME fails"
```

#### Test: cd with absolute pathname operand

When the operand begins with `/`, `cd` bypasses CDPATH and uses the
absolute path directly as the target directory.

```
begin test "cd with absolute pathname operand"
  script
    mkdir -p /tmp/_cd_abs_test
    cd /tmp/_cd_abs_test
    case "$PWD" in */_cd_abs_test) echo pass_abs ;; *) echo "fail: PWD=$PWD" ;; esac
    rm -rf /tmp/_cd_abs_test
  expect
    stdout "pass_abs"
    stderr ""
    exit_code 0
end test "cd with absolute pathname operand"
```

#### Test: dot and dot-dot navigation

During logical pathname canonicalization, `.` components are removed and
`..` ascends one level, so a path like `./../../a/./b` resolves back to
the same directory as `a/b`.

```
begin test "dot and dot-dot navigation"
  script
    mkdir -p dot_a/dot_b
    cd dot_a/dot_b
    inner=$PWD
    cd ./../../dot_a/./dot_b
    if [ "$PWD" = "$inner" ]; then
      echo pass_dot_handling
    else
      echo "fail: PWD=$PWD expected=$inner"
    fi
  expect
    stdout "pass_dot_handling"
    stderr ""
    exit_code 0
end test "dot and dot-dot navigation"
```

#### Test: cd . stays in current directory

`cd .` should leave the working directory unchanged because `.` refers
to the current directory.

```
begin test "cd . stays in current directory"
  script
    before=$PWD
    cd .
    if [ "$PWD" = "$before" ]; then
      echo pass_cd_dot
    else
      echo "fail: PWD=$PWD expected=$before"
    fi
  expect
    stdout "pass_cd_dot"
    stderr ""
    exit_code 0
end test "cd . stays in current directory"
```

#### Test: cd .. goes to parent directory

`cd ..` ascends one directory level; PWD should reflect the parent of the
directory that was current before the command.

```
begin test "cd .. goes to parent directory"
  script
    mkdir -p dotdot_parent/dotdot_child
    cd dotdot_parent/dotdot_child
    cd ..
    case "$PWD" in */dotdot_parent) echo pass_cd_dotdot ;; *) echo "fail: PWD=$PWD" ;; esac
  expect
    stdout "pass_cd_dotdot"
    stderr ""
    exit_code 0
end test "cd .. goes to parent directory"
```

#### Test: cd to a regular file fails

`cd` must fail with a non-zero exit status when the operand names a
regular file rather than a directory.

```
begin test "cd to a regular file fails"
  script
    touch not_a_dir_file
    cd not_a_dir_file 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "cd to a regular file fails"
```

#### Test: cd to directory with no execute permission fails

The user must have execute (search) permission on a directory to `cd`
into it; without that permission, `cd` must fail with a non-zero exit
status.

```
begin test "cd to directory with no execute permission fails"
  script
    mkdir -p no_perm_dir
    chmod 000 no_perm_dir
    cd no_perm_dir 2>/dev/null
    rc=$?
    chmod 755 no_perm_dir
    exit $rc
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "cd to directory with no execute permission fails"
```

#### Test: PWD unchanged after failed cd

When `cd` fails, the working directory must remain unchanged and PWD
must still reflect the original directory.

```
begin test "PWD unchanged after failed cd"
  script
    before=$PWD
    cd /nonexistent_xyz_99999 2>/dev/null || true
    if [ "$PWD" = "$before" ]; then
      echo pass_pwd_unchanged
    else
      echo "fail: PWD=$PWD expected=$before"
    fi
  expect
    stdout "pass_pwd_unchanged"
    stderr ""
    exit_code 0
end test "PWD unchanged after failed cd"
```

#### Test: cd to empty string is an error

Per POSIX (Austin Group Defect 1047), if the directory operand is an
empty string, `cd` shall write a diagnostic message to standard error
and exit with non-zero status.

```
begin test "cd to empty string is an error"
  script
    cd ""
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "cd to empty string is an error"
```

#### Test: cd null curpath no-op

If the canonicalization algorithm reduces curpath to null (e.g., `cd /`
where the path is already root), the shell should handle it gracefully
and not error out.

```
begin test "cd null curpath no-op"
  script
    cd / 2>/dev/null
    echo "ok"
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "cd null curpath no-op"
```

#### Test: cd to deeply nested directory beyond PATH_MAX

When curpath exceeds PATH_MAX bytes, the shell may convert it to an
equivalent relative pathname. This test exercises the PATH_MAX boundary
to verify `cd` does not fail unexpectedly.

```
begin test "cd to deeply nested directory beyond PATH_MAX"
  script
    base=$HOME/deep
    mkdir -p "$base"
    d="$base"
    i=0
    while [ ${#d} -lt 4200 ]; do
      d="$d/sub"
      i=$((i + 1))
    done
    mkdir -p "$d" 2>/dev/null
    if [ -d "$d" ]; then
      cd "$d" 2>/dev/null && echo "cd_ok" || echo "cd_fail"
    else
      echo "cd_ok"
    fi
  expect
    stdout "cd_ok"
    stderr ""
    exit_code 0
end test "cd to deeply nested directory beyond PATH_MAX"
```

#### Test: cd -eP to valid directory succeeds with zero exit

With both `-e` and `-P` in effect, `cd` to a valid directory where PWD
can be determined must succeed with exit status zero.

```
begin test "cd -eP to valid directory succeeds with zero exit"
  script
    mkdir -p $HOME/edir
    cd -eP $HOME/edir
  expect
    stdout ""
    stderr ""
    exit_code 0
end test "cd -eP to valid directory succeeds with zero exit"
```

#### Test: cd accepts -- to end options

Per the POSIX utility syntax guidelines, `--` signals the end of
options; any argument after it is treated as the directory operand, not
as an option.

```
begin test "cd accepts -- to end options"
  script
    mkdir -p dashdir
    cd -- dashdir
    case "$PWD" in */dashdir) echo pass ;; *) echo fail ;; esac
  expect
    stdout "pass"
    stderr ""
    exit_code 0
end test "cd accepts -- to end options"
```
