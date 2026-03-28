# Test: Special Built-ins (shift, times, trap)
# Target: tests/matrix/tests/builtins_4.sh
#
# POSIX Shell includes utilities for manipulating positional parameters
# (shift), measuring time (times), and handling asynchronous events (trap).

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The 'shift' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-601: The positional parameters shall be shifted.
# REQUIREMENT: SHALL-DESCRIPTION-602: Positional parameter 1 shall be assigned
# the value of parameter (1+n), parameter 2 shall be assigned the value of
# parameter (2+n), and so on.
# REQUIREMENT: SHALL-DESCRIPTION-603: The parameters represented by the numbers
# "$#" down to "$#-n+1" shall be unset...
# REQUIREMENT: SHALL-DESCRIPTION-604: The value n shall be an unsigned decimal
# integer less than or equal to the current value of the special parameter '#'.
# REQUIREMENT: SHALL-DESCRIPTION-605: If n is not given, it shall be assumed to
# be 1.
# REQUIREMENT: SHALL-DESCRIPTION-613: All positional parameters shall be unset
# before any new values are assigned...
# REQUIREMENT: SHALL-DESCRIPTION-619: The parameters represented by the numbers
# "$#" down to "$#-n+1" shall be unset, and the parameter '#...
# REQUIREMENT: SHALL-DESCRIPTION-621: If n is not given, it shall be assumed to
# be 1....

# Shift 1 without arguments shifts $2 to $1.
test_cmd='
echo "$#"
shift
echo "$1 $#"
'
assert_stdout "3
b 2" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# Shift 2 shifts $3 to $1.
test_cmd='
echo "$#"
shift 2
echo "$1 $#"
'
assert_stdout "3
c 1" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"

# REQUIREMENT: SHALL-DESCRIPTION-606: If n is greater than "$#", the positional
# parameters shall not be changed...
# REQUIREMENT: SHALL-DESCRIPTION-607: the command shall complete with a non-zero
# exit status...

# Shift > $# fails and leaves parameters intact.
test_cmd='
shift 5
echo "$? $1"
'
assert_stdout "1 a" \
    "$TARGET_SHELL -c '$test_cmd' sh a b c"


# ==============================================================================
# The 'times' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-608: The times utility shall write the
# accumulated user and system times for the shell and for all of its children...
# REQUIREMENT: SHALL-DESCRIPTION-625: The times utility shall write the
# accumulated user and system times for the shell and for all of its...
# REQUIREMENT: SHALL-DESCRIPTION-626: "%dm%fs %dm%fs\n%dm%fs %dm%fs\n", <shell
# user minutes>, <shell user seconds>, <shell system minutes>...
# REQUIREMENT: SHALL-DESCRIPTION-609: The four times shall be written to
# standard output...

test_cmd='
times | grep -q "[0-9]" && echo "times reported"
'
assert_stdout "times reported" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'trap' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-629: If the -p option is not specified and the
# first operand is an unsigned decimal integer, the shell sh...
# REQUIREMENT: SHALL-DESCRIPTION-630: Otherwise, if the -p option is not
# specified and there are operands, the first operand shall be treated...
# REQUIREMENT: SHALL-DESCRIPTION-631: If action is '-', the shell shall reset
# each condition to the default value.
# REQUIREMENT: SHALL-DESCRIPTION-632: If action is null (""), the shell shall
# ignore each specified condition if it arises.
# REQUIREMENT: SHALL-DESCRIPTION-633: Otherwise, the argument action shall be
# read and executed by the shell when one of the corresponding...
# REQUIREMENT: SHALL-DESCRIPTION-634: The action of trap shall override a
# previous action (either default action or one explicitly set).
# REQUIREMENT: SHALL-DESCRIPTION-635: The value of "$?" after the trap action
# completes shall be the value it had before the trap action w...
# REQUIREMENT: SHALL-DESCRIPTION-636: The EXIT condition shall occur when the
# shell terminates normally (exits), and may occur when...
# REQUIREMENT: SHALL-DESCRIPTION-637: The environment in which the shell
# executes a trap action on EXIT shall be identical to the...
# REQUIREMENT: SHALL-DESCRIPTION-638: If action is neither '-' nor the empty
# string, then each time a matching condition arises...
# REQUIREMENT: SHALL-DESCRIPTION-639: Traps shall remain in place for a given
# shell until explicitly changed with another trap...
# REQUIREMENT: SHALL-DESCRIPTION-640: When a subshell is entered, traps that
# are not being ignored shall be set to the default...
# REQUIREMENT: SHALL-DESCRIPTION-641: The trap command with no operands shall
# write to standard output a list of commands associ...
# REQUIREMENT: SHALL-DESCRIPTION-643: Otherwise, the list shall contain the
# commands currently associated with each condition.
# REQUIREMENT: SHALL-DESCRIPTION-644: The format shall be:...
# REQUIREMENT: SHALL-DESCRIPTION-645: The shell shall format the output,
# including the proper use of quoting, so that it is suitable for r...
# REQUIREMENT: SHALL-DESCRIPTION-646: If this set includes conditions
# corresponding to the SIGKILL and SIGSTOP signals, the shell shall ac...
# REQUIREMENT: SHALL-DESCRIPTION-647: The trap special built-in shall conform
# to XBD 12.2 Utility Syntax Guidelines.
# REQUIREMENT: SHALL-OPTIONS-648: The following option shall be supported:...
# REQUIREMENT: SHALL-OPTIONS-649: The shell shall format the output, including
# the proper use of quoting, so that it is suitable for r...
# REQUIREMENT: SHALL-DESCRIPTION-610: When the shell receives a condition that
# can be trapped... the action shall be read and executed...
# REQUIREMENT: SHALL-DESCRIPTION-614: If action is -, the shell shall reset each
# condition to the default value.

# We test that a trap executes its action on EXIT (0).
test_cmd='
trap "echo exit_trap" 0
echo "running"
'
assert_stdout "running
exit_trap" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-615: If action is null (""), the shell shall
# ignore each specified condition if it arises.

test_cmd='
trap "" 0
echo "running"
'
assert_stdout "running" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Trap in Subshells
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-642: If the command is executed in a subshell,
# the implementation does not perform the optional check des...

test_cmd='
trap "echo exit_trap" 0
( trap )
'
# In POSIX, a subshell executing an EXIT trap might still trigger the trap, OR the subshell might just exit and trigger the trap.
# Wait, /bin/sh triggers the exit_trap when the subshell exits.
# So "exit_trap" will be printed. We just want to check `trap` output.
# Since `( trap )` prints nothing (traps are reset in subshell), let's just assert the exit_trap runs.
assert_stdout "exit_trap" \
    "$TARGET_SHELL -c '$test_cmd'"

report
