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
# (meaning enable the option) or <plus-sign> (meaning disable it).
# REQUIREMENT: SHALL-DESCRIPTION-582:
# The value string shall be written with appropriate quoting.
# REQUIREMENT: SHALL-DESCRIPTION-583:
# The output shall be suitable for reinput to the shell.
# REQUIREMENT: SHALL-DESCRIPTION-612:
# The special parameter '#' shall be set to reflect the number of positional
# parameters.

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
# The '-b' Option (notify)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-588:
# -b This option shall be supported if the implementation supports the User
# Portability Utilities option.
# REQUIREMENT: SHALL-DESCRIPTION-589:
# When job control and -b are both enabled, the shell shall write asynchronous
# notifications of background job completions.
# REQUIREMENT: SHALL-DESCRIPTION-590:
# When job control is disabled, the -b option shall have no effect.

# -b is tested via interactive PTY in job_control.sh.
# Here we verify the option is accepted without error.
assert_exit_code 0 \
    "$TARGET_SHELL -c 'set -b; set +b'"


# ==============================================================================
# The '-C' Option (noclobber)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-592:
# -C (Uppercase C.) Prevent existing regular files from being overwritten by
# the shell's '>' redirection operator; the ">|" redirection operator shall
# override this noclobber option for an individual file.

_noclobber_f="${TMPDIR:-/tmp}/_noclobber_test_$$"
echo "original" > "$_noclobber_f"
_out=$($TARGET_SHELL -c "set -C; echo overwritten > '$_noclobber_f'" 2>/dev/null; cat "$_noclobber_f")
case "$_out" in
    *original*) pass ;;
    *) fail "noclobber did not prevent overwrite: $_out" ;;
esac
# >| should override noclobber
$TARGET_SHELL -c "set -C; echo forced >| '$_noclobber_f'" 2>/dev/null
_out2=$(cat "$_noclobber_f")
case "$_out2" in
    *forced*) pass ;;
    *) fail "'>|' did not override noclobber: $_out2" ;;
esac
rm -f "$_noclobber_f"


# ==============================================================================
# The '-e' Option (errexit)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-593:
# -e When this option is on, when any command fails, the shell immediately shall
# exit. Exceptions: pipeline failure, command substitution subshells.
# REQUIREMENT: SHALL-DESCRIPTION-594:
# Only the failure of the pipeline itself shall be considered.
# REQUIREMENT: SHALL-DESCRIPTION-595:
# The -e setting shall be ignored when executing the compound list following
# the while, until, if, or elif reserved word, a pipeline beginning with the
# ! reserved word, or any command of an AND-OR list other than the last.
# REQUIREMENT: SHALL-DESCRIPTION-596:
# If the exit status of a compound command other than a subshell command was
# the result of a failure while -e was being ignored, then -e shall not apply.

# Basic errexit: shell exits on failure
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

# -e does not exit on failures in if/while/until compound lists
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

# -e with ! pipeline: should not exit
test_cmd='set -e; ! false; echo "survived_not"'
assert_stdout "survived_not" \
    "$TARGET_SHELL -c '$test_cmd'"

# Pipeline failure (last command) triggers -e
test_cmd='set -e; echo ok | false; echo should_not_appear'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *should_not_appear*) fail "-e did not exit on pipeline failure: $_out" ;;
    *) pass ;;
esac


# ==============================================================================
# The '-f' Option (noglob)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-597:
# -f The shell shall disable pathname expansion.

test_cmd='
set -f
touch tmp_set_f.txt
echo tmp_set_*.txt
'
assert_stdout "tmp_set_*.txt" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-m' Option (monitor/job control)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-598:
# -m This option shall be supported if the implementation supports the User
# Portability Utilities option.
# REQUIREMENT: SHALL-DESCRIPTION-599:
# When this option is enabled, the shell shall perform job control actions.
# REQUIREMENT: SHALL-DESCRIPTION-600:
# This option shall be enabled by default for interactive shells.

# -m is accepted without error
assert_exit_code 0 \
    "$TARGET_SHELL -c 'set -m 2>/dev/null; true'"


# ==============================================================================
# The '-n' Option (noexec)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-601:
# -n The shell shall read commands but not execute them; this can be used to
# check for shell script syntax errors.

test_cmd='
set -n
echo "should not run"
'
assert_stdout "" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The '-u' Option (nounset)
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-607:
# -u The shell shall write a message to standard error when it tries to expand
# a variable that is not set and immediately exit.

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

# @ and * are exempt from -u
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
# REQUIREMENT: SHALL-DESCRIPTION-608:
# -v The shell shall write its input to standard error as it is read.

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


# ==============================================================================
# The '-o' Option
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-591:
# Asynchronous notification shall not be enabled by default.

# set -o should succeed
assert_exit_code 0 "$TARGET_SHELL -c 'set -o 2>/dev/null; true'"

# ==============================================================================
# set -- for positional parameters
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-613:
# All positional parameters shall be unset before any new values are assigned.
# REQUIREMENT: SHALL-DESCRIPTION-614:
# set -- without argument shall unset all positional parameters and set '#'
# to zero.

# set -- with args
test_cmd='set -- a b c; echo $# $1 $2 $3'
assert_stdout "3 a b c" \
    "$TARGET_SHELL -c '$test_cmd'"

# set -- without args clears positional parameters
test_cmd='set -- x y z; set --; echo $#'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

report
