#!/bin/sh

# Test: cd utility — extended coverage
# Target: tests/matrix/tests/cd_extended.sh
#
# Extended tests for the POSIX 'cd' intrinsic: -L/-P modes, CDPATH,
# cd - / cd with no args, dot/dot-dot handling, and error conditions.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# cd -L (logical mode) — follows symlinks in PWD
# ==============================================================================
# REQUIREMENT: SHALL-CD-1070:
# The cd utility shall change the working directory of the current shell
# execution environment.
# REQUIREMENT: SHALL-CD-1080:
# The cd utility shall then perform actions equivalent to the chdir()
# function called with curpath as the path argument.
# REQUIREMENT: SHALL-CD-1082:
# If the -P option is not in effect, the PWD environment variable shall be
# set to the value that curpath had on entry.
# REQUIREMENT: SHALL-CD-1088:
# -L Handle the operand dot-dot logically; symbolic link components shall not
# be resolved before dot-dot components are processed.
# REQUIREMENT: SHALL-CD-1033:
# If the -L option is in effect, the PWD environment variable shall be updated
# to point to the logical name of the current working directory.
# REQUIREMENT: SHALL-CD-1086:
# The cd utility shall conform to XBD 12.2 Utility Syntax Guidelines.
# REQUIREMENT: SHALL-CD-1099:
# PWD This variable shall be set as specified in the DESCRIPTION.

test_cmd='
    mkdir -p real_target/sub
    ln -s real_target link_l
    cd -L link_l
    case "$PWD" in
        */link_l) echo pass_pwd_logical ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_pwd_logical" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    mkdir -p real_target2/sub
    ln -s real_target2 link_l2
    cd -L link_l2/sub
    case "$PWD" in
        */link_l2/sub) echo pass_logical_sub ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_logical_sub" "$TARGET_SHELL -c '$test_cmd'"

# cd -L .. should go to logical parent (where the symlink lives)
# REQUIREMENT: SHALL-CD-1088:
# Symbolic link components shall not be resolved before dot-dot components
# are processed.
test_cmd='
    mkdir -p real_deep/child
    ln -s real_deep link_deep
    cd -L link_deep/child
    cd -L ..
    case "$PWD" in
        */link_deep) echo pass_logical_dotdot ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_logical_dotdot" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# cd -P (physical mode) — resolves symlinks
# ==============================================================================
# REQUIREMENT: SHALL-CD-1074:
# The curpath value shall then be converted to canonical form.
# REQUIREMENT: SHALL-CD-1075:
# For each dot-dot component, if there is a preceding component and it is
# neither root nor dot-dot, the preceding and dot-dot components are removed.
# REQUIREMENT: SHALL-CD-1076:
# The preceding component, all slash chars, dot-dot, and following slash removed.
# REQUIREMENT: SHALL-CD-1089:
# -P Handle the operand dot-dot physically; symbolic link components shall be
# resolved before dot-dot components are processed.
# REQUIREMENT: SHALL-CD-1083:
# If the -P option is in effect, the PWD environment variable shall be set to
# the string that would be output by pwd -P.

test_cmd='
    mkdir -p phys_real/inner
    ln -s phys_real link_p
    cd -P link_p
    case "$PWD" in
        */phys_real) echo pass_physical ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_physical" "$TARGET_SHELL -c '$test_cmd'"

# cd -P .. from inside a symlinked dir should resolve the real path first
test_cmd='
    mkdir -p phys_real2/inner2
    ln -s phys_real2/inner2 link_p2
    cd -P link_p2
    cd -P ..
    case "$PWD" in
        */phys_real2) echo pass_physical_dotdot ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_physical_dotdot" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# -L and -P together: last one wins
# ==============================================================================
# REQUIREMENT: SHALL-CD-1090:
# If both -L and -P options are specified, the last of these options shall be
# used and all others ignored.

test_cmd='
    mkdir -p combo_real
    ln -s combo_real link_combo
    cd -L -P link_combo
    case "$PWD" in
        */combo_real) echo pass_last_P ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_last_P" "$TARGET_SHELL -c '$test_cmd'"

test_cmd='
    mkdir -p combo_real2
    ln -s combo_real2 link_combo2
    cd -P -L link_combo2
    case "$PWD" in
        */link_combo2) echo pass_last_L ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_last_L" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# CDPATH search behavior
