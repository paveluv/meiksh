# Test Suite for 2.15 Special Built-In Utilities

This test suite covers the **preamble** of Section 2.15 Special Built-In
Utilities of the POSIX.1-2024 Shell Command Language specification.
Individual utilities are tested in separate `builtin_*.md` files.

## Table of contents

- [2.15 Special Built-In Utilities](#215-special-built-in-utilities)

## 2.15 Special Built-In Utilities

The following "special built-in" utilities shall be supported in the shell command language. The output of each command, if any, shall be written to standard output, subject to the normal redirection and piping possible with all commands.

The term "built-in" implies that there is no need to execute a separate executable file because the utility is implemented in the shell itself. An implementation may choose to make any utility a built-in; however, the special built-in utilities described here differ from regular built-in utilities in two respects:

1. An error in a special built-in utility may cause a shell executing that utility to abort, while an error in a regular built-in utility shall not cause a shell executing that utility to abort. (See [2.8.1 Consequences of Shell Errors](#281-consequences-of-shell-errors) for the consequences of errors on interactive and non-interactive shells.) If a special built-in utility encountering an error does not abort the shell, its exit value shall be non-zero.
2. As described in [2.9.1 Simple Commands](#291-simple-commands), variable assignments preceding the invocation of a special built-in utility affect the current execution environment; this shall not be the case with a regular built-in or other utility.

The special built-in utilities in this section need not be provided in a manner accessible via the *exec* family of functions defined in the System Interfaces volume of POSIX.1-2024.

Some of the special built-ins are described as conforming to XBD [*12.2 Utility Syntax Guidelines*](docs/posix/md/basedefs/V1_chap12.md#122-utility-syntax-guidelines). For those that are not, the requirement in [*1.4 Utility Description Defaults*](docs/posix/md/utilities/V3_chap01.md#14-utility-description-defaults) that `"--"` be recognized as a first argument to be discarded does not apply and a conforming application shall not use that argument.

### Tests

#### Test: special built-in output goes to stdout

Output from special built-ins goes to standard output.

```
begin test "special built-in output goes to stdout"
  script
    export FOO=bar
    export -p > /dev/null
    echo ok
  expect
    stdout "ok"
    stderr ""
    exit_code 0
end test "special built-in output goes to stdout"
```

#### Test: error in special built-in produces non-zero exit

An error in a special built-in produces a non-zero exit status.

```
begin test "error in special built-in produces non-zero exit"
  script
    shift 999 2>/dev/null
  expect
    stdout ""
    stderr ""
    exit_code !=0
end test "error in special built-in produces non-zero exit"
```

#### Test: assignment before special built-in persists

Variable assignments preceding a special built-in affect the
current execution environment and persist after the utility completes.

```
begin test "assignment before special built-in persists"
  script
    FOO=bar eval 'echo $FOO'
  expect
    stdout "bar"
    stderr ""
    exit_code 0
end test "assignment before special built-in persists"
```
