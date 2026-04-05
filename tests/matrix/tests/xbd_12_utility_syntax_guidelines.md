# Test Suite for XBD 12.2 Utility Syntax Guidelines

This test suite covers XBD Section 12.2 (Utility Syntax Guidelines) from the
POSIX.1-2024 Base Definitions. These guidelines are relevant to the shell
because all shell built-in utilities and intrinsic utilities shall conform to
them, including `--` end-of-options handling and option-argument parsing.

## Table of contents

- [xbd: 12.2 Utility Syntax Guidelines](#xbd-122-utility-syntax-guidelines)

## xbd: 12.2 Utility Syntax Guidelines

The following guidelines are established for the naming of utilities and for the specification of options, option-arguments, and operands. The [*getopt*()](docs/posix/md/functions/getopt.md) function in the System Interfaces volume of POSIX.1-2024 assists utilities in handling options and operands that conform to these guidelines.

Operands and option-arguments can contain characters not specified in the portable character set.

The guidelines are intended to provide guidance to the authors of future utilities, such as those written specific to a local system or that are components of a larger application. Some of the standard utilities do not conform to all of these guidelines; in those cases, the OPTIONS sections describe the deviations.

- **Guideline 1:** Utility names should be between two and nine characters, inclusive.
- **Guideline 2:** Utility names should include lowercase letters (the **lower** character classification) and digits only from the portable character set.
- **Guideline 3:** Each option name should be a single alphanumeric character (the **alnum** character classification) from the portable character set. The **-W** (capital-W) option shall be reserved for vendor options. Multi-digit options should not be allowed.
- **Guideline 4:** All options should be preceded by the `'-'` delimiter character.
- **Guideline 5:** One or more options without option-arguments, followed by at most one option that takes an option-argument, should be accepted when grouped behind one `'-'` delimiter.
- **Guideline 6:** Each option and option-argument should be a separate argument, except as noted in [12.1 Utility Argument Syntax](#121-utility-argument-syntax), item (2).
- **Guideline 7:** Option-arguments should not be optional.
- **Guideline 8:** When multiple option-arguments are specified to follow a single option, they should be presented as a single argument, using `<comma>` characters within that argument or `<blank>` characters within that argument to separate them.
- **Guideline 9:** All options should precede operands on the command line.
- **Guideline 10:** The first **--** argument that is not an option-argument should be accepted as a delimiter indicating the end of options. Any following arguments should be treated as operands, even if they begin with the `'-'` character.
- **Guideline 11:** The order of different options relative to one another should not matter, unless the options are documented as mutually-exclusive and such an option is documented to override any incompatible options preceding it. If an option that has option-arguments is repeated, the option and option-argument combinations should be interpreted in the order specified on the command line.
- **Guideline 12:** The order of operands may matter and position-related interpretations should be determined on a utility-specific basis.
- **Guideline 13:** For utilities that use operands to represent files to be opened for either reading or writing, the `'-'` operand should be used to mean only standard input (or standard output when it is clear from context that an output file is being specified) or a file named **-**.
- **Guideline 14:** If an argument can be identified according to Guidelines 3 through 10 as an option, or as a group of options without option-arguments behind one `'-'` delimiter, then it should be treated as such.

The utilities in the Shell and Utilities volume of POSIX.1-2024 that claim conformance to these guidelines shall conform completely to these guidelines as if these guidelines contained the term "shall" instead of "should". On some implementations, the utilities accept usage in violation of these guidelines for backwards-compatibility as well as accepting the required form.

Where a utility described in the Shell and Utilities volume of POSIX.1-2024 as conforming to these guidelines is required to accept, or not to accept, the operand `'-'` to mean standard input or output, this usage is explained in the OPERANDS section. Otherwise, if such a utility uses operands to represent files, it is implementation-defined whether the operand `'-'` stands for standard input (or standard output), or for a file named **-**.

It is recommended that all future utilities and applications use these guidelines to enhance user portability. The fact that some historical utilities could not be changed (to avoid breaking existing applications) should not deter this future goal.

### Tests

#### Test: cd accepts -- and processes directory starting with dash

Guideline 10 requires that `--` be accepted as a delimiter indicating the end of options. Any following arguments shall be treated as operands, even if they begin with `-`. This test verifies `cd -- -dir` works correctly.

```
begin test "cd accepts -- and processes directory starting with dash"
  script
    mkdir -p ./-dir
    cd -- -dir && echo "$PWD" | grep -q -- "-dir$" && echo success || echo fail
    cd ..
    rm -rf ./-dir
  expect
    stdout ".*success.*"
    stderr ""
    exit_code 0
end test "cd accepts -- and processes directory starting with dash"
```

#### Test: set accepts -- to indicate end of options

The `--` argument indicates the end of options for the `set` built-in. After `--`, remaining arguments become positional parameters, even if they look like options.

```
begin test "set accepts -- to indicate end of options"
  script
    set -- -x -e
    echo "$1 $2"
  expect
    stdout "-x -e"
    stderr ""
    exit_code 0
end test "set accepts -- to indicate end of options"
```

#### Test: unset accepts --

The `unset` built-in shall accept `--` to indicate end of options per Guideline 10.

```
begin test "unset accepts --"
  script
    myvar=foo
    unset -- myvar
    echo "${myvar:-empty}"
  expect
    stdout "empty"
    stderr ""
    exit_code 0
end test "unset accepts --"
```

#### Test: getopts parses combined option-argument

Per 12.1 item 2, a conforming implementation shall permit applications to specify the option and option-argument in the same argument string without intervening blanks (e.g., `-bfoo` for `-b foo`).

```
begin test "getopts parses combined option-argument"
  script
    getopts "ab:" opt -a -bfoo
    echo "$opt $OPTARG"
  expect
    stdout "a.*"
    stderr ""
    exit_code 0
end test "getopts parses combined option-argument"
```

#### Test: getopts parses separate option-argument

Per Guideline 6, each option and option-argument should be a separate argument. This is the standard form: `-b foo` as two separate arguments.

```
begin test "getopts parses separate option-argument"
  script
    getopts "ab:" opt -b foo
    echo "$opt $OPTARG"
  expect
    stdout "b foo"
    stderr ""
    exit_code 0
end test "getopts parses separate option-argument"
```

#### Test: ulimit -Sf with value adjacent

Per 12.1 item 2, when the SYNOPSIS shows an optional option-argument, a conforming application places the option-argument directly adjacent to the option. `ulimit -Sf` combines the `-S` and `-f` flags, then accepts a value.

```
begin test "ulimit -Sf with value adjacent"
  script
    ulimit -Sf 100
    ulimit -Sf
  expect
    stdout "100"
    stderr ""
    exit_code 0
end test "ulimit -Sf with value adjacent"
```

#### Test: kill -l lists signals

The `kill -l` option lists available signal names. This verifies the utility accepts the `-l` option and produces output containing known signal names.

```
begin test "kill -l lists signals"
  script
    kill -l 2>/dev/null
  expect
    stdout "([^\n]*\n)*.*(HUP|INT|TERM).*(\n.*)*"
    stderr ""
    exit_code 0
end test "kill -l lists signals"
```
