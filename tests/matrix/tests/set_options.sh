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
# REQUIREMENT: SHALL-DESCRIPTION-581: Each name shall start on a separate line,
# using the format:
# REQUIREMENT: SHALL-DESCRIPTION-585: The set special built-in shall support
# XBD 12.2 Utility Syntax Guidelines except that options can be...
# REQUIREMENT: SHALL-DESCRIPTION-588: -bThis option shall be supported if the
# implementation supports the User Portability Utilities optio...
# REQUIREMENT: SHALL-DESCRIPTION-589: When job control and -b are both enabled,
# the shell shall write asynchronous notifications of backgr...
# REQUIREMENT: SHALL-DESCRIPTION-594: Only the failure of the pipeline itself
# shall be considered....
# REQUIREMENT: SHALL-DESCRIPTION-596: If the exit status of a compound command
# other than a subshell command was the result of a failure w...
# REQUIREMENT: SHALL-DESCRIPTION-612: The special parameter '#' shall be set
# to reflect the number of positional parameters....
# REQUIREMENT: SHALL-DESCRIPTION-582: "%s=%s\n", <name>, <value> The value
# string shall be written with appropriate quoting...
# REQUIREMENT: SHALL-DESCRIPTION-583: The output shall be suitable for reinput
# to the shell...

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
# REQUIREMENT: SHALL-DESCRIPTION-587: When this option is on, whenever a value
# is assigned to a variable in the current shell execution environment... it
# shall be marked for export.

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
# REQUIREMENT: SHALL-DESCRIPTION-590: -eWhen this option is on, when any
# command fails... the shell shall immediately exit...

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

# REQUIREMENT: SHALL-DESCRIPTION-591: The shell shall not exit if the command
# that fails is part of the command list immediately following a while or until...
# REQUIREMENT: SHALL-DESCRIPTION-592: part of the test following the if or elif
# reserved words...
# REQUIREMENT: SHALL-DESCRIPTION-593: part of an AND or OR list...

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
# REQUIREMENT: SHALL-DESCRIPTION-595: -fThe shell shall disable pathname
# expansion.

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
# REQUIREMENT: SHALL-DESCRIPTION-598: -nThe shell shall read commands but does
# not execute them.

test_cmd='
set -n
echo "should not run"
'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-u' Option (nounset)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-599: -uWhen the shell tries to expand an unset
# parameter other than the '@' and '*' special parameters, it shall write a
# message to standard error...

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
# REQUIREMENT: SHALL-DESCRIPTION-600: -vThe shell shall write its input to
# standard error as it is read.

test_cmd='
set -v
echo "testing_verbose"
'
assert_stderr_contains "echo \"testing_verbose\"" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-x' Option (xtrace)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-622: -xThe shell shall write to standard error
# a trace for each command after it expands the command and before it executes
# it.

test_cmd='
set -x
echo "testing_xtrace"
'
assert_stderr_contains "echo testing_xtrace" \
    "$TARGET_SHELL -c '$test_cmd'"


report
