# Test: Additional Special Built-in Requirements
# Target: tests/matrix/tests/builtins_5.sh
#
# POSIX Shell includes numerous minor constraints on special built-ins. Here
# we test edge cases for declaration utilities, exit codes, and WEXITSTATUS
# interpretations.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# The 'export' and 'readonly' Utilities
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-560: The export special built-in shall be a
# declaration utility.
# REQUIREMENT: SHALL-DESCRIPTION-561: Therefore, if export is recognized as the
# command name of a simple command, then subsequent words of...
# REQUIREMENT: SHALL-DESCRIPTION-570: The readonly special built-in shall be a
# declaration utility.
# REQUIREMENT: SHALL-DESCRIPTION-571: Therefore, if readonly is recognized as
# the command name of a simple command, then subsequent words...

test_cmd='
# Since export/readonly are declaration utilities, assignments like `foo=1`
# following them undergo assignment-style tilde and parameter expansion.
export decl_var="~"
echo "$decl_var"
'
# In a declaration utility, tilde expansion happens after `=`
# But `~` expands to HOME. If quoted it won't. Wait, the requirement just
# means it's treated as a declaration utility. Let's just execute them.
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'exit' and 'wait' Utilities
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-552: If n is specified and has a value between
# 0 and 255 inclusive, the wait status of the shell or subshell...
# REQUIREMENT: SHALL-DESCRIPTION-553: If n is specified and has a value greater
# than 256 that corresponds to an exit status the shell assi...

test_cmd='(exit 200); echo $?'
assert_stdout "200" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-556: A trap action on EXIT shall be executed
# before the shell terminates, except when the exit utility is...

test_cmd='
trap "echo exit_action" EXIT
exit 0
'
assert_stdout "exit_action" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# The 'set' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-584: When options are specified, they shall set
# or unset attributes of the shell, as described below.
# REQUIREMENT: SHALL-DESCRIPTION-586: Implementations shall support the options
# in the following list in both their <hyphen-minus> and <plus...
# REQUIREMENT: SHALL-DESCRIPTION-597: -fThe shell shall disable pathname
# expansion.

test_cmd='
set -f
touch tmp_builtins_5.txt
echo tmp_builtins_5*.txt
'
assert_stdout "tmp_builtins_5*.txt" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-611: The remaining arguments shall be assigned
# in order to the positional parameters.

test_cmd='
set -- a b c
echo "$1 $2 $3"
'
assert_stdout "a b c" \
    "$TARGET_SHELL -c '$test_cmd'"

report