# ==============================================================================
# REQUIREMENT: SHALL-CD-1034:
# CDPATH: A colon-separated list of pathnames.
# REQUIREMENT: SHALL-CD-1097:
# The cd utility shall use this list in its attempt to change the directory,
# as described in the DESCRIPTION.
# REQUIREMENT: SHALL-CD-1098:
# If CDPATH is not set, it shall be treated as if it were an empty string.

test_cmd='
    mkdir -p /tmp/_cd_test_cdpath/searchdir/target_dir
    CDPATH=/tmp/_cd_test_cdpath/searchdir
    export CDPATH
    cd target_dir
    case "$PWD" in
        */target_dir) echo pass_cdpath ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
    rm -rf /tmp/_cd_test_cdpath
'
assert_stdout "pass_cdpath" "$TARGET_SHELL -c '$test_cmd'"

# CDPATH with multiple entries separated by colon
test_cmd='
    mkdir -p /tmp/_cd_test_cp2/a /tmp/_cd_test_cp2/b/found_here
    CDPATH=/tmp/_cd_test_cp2/a:/tmp/_cd_test_cp2/b
    export CDPATH
    cd found_here
    case "$PWD" in
        */found_here) echo pass_cdpath_multi ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
    rm -rf /tmp/_cd_test_cp2
'
assert_stdout "pass_cdpath_multi" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-CD-1101:
# If a non-empty directory name from CDPATH is not used, and the directory
# argument is not '-', there shall be no output.
test_cmd='
    mkdir -p local_only
    unset CDPATH
    output=$(cd local_only 2>&1)
    if [ -z "$output" ]; then
        echo pass_no_output
    else
        echo "fail: output=$output"
    fi
'
assert_stdout "pass_no_output" "$TARGET_SHELL -c '$test_cmd'"

# When CDPATH is used the new directory should be printed to stdout
test_cmd='
    mkdir -p /tmp/_cd_test_cp3/cdout_dir
    CDPATH=/tmp/_cd_test_cp3
    export CDPATH
    output=$(cd cdout_dir)
    case "$output" in
        */cdout_dir) echo pass_cdpath_output ;;
        *) echo "fail: output=$output" ;;
    esac
    rm -rf /tmp/_cd_test_cp3
'
assert_stdout "pass_cdpath_output" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# cd - (previous directory using OLDPWD)
# ==============================================================================
# REQUIREMENT: SHALL-CD-1085:
# If, during the execution of the above steps, the PWD environment variable
# is set, the OLDPWD shell variable shall also be set to the value of the
# old working directory.
# REQUIREMENT: SHALL-CD-1095:
# If directory consists of a single '-', the cd utility shall behave as if
# directory contained the value of the OLDPWD environment variable, except
# that after it sets the value of PWD it shall write the new value to
# standard output.
# REQUIREMENT: SHALL-CD-1035:
# If a - operand is used and the change is successful, the absolute pathname
# of the new working directory shall be written to the standard output.

test_cmd='
    first=$PWD
    mkdir -p cd_dash_test
    cd cd_dash_test
    second=$PWD
    output=$(cd -)
    if [ "$PWD" = "$first" ] && [ "$output" = "$first" ]; then
        echo pass_cd_dash
    else
        echo "fail: PWD=$PWD output=$output expected=$first"
    fi
'
assert_stdout "pass_cd_dash" "$TARGET_SHELL -c '$test_cmd'"

# cd - should update OLDPWD to the directory we left
test_cmd='
    mkdir -p dash_old1 dash_old2
    cd dash_old1
    cd dash_old2
    cd - >/dev/null
    case "$OLDPWD" in
        */dash_old2) echo pass_oldpwd_updated ;;
        *) echo "fail: OLDPWD=$OLDPWD" ;;
    esac
'
assert_stdout "pass_oldpwd_updated" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-CD-1094:
# If the cd utility cannot determine the contents of OLDPWD, it shall write
# a diagnostic message to standard error and exit with a non-zero status.
test_cmd='unset OLDPWD; cd - 2>/dev/null'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# cd with no arguments (goes to HOME)
# ==============================================================================
# REQUIREMENT: SHALL-CD-1071:
# If no directory operand is given and HOME is empty or undefined, the
# default behavior is implementation-defined.
# REQUIREMENT: SHALL-CD-1072:
# If no directory operand is given and HOME is set to a non-empty value,
# the cd utility shall behave as if the directory named in HOME was specified.
# REQUIREMENT: SHALL-CD-1093:
# The following operands shall be supported: directory.
# REQUIREMENT: SHALL-OPERANDS-010:
# If directory is not specified, cd shall behave as if the directory named
# in the HOME environment variable were specified as the directory operand.

