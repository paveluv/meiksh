# Test Suite for 2.7 Redirection

This test suite covers **Section 2.7 Redirection** of the POSIX.1-2024 Shell
Command Language specification, including all seven subsections: Redirecting
Input, Redirecting Output, Appending Redirected Output, Here-Document,
Duplicating an Input File Descriptor, Duplicating an Output File Descriptor,
and Open File Descriptors for Reading and Writing.

## Table of contents

- [2.7 Redirection](#27-redirection)
- [2.7.1 Redirecting Input](#271-redirecting-input)
- [2.7.2 Redirecting Output](#272-redirecting-output)
- [2.7.3 Appending Redirected Output](#273-appending-redirected-output)
- [2.7.4 Here-Document](#274-here-document)
- [2.7.5 Duplicating an Input File Descriptor](#275-duplicating-an-input-file-descriptor)
- [2.7.6 Duplicating an Output File Descriptor](#276-duplicating-an-output-file-descriptor)
- [2.7.7 Open File Descriptors for Reading and Writing](#277-open-file-descriptors-for-reading-and-writing)

## 2.7 Redirection

Redirection is used to open and close files for the current shell execution environment (see [2.13 Shell Execution Environment](#213-shell-execution-environment)) or for any command. Redirection operators can be used with numbers representing file descriptors (see XBD [*3.141 File Descriptor*](docs/posix/md/basedefs/V1_chap03.md#3141-file-descriptor)) as described below.

The overall format used for redirection is:

```
[n]redir-op word
```

The number *n* is an optional one or more digit decimal number designating the file descriptor number; the application shall ensure it is delimited from any preceding text and immediately precedes the redirection operator *redir-op* (with no intervening `<blank>` characters allowed). If *n* is quoted, the number shall not be recognized as part of the redirection expression. For example:

```
echo \2>a
```

writes the character 2 into file **a**. If any part of *redir-op* is quoted, no redirection expression is recognized. For example:

```
echo 2\>a
```

writes the characters 2\>*a* to standard output. The optional number, redirection operator, and *word* shall not appear in the arguments provided to the command to be executed (if any).

The shell may support an additional format used for redirection:

```
{location}redir-op word
```

where *location* is non-empty and indicates a location where an integer value can be stored, such as the name of a shell variable. If this format is supported its behavior is implementation-defined.

The largest file descriptor number supported in shell redirections is implementation-defined; however, all implementations shall support at least 0 to 9, inclusive, for use by the application.

If the redirection operator is `"<<"` or `"<<-"`, the word that follows the redirection operator shall be subjected to quote removal; it is unspecified whether any of the other expansions occur. For the other redirection operators, the word that follows the redirection operator shall be subjected to tilde expansion, parameter expansion, command substitution, arithmetic expansion, and quote removal. Pathname expansion shall not be performed on the word by a non-interactive shell; an interactive shell may perform it, but if the expansion would result in more than one word it is unspecified whether the redirection proceeds without pathname expansion being performed or the redirection fails.

**Note:** A future version of this standard may require that the redirection fails in this case.

If more than one redirection operator is specified with a command, the order of evaluation is from beginning to end.

A failure to open or create a file shall cause a redirection to fail.

### Tests

#### Test: quoted fd number is not a redirection

When the file descriptor number `n` is quoted, it shall not be recognized as
part of the redirection expression. The quoted `"0"` is treated as a command
name, not a file descriptor.

```
begin test "quoted fd number is not a redirection"
  script
    echo content > tmp.txt
    "0"<tmp.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "quoted fd number is not a redirection"
```

#### Test: fd number must be delimited from preceding text

The file descriptor number is recognized only when it is delimited from any
preceding text. Here the `1` is part of the word `token1`, so `>` still uses
its default file descriptor.

```
begin test "fd number must be delimited from preceding text"
  script
    echo token1>tmp_delimited_fd.txt
    cat tmp_delimited_fd.txt
  expect
    stdout "token1"
    stderr ""
    exit_code 0
end test "fd number must be delimited from preceding text"
```

#### Test: blank between fd number and operator prevents fd designation

The file descriptor number must immediately precede the redirection operator,
with no intervening blanks. A separated `2` is an ordinary argument, not a
file descriptor designator.

```
begin test "blank between fd number and operator prevents fd designation"
  script
    echo literal 2 > tmp_blank_between_fd_and_operator.txt
    cat tmp_blank_between_fd_and_operator.txt
  expect
    stdout "literal 2"
    stderr ""
    exit_code 0
end test "blank between fd number and operator prevents fd designation"
```

#### Test: fd 3 works and does not appear in arguments

The optional number, redirection operator, and word shall not appear in the
arguments provided to the command. All implementations must support file
descriptors 0 through 9.

```
begin test "fd 3 works and does not appear in arguments"
  script
    echo "fd3 content" 3>tmp_fd3.txt
    cat tmp_fd3.txt
  expect
    stdout "fd3 content"
    stderr ""
    exit_code 0
end test "fd 3 works and does not appear in arguments"
```

#### Test: file descriptor 9 is supported for redirection

All implementations shall support file descriptors 0 to 9 inclusive for use
by the application. This test verifies the upper bound of that range.

```
begin test "file descriptor 9 is supported for redirection"
  script
    echo "fd9_data" > tmp_fd9_src.txt
    cat 9<tmp_fd9_src.txt <&9
  expect
    stdout "fd9_data"
    stderr ""
    exit_code 0
end test "file descriptor 9 is supported for redirection"
```

#### Test: failed redirection to read-only directory fails the command

A failure to open or create a file shall cause a redirection to fail. Writing
to a file inside a read-only directory triggers such a failure.

```
begin test "failed redirection to read-only directory fails the command"
  script
    mkdir -p tmp_ro_dir
    chmod -w tmp_ro_dir
    echo "fail" > tmp_ro_dir/file.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "failed redirection to read-only directory fails the command"
```

#### Test: failed input redirection fails the command

A failure to open a file shall cause a redirection to fail. Attempting to
redirect input from a file that does not exist causes the command to fail.

```
begin test "failed input redirection fails the command"
  script
    cat < does_not_exist_file.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "failed input redirection fails the command"
```

#### Test: pathname expansion does not occur on redirection word

Pathname expansion shall not be performed on the redirection word by a
non-interactive shell. Even if multiple matching files exist, a literal `*` in
the redirection target creates a file with that exact literal name.

```
begin test "pathname expansion does not occur on redirection word"
  script
    touch tmp_1_redir.txt tmp_2_redir.txt
    echo "literal" > tmp_*_redir.txt
    ls tmp_*_redir.txt
    rm tmp_1_redir.txt tmp_2_redir.txt "tmp_*_redir.txt"
  expect
    stdout "tmp_\*_redir\.txt\ntmp_1_redir\.txt\ntmp_2_redir\.txt"
    stderr ""
    exit_code 0
end test "pathname expansion does not occur on redirection word"
```

#### Test: redirection words are subject to parameter expansion

For redirection operators other than `<<` and `<<-`, the word shall be
subjected to parameter expansion. A variable holding a filename is expanded
in the redirection target.

```
begin test "redirection words are subject to parameter expansion"
  script
    file_var="tmp_var_redir.txt"
    echo "expanded" > "$file_var"
    cat tmp_var_redir.txt
  expect
    stdout "expanded"
    stderr ""
    exit_code 0
end test "redirection words are subject to parameter expansion"
```

#### Test: redirection words are subject to quote removal

For redirection operators other than `<<` and `<<-`, the word shall be
subjected to quote removal. Quotes used to preserve blanks in the target path
do not become part of the filename.

```
begin test "redirection words are subject to quote removal"
  script
    echo quoted > "tmp quoted redir.txt"
    cat "tmp quoted redir.txt"
  expect
    stdout "quoted"
    stderr ""
    exit_code 0
end test "redirection words are subject to quote removal"
```

#### Test: quoted redirection operator is not recognized

If any part of the redirection operator is quoted, no redirection expression
is recognized; the operator characters are ordinary arguments to the command.

```
begin test "quoted redirection operator is not recognized"
  script
    rm -f tmp_quoted_redirop.txt
    echo 2\>tmp_quoted_redirop.txt
    if test -f tmp_quoted_redirop.txt; then echo file_exists; else echo no_file; fi
  expect
    stdout "2>tmp_quoted_redirop.txt\nno_file"
    stderr ""
    exit_code 0
end test "quoted redirection operator is not recognized"
```

#### Test: redirection words are subject to tilde expansion

For redirection operators other than `<<` and `<<-`, the word shall be
subjected to tilde expansion (among others). A leading tilde in the target
path expands using `HOME`.

```
begin test "redirection words are subject to tilde expansion"
  script
    mkdir -p tmp_home_redir/sub
    HOME="$PWD/tmp_home_redir"
    export HOME
    echo tilde_home > ~/sub/out.txt
    cat tmp_home_redir/sub/out.txt
  expect
    stdout "tilde_home"
    stderr ""
    exit_code 0
end test "redirection words are subject to tilde expansion"
```

#### Test: redirection words are subject to command substitution

For redirection operators other than `<<` and `<<-`, the word shall be
subjected to command substitution. The filename can be produced by a command
substitution in the redirection target.

```
begin test "redirection words are subject to command substitution"
  script
    echo via_subst > $(echo tmp_cmdsubst_redir.txt)
    cat tmp_cmdsubst_redir.txt
  expect
    stdout "via_subst"
    stderr ""
    exit_code 0
end test "redirection words are subject to command substitution"
```

#### Test: redirection words are subject to arithmetic expansion

For redirection operators other than `<<` and `<<-`, the word shall be
subjected to arithmetic expansion. A digit sequence in the filename can come
from `$((...))`.

```
begin test "redirection words are subject to arithmetic expansion"
  script
    echo arith_word > tmp_$((12*2)).txt
    cat tmp_24.txt
  expect
    stdout "arith_word"
    stderr ""
    exit_code 0
end test "redirection words are subject to arithmetic expansion"
```

#### Test: multiple output redirections are evaluated left to right

If more than one redirection operator is specified with a command, the order
of evaluation is from beginning to end; later output redirections replace where
standard output is directed.

```
begin test "multiple output redirections are evaluated left to right"
  script
    echo ordered > tmp_multi_first.txt > tmp_multi_second.txt
    if test -s tmp_multi_first.txt; then echo first_nonempty; else echo first_empty; fi
    if test -s tmp_multi_second.txt; then echo second_nonempty; else echo second_empty; fi
  expect
    stdout "first_empty\nsecond_nonempty"
    stderr ""
    exit_code 0
end test "multiple output redirections are evaluated left to right"
```

#### Test: stderr duplication respects left-to-right redirection order

Redirections are evaluated from beginning to end. Duplicating stderr from the
current stdout before redirecting stdout to a file leaves stderr on the
original destination, while stdout goes to the file. In the harness, that
original destination is the captured stdout stream.

```
begin test "stderr duplication respects left-to-right redirection order"
  script
    (printf out; printf err >&2) 2>&1 > tmp_stderr_order.txt
    cat tmp_stderr_order.txt
  expect
    stdout "errout"
    stderr ""
    exit_code 0
end test "stderr duplication respects left-to-right redirection order"
```

#### Test: redirection operators do not count as command arguments

The optional number, redirection operator, and word shall not appear in the
arguments provided to the command. A function sees only its real operands in
`$#`.

```
begin test "redirection operators do not count as command arguments"
  script
    f() { printf '%s' "$#"; }
    f onlyarg > tmp_argcount.txt
    cat tmp_argcount.txt
  expect
    stdout "1"
    stderr ""
    exit_code 0
end test "redirection operators do not count as command arguments"
```

## 2.7.1 Redirecting Input

Input redirection shall cause the file whose name results from the expansion of *word* to be opened for reading on the designated file descriptor, or standard input if the file descriptor is not specified.

The general format for redirecting input is:

```
[n]<word
```

where the optional *n* represents the file descriptor number. If the number is omitted, the redirection shall refer to standard input (file descriptor 0).

### Tests

#### Test: input redirection with <

Input redirection opens the named file for reading on standard input when no
file descriptor number is specified.

```
begin test "input redirection with <"
  script
    echo world > tmp.txt
    cat < tmp.txt
  expect
    stdout "world"
    stderr ""
    exit_code 0
end test "input redirection with <"
```

#### Test: explicit stdin redirection with 0<

Explicitly specifying file descriptor 0 before `<` is equivalent to omitting
the number — both redirect standard input.

```
begin test "explicit stdin redirection with 0<"
  script
    echo world > tmp.txt
    cat 0< tmp.txt
  expect
    stdout "world"
    stderr ""
    exit_code 0
end test "explicit stdin redirection with 0<"
```

#### Test: input redirection opens file on designated file descriptor

Input redirection `[n]<word` opens the file on file descriptor *n* when *n* is
specified. Duplicating that descriptor onto standard input allows `cat` to read
the file.

```
begin test "input redirection opens file on designated file descriptor"
  script
    echo designated > tmp_n_input.txt
    cat 5<tmp_n_input.txt <&5
  expect
    stdout "designated"
    stderr ""
    exit_code 0
end test "input redirection opens file on designated file descriptor"
```

## 2.7.2 Redirecting Output

The two general formats for redirecting output are:

```
[n]>word
[n]>|word
```

where the optional *n* represents the file descriptor number. If the number is omitted, the redirection shall refer to standard output (file descriptor 1).

Output redirection using the `'>'` format shall fail if the *noclobber* option is set (see the description of [*set*](#set) **-C**) and the file named by the expansion of *word* exists and is either a regular file or a symbolic link that resolves to a regular file; it may also fail if the file is a symbolic link that does not resolve to an existing file. The check for existence, file creation, and open operations shall be performed atomically as is done by the [*open*()](docs/posix/md/functions/open.md) function as defined in System Interfaces volume of POSIX.1-2024 when the O_CREAT and O_EXCL flags are set, except that if the file exists and is a symbolic link, the open operation need not fail with [EEXIST] unless the symbolic link resolves to an existing regular file. Performing these operations atomically ensures that the creation of lock files and unique (often temporary) files is reliable, with important caveats detailed in [*C.2.7.2 Redirecting Output*](docs/posix/md/xrat/V4_xcu_chap01.md#c272-redirecting-output). The check for the type of the file need not be performed atomically with the check for existence, file creation, and open operations. If not, there is a potential race condition that may result in a misleading shell diagnostic message when redirection fails. See XRAT [*C.2.7.2 Redirecting Output*](docs/posix/md/xrat/V4_xcu_chap01.md#c272-redirecting-output) for more details.

In all other cases (*noclobber* not set, redirection using `'>'` does not fail for the reasons stated above, or redirection using the `">|"` format), output redirection shall cause the file whose name results from the expansion of *word* to be opened for output on the designated file descriptor, or standard output if none is specified. If the file does not exist, it shall be created as an empty file; otherwise, it shall be opened as if the [*open*()](docs/posix/md/functions/open.md) function was called with the O_TRUNC flag set.

### Tests

#### Test: basic output redirection with >

Output redirection with `>` creates a file and writes stdout to it. When the
fd number is omitted, it defaults to standard output (fd 1).

```
begin test "basic output redirection with >"
  script
    echo hello > tmp.txt
    cat tmp.txt
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "basic output redirection with >"
```

#### Test: explicit stdout redirection with 1>

Explicitly specifying file descriptor 1 before `>` redirects standard output,
equivalent to omitting the number.

```
begin test "explicit stdout redirection with 1>"
  script
    echo foo 1> tmp.txt
    cat tmp.txt
  expect
    stdout "foo"
    stderr ""
    exit_code 0
end test "explicit stdout redirection with 1>"
```

#### Test: output redirection on designated file descriptor

Output redirection with an explicit file descriptor number opens the file on
that descriptor. Here `2>` redirects standard error to a file.

```
begin test "output redirection on designated file descriptor"
  script
    (echo "stderr_data" >&2) 2>tmp_fd2_redir.txt
    cat tmp_fd2_redir.txt
  expect
    stdout "stderr_data"
    stderr ""
    exit_code 0
end test "output redirection on designated file descriptor"
```

#### Test: output redirection creates nonexistent file

If the target file does not exist, output redirection shall create it as an
empty file. The `:` command writes nothing, so the created file is empty.

```
begin test "output redirection creates nonexistent file"
  script
    rm -f tmp_create_redir.txt
    : > tmp_create_redir.txt
    test -f tmp_create_redir.txt && echo "created"
    test -s tmp_create_redir.txt || echo "empty"
  expect
    stdout "created\nempty"
    stderr ""
    exit_code 0
end test "output redirection creates nonexistent file"
```

#### Test: output redirection with > truncates existing file

When `>` succeeds and the target already exists, the file shall be opened as
if with `O_TRUNC`, so previous contents are discarded before writing.

```
begin test "output redirection with > truncates existing file"
  script
    printf 'old data\n' > tmp_trunc.txt
    echo new > tmp_trunc.txt
    cat tmp_trunc.txt
  expect
    stdout "new"
    stderr ""
    exit_code 0
end test "output redirection with > truncates existing file"
```

#### Test: noclobber prevents overwriting existing file

When the `noclobber` option is set (`set -C`), output redirection with `>`
shall fail if the target file already exists as a regular file.

```
begin test "noclobber prevents overwriting existing file"
  script
    set -C
    echo a > tmp.txt
    echo b > tmp.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "noclobber prevents overwriting existing file"
```

#### Test: noclobber prevents overwriting symlink to regular file

When `noclobber` is set, output redirection with `>` shall also fail if the
target is a symbolic link that resolves to an existing regular file.

```
begin test "noclobber prevents overwriting symlink to regular file"
  script
    printf old > tmp_target.txt
    ln -s tmp_target.txt tmp_link.txt
    set -C
    echo new > tmp_link.txt
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "noclobber prevents overwriting symlink to regular file"
```

#### Test: noclobber allows creating new files

When `noclobber` is set, output redirection with `>` shall only fail if the
target already exists as a regular file. Creating a new file is permitted.

```
begin test "noclobber allows creating new files"
  script
    set -C
    rm -f tmp_new_clobber.txt
    echo "new" > tmp_new_clobber.txt
    cat tmp_new_clobber.txt
  expect
    stdout "new"
    stderr ""
    exit_code 0
end test "noclobber allows creating new files"
```

#### Test: greater-pipe output redirection overwrites despite noclobber

When `noclobber` is set, redirection using the `">|"` format shall still open
the existing file for output (truncating it), unlike plain `>`.

```
begin test "greater-pipe output redirection overwrites despite noclobber"
  script
    set -C
    echo first > tmp_force_clobber.txt
    echo second >| tmp_force_clobber.txt
    cat tmp_force_clobber.txt
  expect
    stdout "second"
    stderr ""
    exit_code 0
end test "greater-pipe output redirection overwrites despite noclobber"
```

#### Test: greater-pipe output redirection creates non-existent file

Even if `noclobber` is set or unset, the `>|` format creates the target file
if it does not already exist.

```
begin test "greater-pipe output redirection creates non-existent file"
  script
    set -C
    rm -f tmp_greater_pipe_create.txt
    echo "created" >| tmp_greater_pipe_create.txt
    cat tmp_greater_pipe_create.txt
  expect
    stdout "created"
    stderr ""
    exit_code 0
end test "greater-pipe output redirection creates non-existent file"
```

## 2.7.3 Appending Redirected Output

Appended output redirection shall cause the file whose name results from the expansion of word to be opened for output on the designated file descriptor. The file shall be opened as if the [*open*()](docs/posix/md/functions/open.md) function as defined in the System Interfaces volume of POSIX.1-2024 was called with the O_APPEND flag set. If the file does not exist, it shall be created.

The general format for appending redirected output is as follows:

```
[n]>>word
```

where the optional *n* represents the file descriptor number. If the number is omitted, the redirection refers to standard output (file descriptor 1).

### Tests

#### Test: append redirection with >>

The `>>` operator appends to the file rather than truncating it. If the file
does not exist, it is created.

```
begin test "append redirection with >>"
  script
    echo a >> tmp_append.txt
    echo b >> tmp_append.txt
    cat tmp_append.txt
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "append redirection with >>"
```

#### Test: append redirection with explicit file descriptor

The optional *n* in `[n]>>word` opens the file for append on file descriptor *n*.
Writing with `>&n` appends through that descriptor the same way `>>` does for
standard output.

```
begin test "append redirection with explicit file descriptor"
  script
    echo first >> tmp_append3.txt
    exec 3>>tmp_append3.txt
    echo second >&3
    exec 3>&-
    cat tmp_append3.txt
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "append redirection with explicit file descriptor"
```

## 2.7.4 Here-Document

The redirection operators `"<<"` and `"<<-"` both allow redirection of subsequent lines read by the shell to the input of a command. The redirected lines are known as a "here-document".

The here-document shall be treated as a single word that begins after the next **NEWLINE** token and continues until there is a line containing only the delimiter and a `<newline>`, with no `<blank>` characters in between. Then the next here-document starts, if there is one. For the purposes of locating this terminating line, the end of a *command_string* operand (see [*sh*](docs/posix/md/utilities/sh.md)) shall be treated as a `<newline>` character, and the end of the *commands* string in `$(commands)` and `` `commands` `` may be treated as a `<newline>`. If the end of input is reached without finding the terminating line, the shell should, but need not, treat this as a redirection error. The format is as follows:

```
[n]<<word
    here-document
delimiter
```

where the optional *n* represents the file descriptor number. If the number is omitted, the here-document refers to standard input (file descriptor 0). It is unspecified whether the file descriptor is opened as a regular file or some other type of file. Portable applications cannot rely on the file descriptor being seekable (see XSH [*lseek*()](docs/posix/md/functions/lseek.md)).

If any part of *word* is quoted, not counting double-quotes outside a command substitution if the here-document is inside one, the delimiter shall be formed by performing quote removal on *word*, and the here-document lines shall not be expanded. Otherwise:

- The delimiter shall be the *word* itself.
- The removal of `<backslash>``<newline>` for line continuation (see [2.2.1 Escape Character (Backslash)](#221-escape-character-backslash)) shall be performed during the search for the trailing delimiter. (As a consequence, the trailing delimiter is not recognized immediately after a `<newline>` that was removed by line continuation.) It is unspecified whether the line containing the trailing delimiter is itself subject to this line continuation.
- All lines of the here-document shall be expanded, when the redirection operator is evaluated but after the trailing delimiter for the here-document has been located, for parameter expansion, command substitution, and arithmetic expansion. If the redirection operator is never evaluated (because the command it is part of is not executed), the here-document shall be read without performing any expansions.
- Any `<backslash>` characters in the input shall behave as the `<backslash>` inside double-quotes (see [2.2.3 Double-Quotes](#223-double-quotes)). However, the double-quote character (`'"'`) shall not be treated specially within a here-document, except when the double-quote appears within `"$()"`, ```"``"```, or `"${}"`.

If the redirection operator is `"<<-"`, all leading `<tab>` characters shall be stripped from input lines after `<backslash>``<newline>` line continuation (when it applies) has been performed, and from the line containing the trailing delimiter. Stripping of leading `<tab>` characters shall occur as the here-document is read from the shell input (and consequently does not affect any `<tab>` characters that result from expansions).

If more than one `"<<"` or `"<<-"` operator is specified on a line, the here-document associated with the first operator shall be supplied first by the application and shall be read first by the shell.

When a here-document is read from a terminal device and the shell is interactive, it shall write the contents of the variable *PS2*, processed as described in [2.5.3 Shell Variables](#253-shell-variables), to standard error before reading each line of input until the delimiter has been recognized.

---

*The following sections are informative.*

##### Examples

An example of a here-document follows:

```
cat <<eof1; cat <<eof2
Hi,
eof1
Helene.
eof2
```

*End of informative text.*

---

### Tests

#### Test: here-document basic usage

A basic here-document with an unquoted delimiter. The delimiter on its own line
terminates the here-document body.

```
begin test "here-document basic usage"
  script
    cat <<EOF
    line1
    line2
    EOF
  expect
    stdout "line1\nline2"
    stderr ""
    exit_code 0
end test "here-document basic usage"
```

#### Test: here-document with variable expansion

When the delimiter word is not quoted, all lines of the here-document are
expanded for parameter expansion, command substitution, and arithmetic
expansion.

```
begin test "here-document with variable expansion"
  script
    var=hello
    cat <<EOF
    $var
    EOF
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "here-document with variable expansion"
```

#### Test: here-document lines expanded after delimiter is located

All lines of the here-document shall be expanded *after* the trailing delimiter
has been located. A word containing an expansion that evaluates to the delimiter
does not act as the delimiter.

```
begin test "here-document lines expanded after delimiter is located"
  script
    EOF="EOF"
    cat <<EOF
    $EOF
    EOF
  expect
    stdout "EOF"
    stderr ""
    exit_code 0
end test "here-document lines expanded after delimiter is located"
```

#### Test: here-document performs command and arithmetic expansion

When the delimiter word is not quoted, here-document lines shall also be
expanded for command substitution and arithmetic expansion.

```
begin test "here-document performs command and arithmetic expansion"
  script
    cat <<EOF
    $(printf cmd)
    $((2 + 3))
    EOF
  expect
    stdout "cmd\n5"
    stderr ""
    exit_code 0
end test "here-document performs command and arithmetic expansion"
```

#### Test: here-document with quoted delimiter suppresses expansion

When any part of the delimiter word is quoted, here-document lines are not
expanded. Variables like `$var` remain literal.

```
begin test "here-document with quoted delimiter suppresses expansion"
  script
    var=hello
    cat <<'EOF'
    $var
    EOF
  expect
    stdout "\$var"
    stderr ""
    exit_code 0
end test "here-document with quoted delimiter suppresses expansion"
```

#### Test: quoted here-document delimiter suppresses command and arithmetic expansion

When any part of the delimiter word is quoted, here-document lines shall not
be expanded at all, including command substitution and arithmetic expansion.

```
begin test "quoted here-document delimiter suppresses command and arithmetic expansion"
  script
    cat <<'EOF'
    $(printf no)
    $((1 + 2))
    EOF
  expect
    stdout "\$\(printf no\)\n\$\(\(1 \+ 2\)\)"
    stderr ""
    exit_code 0
end test "quoted here-document delimiter suppresses command and arithmetic expansion"
```

#### Test: partially quoted here-document delimiter suppresses expansion

When any part of the delimiter word is quoted, quote removal forms the
delimiter and here-document lines shall not be expanded. A partially quoted
word like `E"O"F` becomes the delimiter `EOF` after quote removal.

```
begin test "partially quoted here-document delimiter suppresses expansion"
  script
    var=hello
    cat <<E"O"F
    $var
    EOF
  expect
    stdout "\$var"
    stderr ""
    exit_code 0
end test "partially quoted here-document delimiter suppresses expansion"
```

#### Test: here-document double-quote is special inside dollar-paren

The double-quote character shall not be treated specially within a
here-document, except when it appears within `$()`. Inside a command
substitution, double-quotes retain their quoting effect.

```
begin test "here-document double-quote is special inside dollar-paren"
  script
    cat <<EOF
    $(echo "hello world")
    EOF
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "here-document double-quote is special inside dollar-paren"
```

#### Test: here-document expands variables and handles backslash continuation

In an unquoted here-document, variables are expanded, backslash-newline acts as
line continuation, and double-quote characters are not treated specially.

```
begin test "here-document expands variables and handles backslash continuation"
  script
    var="expanded"; cat <<EOF
    this is $var
    backslash \
    continues
    double "quotes"
    EOF
  expect
    stdout "this is expanded\nbackslash continues\ndouble ""quotes"""
    stderr ""
    exit_code 0
end test "here-document expands variables and handles backslash continuation"
```

#### Test: here-document backslashes behave as in double quotes

In an unquoted here-document, backslashes shall behave as they do inside
double-quotes: they can preserve a literal `$` and a literal backslash.

```
begin test "here-document backslashes behave as in double quotes"
  script
    value=expanded
    cat <<EOF
    \$value
    \\
    EOF
  expect
    stdout "\$value\n\\"
    stderr ""
    exit_code 0
end test "here-document backslashes behave as in double quotes"
```

#### Test: here-document backslash does not escape double quote

Because double-quotes are not treated specially within an unquoted here-document
(unless inside a substitution), a backslash preceding a double-quote does not
escape it; both characters are preserved literally.

```
begin test "here-document backslash does not escape double quote"
  script
    cat <<EOF
    \"
    EOF
  expect
    stdout "\\"""
    stderr ""
    exit_code 0
end test "here-document backslash does not escape double quote"
```

#### Test: here-document not expanded when command is not executed

If the redirection operator is never evaluated (because the command is not
executed), the here-document shall be read without performing any expansions.

```
begin test "here-document not expanded when command is not executed"
  script
    if false; then cat <<EOF
    this contains an invalid $var_that_fails_if_expanded
    EOF
    fi
    echo "survived"
  expect
    stdout "survived"
    stderr ""
    exit_code 0
end test "here-document not expanded when command is not executed"
```

#### Test: multiple here-documents on one line are read in order

When multiple `<<` operators appear on one line, the here-documents are
supplied and read in order from left to right.

```
begin test "multiple here-documents on one line are read in order"
  script
    cat <<EOF1; cat <<EOF2
    first
    EOF1
    second
    EOF2
  expect
    stdout "first\nsecond"
    stderr ""
    exit_code 0
end test "multiple here-documents on one line are read in order"
```

#### Test: here-document can target an explicit file descriptor

The optional *n* in `[n]<<word` designates the file descriptor that receives
the here-document input. Duplicating that descriptor onto standard input lets
`cat` read the here-document contents.

```
begin test "here-document can target an explicit file descriptor"
  script
    cat 5<<EOF <&5
    heredoc-fd
    EOF
  expect
    stdout "heredoc-fd"
    stderr ""
    exit_code 0
end test "here-document can target an explicit file descriptor"
```

#### Test: here-document delimiter search applies backslash-newline continuation

When searching for the trailing delimiter of an unquoted here-document,
`<backslash><newline>` line continuation shall be performed. A delimiter split
across those two physical lines is therefore recognized.

```
begin test "here-document delimiter search applies backslash-newline continuation"
  script
    cat <<EOF
    before
    EO\
    F
    echo after
  expect
    stdout "before\nafter"
    stderr ""
    exit_code 0
end test "here-document delimiter search applies backslash-newline continuation"
```

#### Test: interactive here-document prompt goes to stderr stream

When a here-document is read from a terminal device and the shell is
interactive, `PS2` shall be written to standard error before each input line.

```
begin test "interactive here-document prompt goes to stderr stream"
  script
    $SHELL -i > hd_stdout.txt 2> hd_stderr.txt <<'EOF'
    PS2='heredoc> '
    cat <<EOT
    first
    EOT
    exit
    EOF
    grep -q 'heredoc> ' hd_stderr.txt && echo stderr_ok || echo stderr_missing
    grep -q 'heredoc> ' hd_stdout.txt && echo stdout_leak || echo stdout_clean
  expect
    stdout "stderr_ok\nstdout_clean"
    stderr ""
    exit_code 0
end test "interactive here-document prompt goes to stderr stream"
```

#### Test: here-document with <<- strips leading tabs

The `<<-` operator strips all leading tab characters from input lines and from
the trailing delimiter line. This allows here-documents to be indented in
scripts without affecting the content.

```
begin test "here-document with <<- strips leading tabs"
  script
    cat <<-EOF
    	line 1
    		line 2
    EOF
  expect
    stdout "line 1\nline 2"
    stderr ""
    exit_code 0
end test "here-document with <<- strips leading tabs"
```

#### Test: here-document with <<- strips tabs from delimiter line

With `<<-`, leading tab characters are stripped from the line containing the
trailing delimiter as the here-document is read. A tab-indented delimiter line
still terminates the here-document.

```
begin test "here-document with <<- strips tabs from delimiter line"
  script
    printf 'cat <<-EOF\n\tline\n\tEOF\necho after\n' > tmp_tab_delim.sh
    $SHELL tmp_tab_delim.sh
  expect
    stdout "line\nafter"
    stderr ""
    exit_code 0
end test "here-document with <<- strips tabs from delimiter line"
```

#### Test: here-document with <<- does not strip tabs produced by expansion

Tab stripping for `<<-` occurs as the here-document is read from shell input,
so a leading tab that comes from expansion is not removed.

```
begin test "here-document with <<- does not strip tabs produced by expansion"
  script
    tab=$(printf '\t')
    cat <<-EOF | od -An -tx1 | tr -d ' \n'
    ${tab}x
    EOF
  expect
    stdout "09780a"
    stderr ""
    exit_code 0
end test "here-document with <<- does not strip tabs produced by expansion"
```

#### Test: here-document terminator line allows no blanks before delimiter

The here-document ends at a line containing only the delimiter and a newline,
with no `<blank>` characters before the delimiter. A line that includes
leading blanks before the delimiter text is part of the document body.

```
begin test "here-document terminator line allows no blanks before delimiter"
  script
    cat <<EOF
    body
     EOF
    EOF
  expect
    stdout "body\n EOF"
    stderr ""
    exit_code 0
end test "here-document terminator line allows no blanks before delimiter"
```

#### Test: here-document trailing delimiter not recognized after line continuation

The search for the trailing delimiter occurs while processing `<backslash><newline>`
line continuation. As a consequence, if a `<newline>` immediately precedes the
delimiter but is removed by line continuation, the delimiter is not recognized.

```
begin test "here-document trailing delimiter not recognized after line continuation"
  script
    cat <<EOF
    body\
    EOF
    real body
    EOF
  expect
    stdout "bodyEOF\nreal body"
    stderr ""
    exit_code 0
end test "here-document trailing delimiter not recognized after line continuation"
```

#### Test: outer double-quotes around command substitution do not suppress here-document expansion

If any part of the delimiter word is quoted, here-document lines shall not
be expanded — but double-quotes outside a command substitution are not counted
when the here-document is inside one. Outer `"` around `$()` must not suppress
expansion of the here-document body.

```
begin test "outer double-quotes around command substitution do not suppress here-document expansion"
  script
    _HD_OUTER=expanded
    x="$(cat <<EOF
    $_HD_OUTER
    EOF
    )"
    echo "$x"
  expect
    stdout "expanded"
    stderr ""
    exit_code 0
end test "outer double-quotes around command substitution do not suppress here-document expansion"
```

#### Test: here-document double-quote is special inside backticks

The double-quote character shall not be treated specially within a
here-document, except when it appears within backtick command substitution
(among others). Inside backticks, double-quotes retain their quoting effect.

```
begin test "here-document double-quote is special inside backticks"
  script
    cat <<EOF
    `echo "hello world"`
    EOF
  expect
    stdout "hello world"
    stderr ""
    exit_code 0
end test "here-document double-quote is special inside backticks"
```

#### Test: here-document terminated by end of command string

For the purposes of locating the terminating line, the end of a
`command_string` operand (see sh) shall be treated as a `<newline>` character.
A here-document whose delimiter is the very last token in `sh -c` (with no
trailing newline) is properly terminated.

```
begin test "here-document terminated by end of command string"
  script
    $SHELL -c "$(printf 'cat <<EOF\nhello\nEOF')"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "here-document terminated by end of command string"
```

#### Test: here-document double-quote is special inside braces

The double-quote character shall not be treated specially within a
here-document, except when it appears within `${}` (among others). Here the
`"` inside `${:+}` acts as a quoting character, so the output contains no
literal quote characters.

```
begin test "here-document double-quote is special inside braces"
  script
    _HD_BR=yes
    cat <<EOF
    ${_HD_BR:+"has value"}
    EOF
  expect
    stdout "has value"
    stderr ""
    exit_code 0
end test "here-document double-quote is special inside braces"
```

#### Test: here-document <<- strips tabs after backslash-newline continuation

With `<<-`, leading tab stripping occurs after backslash-newline line
continuation has been performed. When two tab-indented lines are joined by
continuation, the second line's tab becomes embedded and survives stripping.

```
begin test "here-document <<- strips tabs after backslash-newline continuation"
  script
    printf 'cat <<-DELIM\n\tcont\\\n\tinued\nDELIM\n' > tmp_tab_cont.sh
    $SHELL tmp_tab_cont.sh
  expect
    stdout "cont\tinued"
    stderr ""
    exit_code 0
end test "here-document <<- strips tabs after backslash-newline continuation"
```

## 2.7.5 Duplicating an Input File Descriptor

The redirection operator:

```
[n]<&word
```

shall duplicate one input file descriptor from another, or shall close one. If *word* evaluates to one or more digits, the file descriptor denoted by *n*, or standard input if *n* is not specified, shall be made to be a copy of the file descriptor denoted by *word*; if the digits in *word* do not represent an already open file descriptor, a redirection error shall result (see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)); if the file descriptor denoted by *word* represents an open file descriptor that is not open for input, a redirection error may result. If *word* evaluates to `'-'`, file descriptor *n*, or standard input if *n* is not specified, shall be closed. Attempts to close a file descriptor that is not open shall not constitute an error. If *word* evaluates to something else, the behavior is unspecified.

### Tests

#### Test: duplicate input fd from file

The `<&` operator duplicates an input file descriptor. A file opened on fd 5
can be read via `cat <&5`.

```
begin test "duplicate input fd from file"
  script
    echo "input dup" > tmp_in.txt
    exec 5<tmp_in.txt
    cat <&5
    exec 5<&-
  expect
    stdout "input dup"
    stderr ""
    exit_code 0
end test "duplicate input fd from file"
```

#### Test: closing an unopened input fd is not an error

Attempts to close a file descriptor that is not open shall not constitute an
error. Closing an unused fd 8 with `8<&-` succeeds silently.

```
begin test "closing an unopened input fd is not an error"
  script
    echo "ok" 8<&-
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "closing an unopened input fd is not an error"
```

#### Test: duplicate input fd evaluates word to dash for closing

If word evaluates to '-', file descriptor `n` (or standard input) shall be
closed. This applies after `word` is expanded.

```
begin test "duplicate input fd evaluates word to dash for closing"
  script
    echo "data" > tmp_var_dash_in.txt
    exec 5<tmp_var_dash_in.txt
    var="-"
    exec 5<&$var
    (cat <&5) 2>/dev/null || echo "fd5_closed"
  expect
    stdout "fd5_closed"
    stderr ""
    exit_code 0
end test "duplicate input fd evaluates word to dash for closing"
```

#### Test: duplicate input from non-open file descriptor is a redirection error

If *word* in `[n]<&word` evaluates to digits that do not represent an already
open file descriptor, a redirection error shall result.

```
begin test "duplicate input from non-open file descriptor is a redirection error"
  script
    true 3<&99
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "duplicate input from non-open file descriptor is a redirection error"
```

#### Test: omitted n with input fd close closes standard input

If *word* evaluates to `'-'` and *n* is omitted in `[n]<&word`, standard input
shall be closed for the command.

```
begin test "omitted n with input fd close closes standard input"
  script
    cat <&-
  expect
    stdout ""
    stderr "(.|\n)+"
    exit_code !=0
end test "omitted n with input fd close closes standard input"
```

#### Test: duplicate input fd to explicit fd number

When *n* is specified in `[n]<&word`, file descriptor *n* (not standard input)
shall be made to be a copy of the file descriptor denoted by *word*.

```
begin test "duplicate input fd to explicit fd number"
  script
    echo "dup_n_data" > tmp_dup_in_n.txt
    exec 5<tmp_dup_in_n.txt
    exec 3<&5
    exec 5<&-
    cat <&3
    exec 3<&-
  expect
    stdout "dup_n_data"
    stderr ""
    exit_code 0
end test "duplicate input fd to explicit fd number"
```

#### Test: duplicate input fd uses expanded word for target fd

The `word` in `[n]<&word` is expanded. A variable that expands to a file
descriptor number acts as the target descriptor.

```
begin test "duplicate input fd uses expanded word for target fd"
  script
    echo "data" > tmp_var_fd.txt
    exec 5<tmp_var_fd.txt
    fd=5
    cat <&$fd
    exec 5<&-
  expect
    stdout "data"
    stderr ""
    exit_code 0
end test "duplicate input fd uses expanded word for target fd"
```

#### Test: closing specific input fd with n<&-

When *word* evaluates to `'-'` in `[n]<&word`, file descriptor *n* shall be
closed. After closing fd 5, attempting to read from it causes a redirection
error.

```
begin test "closing specific input fd with n<&-"
  script
    echo data > tmp_close_in5.txt
    exec 5<tmp_close_in5.txt
    cat <&5
    exec 5<&-
    (cat <&5) 2>/dev/null || echo "fd5_closed"
  expect
    stdout "data\nfd5_closed"
    stderr ""
    exit_code 0
end test "closing specific input fd with n<&-"
```

## 2.7.6 Duplicating an Output File Descriptor

The redirection operator:

```
[n]>&word
```

shall duplicate one output file descriptor from another, or shall close one. If *word* evaluates to one or more digits, the file descriptor denoted by *n*, or standard output if *n* is not specified, shall be made to be a copy of the file descriptor denoted by *word*; if the digits in *word* do not represent an already open file descriptor, a redirection error shall result (see [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors)); if the file descriptor denoted by *word* represents an open file descriptor that is not open for output, a redirection error may result. If *word* evaluates to `'-'`, file descriptor *n*, or standard output if *n* is not specified, is closed. Attempts to close a file descriptor that is not open shall not constitute an error. If *word* evaluates to something else, the behavior is unspecified.

### Tests

#### Test: duplicate output fd to file

The `>&` operator duplicates an output file descriptor. Opening fd 4 for
writing and then writing to `>&4` sends output to that file.

```
begin test "duplicate output fd to file"
  script
    exec 4>tmp_dup.txt
    echo "dup test" >&4
    exec 4>&-
    cat tmp_dup.txt
  expect
    stdout "dup test"
    stderr ""
    exit_code 0
end test "duplicate output fd to file"
```

#### Test: closing an unopened output fd is not an error

Attempts to close a file descriptor that is not open shall not constitute an
error. Closing an unused fd 9 with `9>&-` succeeds silently.

```
begin test "closing an unopened output fd is not an error"
  script
    echo "ok" 9>&-
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "closing an unopened output fd is not an error"
```

#### Test: duplicate output fd evaluates word to dash for closing

If word evaluates to '-', file descriptor `n` (or standard output) shall be
closed. This applies after `word` is expanded.

```
begin test "duplicate output fd evaluates word to dash for closing"
  script
    exec 5>tmp_var_dash_out.txt
    var="-"
    exec 5>&$var
    (echo "fail" >&5) 2>/dev/null || echo "fd5_closed"
  expect
    stdout "fd5_closed"
    stderr ""
    exit_code 0
end test "duplicate output fd evaluates word to dash for closing"
```

#### Test: duplicate output from non-open file descriptor is a redirection error

If *word* in `[n]>&word` evaluates to digits that do not represent an already
open file descriptor, a redirection error shall result.

```
begin test "duplicate output from non-open file descriptor is a redirection error"
  script
    true 3>&99
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "duplicate output from non-open file descriptor is a redirection error"
```

#### Test: omitted n with output fd close closes standard output

If *word* evaluates to `'-'` and *n* is omitted in `[n]>&word`, standard
output is closed for the command.

```
begin test "omitted n with output fd close closes standard output"
  script
    echo hi >&-
  expect
    stdout ""
    stderr ".+"
    exit_code !=0
end test "omitted n with output fd close closes standard output"
```

#### Test: duplicate output fd to explicit fd number

When *n* is specified in `[n]>&word`, file descriptor *n* (not standard
output) shall be made to be a copy of the file descriptor denoted by *word*.

```
begin test "duplicate output fd to explicit fd number"
  script
    exec 5>tmp_dup_out_n.txt
    exec 3>&5
    exec 5>&-
    echo "dup_out_n_data" >&3
    exec 3>&-
    cat tmp_dup_out_n.txt
  expect
    stdout "dup_out_n_data"
    stderr ""
    exit_code 0
end test "duplicate output fd to explicit fd number"
```

#### Test: duplicate output fd uses expanded word for target fd

The `word` in `[n]>&word` is expanded. A variable that expands to a file
descriptor number acts as the target descriptor.

```
begin test "duplicate output fd uses expanded word for target fd"
  script
    exec 5>tmp_var_out_fd.txt
    fd=5
    echo "data" >&$fd
    exec 5>&-
    cat tmp_var_out_fd.txt
  expect
    stdout "data"
    stderr ""
    exit_code 0
end test "duplicate output fd uses expanded word for target fd"
```

#### Test: closing specific output fd with n>&-

When *word* evaluates to `'-'` in `[n]>&word`, file descriptor *n* shall be
closed. After closing fd 4, attempting to write to it causes a redirection
error, but data written before the close is preserved.

```
begin test "closing specific output fd with n>&-"
  script
    exec 4>tmp_close_out4.txt
    echo "written" >&4
    exec 4>&-
    (echo "fail" >&4) 2>/dev/null || echo "fd4_closed"
    cat tmp_close_out4.txt
  expect
    stdout "fd4_closed\nwritten"
    stderr ""
    exit_code 0
end test "closing specific output fd with n>&-"
```

## 2.7.7 Open File Descriptors for Reading and Writing

The redirection operator:

```
[n]<>word
```

shall cause the file whose name is the expansion of *word* to be opened for both reading and writing on the file descriptor denoted by *n*, or standard input if *n* is not specified. If the file does not exist, it shall be created.

### Tests

#### Test: read-write redirection creates file and allows writing

The `<>` operator opens a file for both reading and writing. If the file does
not exist, it is created. This test opens fd 3 for read-write and redirects
stdout to it.

```
begin test "read-write redirection creates file and allows writing"
  script
    echo "rw test" 3<>tmp_rw.txt 1>&3
    cat tmp_rw.txt
  expect
    stdout "rw test"
    stderr ""
    exit_code 0
end test "read-write redirection creates file and allows writing"
```

#### Test: read-write redirection with omitted fd uses standard input

When the file descriptor is omitted for `<>`, the redirection shall refer to
standard input. `cat` can therefore read the file through stdin.

```
begin test "read-write redirection with omitted fd uses standard input"
  script
    printf 'rwline\n' > tmp_rw_stdin.txt
    cat <> tmp_rw_stdin.txt
  expect
    stdout "rwline"
    stderr ""
    exit_code 0
end test "read-write redirection with omitted fd uses standard input"
```

#### Test: read-write redirection opens the designated fd for reading

When an explicit file descriptor is given with `<>`, the file is opened on
that descriptor for reading as well as writing. Duplicating the descriptor to
standard input lets `cat` read from it.

```
begin test "read-write redirection opens the designated fd for reading"
  script
    printf 'rwfd\n' > tmp_rwfd.txt
    cat 4<>tmp_rwfd.txt <&4
  expect
    stdout "rwfd"
    stderr ""
    exit_code 0
end test "read-write redirection opens the designated fd for reading"
```
