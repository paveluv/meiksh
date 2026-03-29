#!/bin/sh

# Test: cd utility
# Target: tests/matrix/tests/cd.sh
#
# Tests the POSIX 'cd' intrinsic utility.

. "$MATRIX_DIR/lib.sh"

# REQUIREMENT: SHALL-CD-1086:
# The cd utility shall conform to XBD 12.2 Utility Syntax Guidelines .
# REQUIREMENT: SHALL-CD-1033:
# The following options shall be supported by the implementation: -e If the -P
# option is in effect, the current working directory is successfully changed,
# and the correct value of the PWD environment variable cannot be determined,
# exit with exit status 1.
# REQUIREMENT: SHALL-OPERANDS-010:
# The following operands shall be supported: - A single <hyphen-minus> shall be
# treated as the first operand and then ignored.
# REQUIREMENT: SHALL-CD-1093:
# The following operands shall be supported: directory An absolute or relative
# pathname of the directory that shall become the new working directory.
# REQUIREMENT: SHALL-CD-1095:
# If directory consists of a single '-' (<hyphen-minus>) character, the cd
# utility shall behave as if directory contained the value of the OLDPWD
# environment variable, except that after it sets the value of PWD it shall
# write the new value to standard output.
# REQUIREMENT: SHALL-SH-1017:
# The following environment variables shall affect the execution of sh : ENV
# This variable, when and only when an interactive shell is invoked, shall be
# subjected to parameter expansion (see 2.6.2 Parameter Expansion ) by the
# shell, and the resulting value shall be used as a pathname of a file
# containing shell commands to execute in the current environment.
# REQUIREMENT: SHALL-CD-1097:
# The cd utility shall use this list in its attempt to change the directory, as
# described in the DESCRIPTION.
# REQUIREMENT: SHALL-CD-1098:
# If CDPATH is not set, it shall be treated as if it were an empty string.
# REQUIREMENT: SHALL-CD-1094:
# If the cd utility cannot determine the contents of
# OLDPWD , it shall write a diagnostic message to standard error and exit with a
# non-zero status.
# REQUIREMENT: SHALL-CD-1101:
# If a non-empty directory name from CDPATH is not used, and the directory
# argument is not '-' , there shall be no output.
# REQUIREMENT: SHALL-CD-1097:
# The cd utility shall use this list in its attempt to change the directory, as
# described in the DESCRIPTION.
# REQUIREMENT: SHALL-CD-1099:
# PWD This variable shall be set as specified in the DESCRIPTION.
# REQUIREMENT: SHALL-CD-1033:
# If the -L option is in effect, the PWD environment
# variable shall be updated to point to the logical name of the current working
# directory.
# REQUIREMENT: SHALL-CD-1083:
# If the -P option is in effect, the PWD environment variable shall be set to
# the string that would be output by pwd -P .
# REQUIREMENT: SHALL-CD-1035:
# If the new directory cannot be determined or if an
# error occurred, it shall remain unchanged.
# REQUIREMENT: SHALL-CD-1035:
# If a - operand is used and the change is
# successful, the absolute pathname of the new working directory shall be
# written to the standard output as follows: "%s\n" , < new directory >
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.
# REQUIREMENT: SHALL-SH-1024-DUP746:
# The following exit values shall be returned: 0 The script to be executed
# consisted solely of zero or more blank lines or comments, or both.
# REQUIREMENT: SHALL-CD-1033:
# The directory was successfully changed.

test_cmd='
    mkdir -p foo/bar
    cd foo
    echo "$PWD" | grep -q foo && echo pass1 || echo fail1
    cd bar
    echo "$PWD" | grep -q foo/bar && echo pass2 || echo fail2
    cd - >/dev/null
    echo "$PWD" | grep -q foo$ && echo pass3 || echo fail3
    cd - >/dev/null
    echo "$PWD" | grep -q foo/bar && echo pass4 || echo fail4
'
assert_stdout "pass1
pass2
pass3
pass4" "$TARGET_SHELL -c '$test_cmd'"

# test cd - output
test_cmd='
    mkdir -p abc
    old=$PWD
    cd abc
    cd - | grep -q "^$old" && echo pass || echo fail
'
assert_stdout "pass" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-CD-1090:
# If both -L and -P options are specified, the last of these options shall be
# used and all others ignored.
# REQUIREMENT: SHALL-CD-1091:
# If neither -L nor -P is specified, the operand shall be handled dot-dot
# logically; see the DESCRIPTION.
# REQUIREMENT: SHALL-CD-1088:
# -L Handle the operand dot-dot logically; symbolic link components shall not
# be resolved before dot-dot components are processed (see steps 8. and 9. in
# the DESCRIPTION).
# REQUIREMENT: SHALL-CD-1089:
# -P Handle the operand dot-dot physically; symbolic link components shall be
# resolved before dot-dot components are processed (see step 7. in the
# DESCRIPTION).
test_cmd='
    mkdir -p real/dir
    ln -s real/dir symlink
    cd symbolic >/dev/null 2>&1 || true
    cd symlink
    # By default, logical cd is used.
    cd ..
    # We should be back at start, not in "real"
    echo "$PWD" | grep -q real && echo "fail: physical" || echo "pass logical"
'
assert_stdout "pass logical" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    mkdir -p real/dir2
    ln -s real/dir2 symlink2
    cd symlink2
    # Use -P
    cd -P ..
    # We should be in "real"
    echo "$PWD" | grep -q real && echo "pass physical" || echo "fail logical"
'
assert_stdout "pass physical" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-CD-1094:
# If directory is an empty string, cd shall write a diagnostic message to
# standard error and exit with non-zero status.
assert_exit_code_non_zero "$TARGET_SHELL -c 'cd \"\" 2>/dev/null'"

# REQUIREMENT: SHALL-CD-1094:
# If the cd utility cannot determine the contents of
# OLDPWD , it shall write a diagnostic message to standard error and exit with a
# non-zero status.
test_cmd='
    unset OLDPWD
    cd - 2>/dev/null
'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-CD-1099:
# PWD This variable shall be set as specified in the DESCRIPTION.

report
