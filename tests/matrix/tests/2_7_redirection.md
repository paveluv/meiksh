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

Redirection is used to open and close files for the current shell execution environment (see [2.13 Shell Execution Environment](#213-shell-execution-environment)) or for any command. Redirection operators can be used with numbers representing file descriptors (see XBD [*3.141 File Descriptor*](../basedefs/V1_chap03.md#3141-file-descriptor)) as described below.

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

#### Test: pathname expansion does not occur on redirection word

Pathname expansion shall not be performed on the redirection word by a
non-interactive shell. A literal `*` in the redirection target creates a file
with that name rather than expanding against existing filenames.

```
begin test "pathname expansion does not occur on redirection word"
  script
    echo "literal" > tmp_*_redir.txt
    ls tmp_*_redir.txt
  expect
    stdout "tmp_\*_redir.txt"
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

#### Test: append redirection appends to file

Redirection with `>>` appends to an existing file rather than truncating it.
This also exercises that fd numbers 0-9 are supported and that the redirection
components do not appear in command arguments.

```
begin test "append redirection appends to file"
  script
    echo "initial" > tmp_append.txt
    echo "append" >> tmp_append.txt
    cat tmp_append.txt
  expect
    stdout "initial\nappend"
    stderr ""
    exit_code 0
end test "append redirection appends to file"
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

## 2.7.2 Redirecting Output

The two general formats for redirecting output are:

```
[n]>word
[n]>|word
```

where the optional *n* represents the file descriptor number. If the number is omitted, the redirection shall refer to standard output (file descriptor 1).

Output redirection using the `'>'` format shall fail if the *noclobber* option is set (see the description of [*set*](#set) **-C**) and the file named by the expansion of *word* exists and is either a regular file or a symbolic link that resolves to a regular file; it may also fail if the file is a symbolic link that does not resolve to an existing file. The check for existence, file creation, and open operations shall be performed atomically as is done by the [*open*()](../functions/open.md) function as defined in System Interfaces volume of POSIX.1-2024 when the O_CREAT and O_EXCL flags are set, except that if the file exists and is a symbolic link, the open operation need not fail with [EEXIST] unless the symbolic link resolves to an existing regular file. Performing these operations atomically ensures that the creation of lock files and unique (often temporary) files is reliable, with important caveats detailed in [*C.2.7.2 Redirecting Output*](../xrat/V4_xcu_chap01.md#c272-redirecting-output). The check for the type of the file need not be performed atomically with the check for existence, file creation, and open operations. If not, there is a potential race condition that may result in a misleading shell diagnostic message when redirection fails. See XRAT [*C.2.7.2 Redirecting Output*](../xrat/V4_xcu_chap01.md#c272-redirecting-output) for more details.

In all other cases (*noclobber* not set, redirection using `'>'` does not fail for the reasons stated above, or redirection using the `">|"` format), output redirection shall cause the file whose name results from the expansion of *word* to be opened for output on the designated file descriptor, or standard output if none is specified. If the file does not exist, it shall be created as an empty file; otherwise, it shall be opened as if the [*open*()](../functions/open.md) function was called with the O_TRUNC flag set.

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

## 2.7.3 Appending Redirected Output

Appended output redirection shall cause the file whose name results from the expansion of word to be opened for output on the designated file descriptor. The file shall be opened as if the [*open*()](../functions/open.md) function as defined in the System Interfaces volume of POSIX.1-2024 was called with the O_APPEND flag set. If the file does not exist, it shall be created.

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

## 2.7.4 Here-Document

The redirection operators `"<<"` and `"<<-"` both allow redirection of subsequent lines read by the shell to the input of a command. The redirected lines are known as a "here-document".

The here-document shall be treated as a single word that begins after the next **NEWLINE** token and continues until there is a line containing only the delimiter and a `<newline>`, with no `<blank>` characters in between. Then the next here-document starts, if there is one. For the purposes of locating this terminating line, the end of a *command_string* operand (see [*sh*](../utilities/sh.md)) shall be treated as a `<newline>` character, and the end of the *commands* string in `$(commands)` and `` `commands` `` may be treated as a `<newline>`. If the end of input is reached without finding the terminating line, the shell should, but need not, treat this as a redirection error. The format is as follows:

```
[n]<<word
    here-document
delimiter
```

where the optional *n* represents the file descriptor number. If the number is omitted, the here-document refers to standard input (file descriptor 0). It is unspecified whether the file descriptor is opened as a regular file or some other type of file. Portable applications cannot rely on the file descriptor being seekable (see XSH [*lseek*()](../functions/lseek.md)).

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