test_cmd='
    export HOME=/tmp
    cd /
    cd
    if [ "$PWD" = "/tmp" ]; then
        echo pass_cd_home
    else
        echo "fail: PWD=$PWD"
    fi
'
assert_stdout "pass_cd_home" "$TARGET_SHELL -c '$test_cmd'"

# Unset HOME: cd with no args should produce an error
test_cmd='unset HOME; cd 2>/dev/null'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# . and .. handling in paths
# ==============================================================================
# REQUIREMENT: SHALL-CD-1091:
# If neither -L nor -P is specified, the operand shall be handled dot-dot
# logically.

test_cmd='
    mkdir -p dot_a/dot_b
    cd dot_a/dot_b
    inner=$PWD
    cd ./../../dot_a/./dot_b
    if [ "$PWD" = "$inner" ]; then
        echo pass_dot_handling
    else
        echo "fail: PWD=$PWD expected=$inner"
    fi
'
assert_stdout "pass_dot_handling" "$TARGET_SHELL -c '$test_cmd'"

# cd to "." should remain in current directory
test_cmd='
    before=$PWD
    cd .
    if [ "$PWD" = "$before" ]; then
        echo pass_cd_dot
    else
        echo "fail: PWD=$PWD expected=$before"
    fi
'
assert_stdout "pass_cd_dot" "$TARGET_SHELL -c '$test_cmd'"

# cd .. from a subdirectory should go to parent
test_cmd='
    mkdir -p dotdot_parent/dotdot_child
    cd dotdot_parent/dotdot_child
    cd ..
    case "$PWD" in
        */dotdot_parent) echo pass_cd_dotdot ;;
        *) echo "fail: PWD=$PWD" ;;
    esac
'
assert_stdout "pass_cd_dotdot" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Error conditions
# ==============================================================================
# REQUIREMENT: SHALL-CD-1081:
# If these actions fail for any reason, cd shall display an appropriate error
# message and the command shall not alter the working directory.
# REQUIREMENT: SHALL-CD-1036:
# Exit value 0 if current working directory successfully changed.
# REQUIREMENT: SHALL-CD-1103:
# The working directory shall remain unchanged.
# REQUIREMENT: SHALL-CD-1094:
# If directory is an empty string, cd shall write a diagnostic message to
# standard error and exit with a non-zero exit status.
# REQUIREMENT: SHALL-CD-1035:
# If the new directory cannot be determined or if an error occurred, it shall
# remain unchanged.
# REQUIREMENT: SHALL-STDERR-518:
# The standard error shall be used only for diagnostic messages.

# cd to a regular file (not a directory) should fail
test_cmd='
    touch not_a_dir_file
    cd not_a_dir_file 2>/dev/null
'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# cd to a nonexistent path should fail
test_cmd='cd /nonexistent_path_xyz_12345 2>/dev/null'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# cd to a directory with no execute permission should fail
test_cmd='
    mkdir -p no_perm_dir
    chmod 000 no_perm_dir
    cd no_perm_dir 2>/dev/null
    rc=$?
    chmod 755 no_perm_dir
    exit $rc
'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

# PWD should remain unchanged after a failed cd
test_cmd='
    before=$PWD
    cd /nonexistent_xyz_99999 2>/dev/null || true
    if [ "$PWD" = "$before" ]; then
        echo pass_pwd_unchanged
    else
        echo "fail: PWD=$PWD expected=$before"
    fi
'
assert_stdout "pass_pwd_unchanged" "$TARGET_SHELL -c '$test_cmd'"

# cd to an empty string should fail
# REQUIREMENT: SHALL-CD-1094:
# If directory is an empty string, cd shall write a diagnostic message to
# standard error and exit with non-zero status.
test_cmd='cd "" 2>/dev/null; echo survived'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
# cd "" should fail; if it doesn't fail, the test should still detect it
# by checking that the exit code of cd was non-zero
assert_exit_code_non_zero "$TARGET_SHELL -c 'cd \"\" 2>/dev/null'"

report
