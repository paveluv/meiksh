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

#### Test: getopts parses grouped short options

Guideline 5 requires that options without option-arguments can be grouped
behind one `-` delimiter. When getopts encounters `-ab`, it shall parse `a`
and `b` as two separate options.

```
begin test "getopts parses grouped short options"
  script
    set -- -ab
    OPTIND=1
    while getopts "ab" opt "$@"; do
      echo "$opt"
    done
  expect
    stdout "a\nb"
    stderr ""
    exit_code 0
end test "getopts parses grouped short options"
```

#### Test: getopts parses grouped options with trailing option-argument

Guideline 5 requires that one or more options without option-arguments,
followed by at most one option that takes an option-argument, shall be
accepted when grouped behind one `-` delimiter.

```
begin test "getopts parses grouped options with trailing option-argument"
  script
    set -- -ab value
    OPTIND=1
    while getopts "ab:" opt "$@"; do
      echo "$opt ${OPTARG:-none}"
    done
  expect
    stdout "a none\nb value"
    stderr ""
    exit_code 0
end test "getopts parses grouped options with trailing option-argument"
```

#### Test: getopts parses combined option-argument

Per Guideline 6 (with the exception noted in 12.1 item 2), a conforming
implementation shall permit applications to specify the option and
option-argument in the same argument string without intervening blanks.

```
begin test "getopts parses combined option-argument"
  script
    set -- -a -bfoo
    OPTIND=1
    getopts "ab:" opt "$@"
    echo "$opt ${OPTARG-unset}"
    getopts "ab:" opt "$@"
    echo "$opt $OPTARG"
  expect
    stdout "a unset\nb foo"
    stderr ""
    exit_code 0
end test "getopts parses combined option-argument"
```

#### Test: getopts parses separate option-argument

Guideline 6 requires that each option and option-argument shall be a
separate argument. This tests the standard form where `-b` and `foo` are
two separate arguments.

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

#### Test: getopts stops at first non-option operand

Guideline 9 requires that all options shall precede operands on the command
line. When getopts encounters a non-option argument (`file`), it shall stop
processing and not recognize a subsequent `-b` as an option.

```
begin test "getopts stops at first non-option operand"
  script
    set -- -a file -b
    OPTIND=1
    while getopts "ab" opt "$@"; do
      echo "$opt"
    done
    echo "OPTIND=$OPTIND"
  expect
    stdout "a\nOPTIND=2"
    stderr ""
    exit_code 0
end test "getopts stops at first non-option operand"
```

#### Test: getopts stops at -- delimiter

Guideline 10 requires that the first `--` argument that is not an
option-argument shall be accepted as a delimiter indicating the end of
options. After `--`, getopts shall stop and any following arguments shall
be treated as operands.

```
begin test "getopts stops at -- delimiter"
  script
    set -- -a -- -b
    OPTIND=1
    while getopts "ab" opt "$@"; do
      echo "$opt"
    done
    echo "OPTIND=$OPTIND"
  expect
    stdout "a\nOPTIND=3"
    stderr ""
    exit_code 0
end test "getopts stops at -- delimiter"
```

#### Test: set accepts -- to indicate end of options

Guideline 10 requires `--` to signal end of options. After `set --`,
remaining arguments shall become positional parameters even if they
begin with `-`.

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

#### Test: cd accepts -- and processes directory starting with dash

Guideline 10 requires `--` to end option processing. After `cd --`, the
following argument shall be treated as a directory operand even if it begins
with `-`.

```
begin test "cd accepts -- and processes directory starting with dash"
  script
    dir=$(mktemp -d)
    mkdir "$dir/-testdir"
    cd -- "$dir/-testdir" && basename "$PWD"
    rm -rf "$dir"
  expect
    stdout "-testdir"
    stderr ""
    exit_code 0
end test "cd accepts -- and processes directory starting with dash"
```

#### Test: unset accepts -- to indicate end of options

Guideline 10 requires `--` to end option processing. After `unset --`,
the following argument shall be treated as a variable name operand.

```
begin test "unset accepts -- to indicate end of options"
  script
    myvar=foo
    unset -- myvar
    echo "${myvar:-empty}"
  expect
    stdout "empty"
    stderr ""
    exit_code 0
end test "unset accepts -- to indicate end of options"
```

#### Test: export accepts -- to indicate end of options

Guideline 10 requires `--` to end option processing. After `export --`,
the following argument shall be treated as a name operand.

```
begin test "export accepts -- to indicate end of options"
  script
    export -- TESTVAR=hello
    echo "$TESTVAR"
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "export accepts -- to indicate end of options"
```

#### Test: readonly accepts -- to indicate end of options

Guideline 10 requires `--` to end option processing. After `readonly --`,
the following argument shall be treated as a name operand.

```
begin test "readonly accepts -- to indicate end of options"
  script
    readonly -- ROVAR=world
    echo "$ROVAR"
  expect
    stdout "world"
    stderr ""
    exit_code 0
end test "readonly accepts -- to indicate end of options"
```

#### Test: set option order does not matter

Guideline 11 requires that the order of different options relative to one
another shall not matter. Setting `set -f -u` and `set -u -f` shall produce
the same effect.

```
begin test "set option order does not matter"
  script
    set -f -u
    echo "ok1"
    set +f +u
    set -u -f
    echo "ok2"
    set +u +f
  expect
    stdout "ok1\nok2"
    stderr ""
    exit_code 0
end test "set option order does not matter"
```

#### Test: getopts processes repeated option-argument pairs in order

Guideline 11 requires that if an option with option-arguments is repeated,
the option and option-argument combinations shall be interpreted in the order
specified on the command line.

```
begin test "getopts processes repeated option-argument pairs in order"
  script
    set -- -b first -b second
    OPTIND=1
    while getopts "b:" opt "$@"; do
      echo "$opt $OPTARG"
    done
  expect
    stdout "b first\nb second"
    stderr ""
    exit_code 0
end test "getopts processes repeated option-argument pairs in order"
```

#### Test: ulimit accepts grouped short options

Guideline 5 requires that options without option-arguments followed by at
most one option that takes an option-argument shall be accepted when grouped
behind one `-` delimiter. This verifies `ulimit -Sf` groups `-S` (modifier)
and `-f` (resource) correctly.

```
begin test "ulimit accepts grouped short options"
  script
    ulimit -Sf 100
    ulimit -Sf
  expect
    stdout "100"
    stderr ""
    exit_code 0
end test "ulimit accepts grouped short options"
```

#### Test: read accepts -- to indicate end of options

Guideline 10 requires `--` to end option processing. After `read --`,
the following argument shall be treated as a variable name operand.

```
begin test "read accepts -- to indicate end of options"
  script
    echo "hello" | { read -- var; echo "$var"; }
  expect
    stdout "hello"
    stderr ""
    exit_code 0
end test "read accepts -- to indicate end of options"
```
