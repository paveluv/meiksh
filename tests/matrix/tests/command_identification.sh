#!/bin/sh

# Test: Command Identification and History Utilities
# Target: tests/matrix/tests/command_identification.sh
#
# Tests POSIX intrinsic utilities related to command identification:
# type, hash, command, alias, and unalias.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-TYPE-1294: The type utility shall conform to XBD 12.2
# Utility Syntax Guidelines.
# REQUIREMENT: SHALL-BG-1029: The following operand shall be supported: job_id
# Specify the job to be resumed as a background job.
# REQUIREMENT: SHALL-SH-1017: The following environment variables shall affect
# the execution of sh : ENV This variable, when and only when an interactive
# shell is invoked, shall be subjected to parameter expansion (see 2.6.2
# Parameter Expansion ) by the shell, and the resulting value shall be used as a
# pathname of a file containing shell commands to execute in the current
# environment.
# REQUIREMENT: SHALL-TYPE-1297: The standard output of type contains information
# about each operand in an unspecified format.
# REQUIREMENT: SHALL-STDERR-518: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-SH-1024-DUP746: The following exit values shall be
# returned: 0 The script to be executed consisted solely of zero or more blank
# lines or comments, or both.
# REQUIREMENT: SHALL-TYPE-1300: No error occurred.

test_cmd='
    type sh >/dev/null && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-HASH-1213: The hash utility shall conform to XBD 12.2
# Utility Syntax Guidelines .
# REQUIREMENT: SHALL-V3CHAP02-1021: The following option shall be supported: -p
# Write to standard output a list of commands associated with each condition
# operand.
# REQUIREMENT: SHALL-BG-1029: The following operand shall be supported: job_id
# Specify the job to be resumed as a background job.
# REQUIREMENT: SHALL-SH-1017: The following environment variables shall affect
# the execution of sh : ENV This variable, when and only when an interactive
# shell is invoked, shall be subjected to parameter expansion (see 2.6.2
# Parameter Expansion ) by the shell, and the resulting value shall be used as a
# pathname of a file containing shell commands to execute in the current
# environment.
# REQUIREMENT: SHALL-HASH-1217: The standard output of hash shall be used when
# no arguments are specified.
# REQUIREMENT: SHALL-HASH-1218: This list shall consist of those utilities named
# in previous hash invocations that have been invoked, and may contain those
# invoked and found through the normal command search process.
# REQUIREMENT: SHALL-HASH-1219: This list shall be cleared when the contents of
# the PATH environment variable are changed.
# REQUIREMENT: SHALL-SH-1024-DUP746: The following exit values shall be
# returned: 0 The script to be executed consisted solely of zero or more blank
# lines or comments, or both.

test_cmd='
    hash -r
    hash sh
    hash >/dev/null && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-ALIAS-1076: The alias utility shall conform to XBD 12.2
# Utility Syntax Guidelines.
# REQUIREMENT: SHALL-OPERANDS-010: The following operands shall be supported: -
# A single <hyphen-minus> shall be treated as the first operand and then
# ignored.
# REQUIREMENT: SHALL-SH-1017: The following environment variables shall affect
# the execution of sh : ENV This variable, when and only when an interactive
# shell is invoked, shall be subjected to parameter expansion (see 2.6.2
# Parameter Expansion ) by the shell, and the resulting value shall be used as a
# pathname of a file containing shell commands to execute in the current
# environment.
# REQUIREMENT: SHALL-ALIAS-1026-DUP757: The format for displaying aliases (when
# no operands or only name operands are specified) shall be: "%s=%s\n", name ,
# value The value string shall be written with appropriate quoting so that it is
# suitable for reinput to the shell.
# REQUIREMENT: SHALL-ALIAS-1026-DUP757: The format for displaying aliases (when
# no operands or only name operands are specified) shall be: "%s=%s\n", name ,
# value The value string shall be written with appropriate quoting so that it is
# suitable for reinput to the shell.
# REQUIREMENT: SHALL-V3CHAP02-1019: Each name shall start on a separate line,
# using the format: "%s=%s\n", < name >, < value > The value string shall be
# written with appropriate quoting; see the description of shell quoting in 2.2
# Quoting .
# REQUIREMENT: SHALL-STDERR-518: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-SH-1024-DUP746: The following exit values shall be
# returned: 0 The script to be executed consisted solely of zero or more blank
# lines or comments, or both.
# REQUIREMENT: SHALL-ALIAS-1028: The following exit values shall be returned: 0
# Successful completion.

test_cmd='
    alias foo=bar
    alias foo | grep -q "foo=" && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-UNALIAS-1337: The unalias utility shall conform to XBD 12.2
# Utility Syntax Guidelines .
# REQUIREMENT: SHALL-V3CHAP02-1021: The following option shall be supported: -p
# Write to standard output a list of commands associated with each condition
# operand.
# REQUIREMENT: SHALL-BG-1029: The following operand shall be supported: job_id
# Specify the job to be resumed as a background job.
# REQUIREMENT: SHALL-SH-1017: The following environment variables shall affect
# the execution of sh : ENV This variable, when and only when an interactive
# shell is invoked, shall be subjected to parameter expansion (see 2.6.2
# Parameter Expansion ) by the shell, and the resulting value shall be used as a
# pathname of a file containing shell commands to execute in the current
# environment.
# REQUIREMENT: SHALL-STDERR-518: The standard error shall be used only for
# diagnostic messages.
# REQUIREMENT: SHALL-SH-1024-DUP746: The following exit values shall be
# returned: 0 The script to be executed consisted solely of zero or more blank
# lines or comments, or both.

