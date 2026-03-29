# Test: The 'set' Utility and Shell Options
# Target: tests/matrix/tests/set_options.sh
#
# POSIX Shell includes the 'set' utility to manipulate positional parameters
# and toggle various behavioral flags. This suite tests flags like -e (errexit),
# -f (noglob), -u (nounset), -v (verbose), and -x (xtrace).

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The 'set' Utility Formatting
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1019:
# Each name shall start on a separate line, using the format: "%s=%s\n", < name
# >, < value > The value string shall be written with appropriate quoting; see
# the description of shell quoting in 2.2 Quoting .
# REQUIREMENT: SHALL-DESCRIPTION-585:
# The set special built-in shall support XBD 12.2 Utility Syntax Guidelines
# except that options can be specified with either a leading <hyphen-minus>
# (meaning enable the option) or <plus-sign> (meaning disable it) unless
# otherwise specified.
# REQUIREMENT: SHALL-DESCRIPTION-588:
# -b This option shall be supported if the implementation supports the User
# Portability Utilities option.
# REQUIREMENT: SHALL-DESCRIPTION-589:
# When job control and -b are both enabled, the shell shall write asynchronous
# notifications of background job completions (including termination by a
# signal), and may write asynchronous notifications of background job
# suspensions.
# REQUIREMENT: SHALL-DESCRIPTION-594:
# Only the failure of the pipeline itself shall be considered.
# REQUIREMENT: SHALL-DESCRIPTION-596:
# If the exit status of a compound command other than a subshell command was
# the result of a failure while -e was being ignored, then -e shall not apply to
# this command.
# REQUIREMENT: SHALL-DESCRIPTION-612:
# The special parameter '#' shall be set to reflect the number of positional
# parameters.
# REQUIREMENT: SHALL-DESCRIPTION-582:
# The value string shall be written with appropriate quoting; see the
# description of shell quoting in 2.2 Quoting .
# REQUIREMENT: SHALL-DESCRIPTION-583:
# The output shall be suitable for reinput to the shell, setting or resetting,
# as far as possible, the variables that are currently set; read-only variables
# cannot be reset.

test_cmd='
my_weird_var="space and literal and \"quotes\""
output=$(set | grep "my_weird_var=")
unset my_weird_var
eval "$output"
echo "$my_weird_var"
'
assert_stdout "space and literal and \"quotes\"" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-a' Option (allexport)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-587:
# When this option is on, whenever a value is assigned to a variable in the
# current shell execution environment, the export attribute shall be set for the
# variable.

test_cmd='
set -a
auto_exported="yes"
env | grep -q "^auto_exported=yes$" && echo "exported"
'
assert_stdout "exported" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-e' Option (errexit)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-590:
# When job control is disabled, the -b option shall have no effect.

test_cmd='
set -e
echo "start"
false
echo "should not run"
'
assert_stdout "start" \
    "$TARGET_SHELL -c '$test_cmd'"
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-591:
# Asynchronous notification shall not be enabled by default.
# REQUIREMENT: SHALL-DESCRIPTION-592:
# -C (Uppercase C.) Prevent existing regular files from being overwritten by
# the shell's '>' redirection operator (see 2.7.2 Redirecting Output ); the ">|"
# redirection operator shall override this noclobber option for an individual
# file.
# REQUIREMENT: SHALL-DESCRIPTION-593:
# -e When this option is on, when any command fails (for any of the reasons
# listed in 2.8.1 Consequences of Shell Errors or by returning an exit status
# greater than zero), the shell immediately shall exit, as if by executing the
# exit special built-in utility with no arguments, with the following
# exceptions: The failure of any individual command in a multi-command pipeline,
# or of any subshell environments in which command substitution was performed
# during word expansion, shall not cause the shell to exit.

test_cmd='
set -e
if false; then echo "no"; fi
false || true
true && false || true
while false; do echo "no"; done
echo "survived"
'
assert_stdout "survived" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-f' Option (noglob)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-595:
# The -e setting shall be ignored when executing the compound list following
# the while , until , if , or elif reserved word, a pipeline beginning with the
# ! reserved word, or any command of an AND-OR list other than the last.

test_cmd='
set -f
touch tmp_set_f.txt
echo tmp_set_*.txt
'
assert_stdout "tmp_set_*.txt" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-n' Option (noexec)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-598:
# -m This option shall be supported if the implementation supports the User
# Portability Utilities option.

test_cmd='
set -n
echo "should not run"
'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-u' Option (nounset)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-599:
# When this option is enabled, the shell shall perform job control actions as
# described in 2.11 Job Control .

test_cmd='
set -u
echo "start"
echo "${this_var_is_definitely_unset}"
echo "should not run"
'
assert_stdout "start" \
    "$TARGET_SHELL -c '$test_cmd'"
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# @ and * are exempt.
test_cmd='
set -u
for i in "$@"; do echo $i; done
echo "survived"
'
assert_stdout "survived" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-v' Option (verbose)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-600:
# This option shall be enabled by default for interactive shells.

test_cmd='
set -v
echo "testing_verbose"
'
assert_stderr_contains "echo \"testing_verbose\"" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-x' Option (xtrace)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-609:
# -x The shell shall write to standard error a trace for each command after it
# expands the command and before it executes it.

test_cmd='
set -x
echo "testing_xtrace"
'
assert_stderr_contains "echo testing_xtrace" \
    "$TARGET_SHELL -c '$test_cmd'"


report