test_cmd='
    alias foo=bar
    unalias foo
    alias foo >/dev/null 2>&1 || echo pass
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    alias foo=bar
    alias baz=qux
    unalias -a
    alias >/dev/null 2>&1 || echo pass
'
# On some systems, `alias` outputting nothing returns 0, on others 1.
# But it shouldn't list foo=bar.
test_cmd='
    alias foo=bar
    alias baz=qux
    unalias -a
    val=$(alias)
    [ -z "$val" ] && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-COMMAND-1110: The command utility shall conform to XBD 12.2
# Utility Syntax Guidelines .
# REQUIREMENT: SHALL-COMMAND-1037: The following options shall be supported: -p
# Perform the command search using a default value for PATH that is guaranteed
# to find all of the standard utilities.
# REQUIREMENT: SHALL-COMMAND-1112: Executable utilities, regular built-in
# utilities, command_name s including a <slash> character, and any
# implementation-provided functions that are found using the PATH variable (as
# described in 2.9.1.4 Command Search and Execution ), shall be written as
# absolute pathnames.
# REQUIREMENT: SHALL-COMMAND-1113: Shell functions, special built-in utilities,
# regular built-in utilities not associated with a PATH search, and shell
# reserved words shall be written as just their names.
# REQUIREMENT: SHALL-COMMAND-1114: An alias shall be written as a command line
# that represents its alias definition.
# REQUIREMENT: SHALL-COMMAND-1115: Otherwise, no output shall be written and the
# exit status shall reflect that the name was not found.
# REQUIREMENT: SHALL-COMMAND-1116: Although the format of this string is
# unspecified, it shall indicate in which of the following categories
# command_name falls and shall include the information stated: Executable
# utilities, regular built-in utilities, and any implementation-provided
# functions that are found using the PATH variable (as described in 2.9.1.4
# Command Search and Execution ), shall be identified as such and include the
# absolute pathname in the string.
# REQUIREMENT: SHALL-COMMAND-1117: Other shell functions shall be identified as
# functions.
# REQUIREMENT: SHALL-COMMAND-1118: Aliases shall be identified as aliases and
# their definitions included in the string.
# REQUIREMENT: SHALL-COMMAND-1119: Special built-in utilities shall be
# identified as special built-in utilities.
# REQUIREMENT: SHALL-COMMAND-1120: Regular built-in utilities not associated
# with a PATH search shall be identified as regular built-in utilities. (The
# term "regular" need not be used.)
# REQUIREMENT: SHALL-COMMAND-1121: Shell reserved words shall be identified as
# reserved words.
# REQUIREMENT: SHALL-SH-1017: The following environment variables shall affect
# the execution of sh : ENV This variable, when and only when an interactive
# shell is invoked, shall be subjected to parameter expansion (see 2.6.2
# Parameter Expansion ) by the shell, and the resulting value shall be used as a
# pathname of a file containing shell commands to execute in the current
# environment.
# REQUIREMENT: SHALL-COMMAND-1123: The standard output shall be the same as that
# of the invoked utility.
# REQUIREMENT: SHALL-COMMAND-1124: The standard error shall be the same as that
# of the invoked utility.
# REQUIREMENT: SHALL-COMMAND-1040: When the -v or -V options are specified, the
# following exit values shall be returned: 0 Successful completion.
# REQUIREMENT: SHALL-ALIAS-1028: The following exit values shall be returned: 0
# Successful completion.
# REQUIREMENT: SHALL-COMMAND-1127: Otherwise, the exit status of command shall
# be that of the simple command specified by the arguments to command .
# REQUIREMENT: SHALL-COMMAND-1041: Otherwise, the following exit values shall be
# returned: 126 The utility specified by command_name was found but could not be
# invoked.
# REQUIREMENT: SHALL-COMMAND-1129: The utility specified by the command_name
# operand could not be found.
# REQUIREMENT: SHALL-COMMAND-1130: The utility specified by the command_name
# operand could be found, but could not be invoked.
# REQUIREMENT: SHALL-COMMAND-1127: Otherwise, the exit status of command shall
# be that of the simple command specified by the arguments to command .

test_cmd='
    command -v sh >/dev/null && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    command -v missing_cmd_not_found >/dev/null || echo pass
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    command -V sh >/dev/null && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# Command without -v/-V invokes the utility, bypassing aliases and functions
test_cmd='
    alias echo="echo alias_"
    command echo "hello"
'
assert_stdout "hello" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    command missing_cmd_not_found >/dev/null 2>&1
    echo $?
'
assert_stdout "127" "$TARGET_SHELL -c '$test_cmd'"

# Try an unexecutable file
mkdir -p mydir
touch mydir/nonexec
test_cmd='
    PATH="$PWD/mydir:$PATH"
    command nonexec >/dev/null 2>&1
    echo $?
'
assert_stdout "126" "$TARGET_SHELL -c '$test_cmd'"
rm -rf mydir

report
