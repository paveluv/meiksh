# Test: Exit Status and Error Consequences
# Target: tests/matrix/tests/exit_status.sh
#
# POSIX Shell uses the exit status of commands to control execution flow. This
# suite verifies various required exit statuses (like 127 for not found, 126
# for not executable, and proper propagation of $?).

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Command Search Exit Statuses
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-4-296:
# If the search is unsuccessful, the command shall fail with an exit status of
# 127 and the shell shall write an error message.
# REQUIREMENT: SHALL-2-9-1-6-310:
# If the search is unsuccessful, the command
# shall fail with an exit status of 127 and the shell shall write an error...
# REQUIREMENT: SHALL-2-8-2-259:
# The exit status of a command shall be determined as follows: If the command
# is not found, the exit status shall be 127.

# Executing a completely non-existent command returns 127.
test_cmd='this_command_does_not_exist_xyz123'
assert_stdout "127" \
    "$TARGET_SHELL -c '$test_cmd'; echo \"\$?\""

# REQUIREMENT: SHALL-2-9-1-6-304:
# If the execl () function fails due to an error equivalent to the [ENOEXEC]
# error defined in the System Interfaces volume of POSIX.1-2024, the shell shall
# execute a command equivalent to having a shell invoked with the pathname
# resulting from the search as its first operand, with any remaining arguments
# passed to the new shell, except that the value of "$0" in the new shell may be
# set to the command name.
# REQUIREMENT: SHALL-2-9-1-6-305:
# In this case, it shall write an error message, and the command shall fail
# with an exit status of 126.
# REQUIREMENT: SHALL-2-8-2-260:
# Otherwise, if the command name is found, but it is not an executable utility,
# the exit status shall be 126.

# Executing a file that exists but lacks execute permissions or isn't a valid
# executable format returns 126.
touch tmp_not_exec
chmod -x tmp_not_exec
test_cmd='./tmp_not_exec'
assert_stdout "126" \
    "$TARGET_SHELL -c '$test_cmd'; echo \"\$?\""

# ==============================================================================
# Built-in Utility Exit Statuses
# ==============================================================================
# REQUIREMENT: SHALL-EXIT-STATUS-537:
# If there are no argument s, or only null argument s, eval shall return a zero
# exit status; otherwise, it shall return the exit status of the command defined
# by the string of concatenated argument s separated by <space> characters, or a
# non-zero exit status if the concatenation could not be parsed as a command and
# the shell is interactive (and therefore did not abort).

test_cmd='eval'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

test_cmd='eval ""'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-EXIT-STATUS-548:
# Otherwise, exec shall return a zero exit status.

# `exec` with no utility argument (just redirections or nothing) returns 0.
test_cmd='exec'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-EXIT-STATUS-578:
# The exit status shall be n , if specified, except that the behavior is
# unspecified if n is not an unsigned decimal integer or is greater than 255.
# REQUIREMENT: SHALL-EXIT-STATUS-579:
# If n is not specified, the result shall be as if n were specified with the
# current value of the special parameter '?' (see 2.5.2 Special Parameters ),
# except that if the return command would cause the end of execution of a trap
# action, the value for the special parameter '?' that is considered "current"
# shall be the value it had immediately preceding the trap action.

test_cmd='false; return 0 2>/dev/null'
# Outside of a function or dot script, behavior is mostly unspecified, but
# `return`
# takes the exit status of the previous command if n is not specified. Wait, we
# must test inside a function for `return`.

test_cmd='myfunc() { false; return; }; myfunc; echo "$?"'
assert_stdout "1" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-DESCRIPTION-576:
# The return utility shall cause the shell to stop executing the current
# function or dot script.
test_cmd='myfunc() { return 5; }; myfunc; echo "$?"'
assert_stdout "5" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-EXIT-STATUS-623:
# If the n operand is invalid or is greater than "$#" , this may be treated as
# an error and a non-interactive shell may exit; if the shell does not exit in
# this case, a non-zero exit status shall be returned and a warning message
# shall be written to standard error.
# REQUIREMENT: SHALL-EXIT-STATUS-624:
# Otherwise, zero shall be returned.

# `shift` with valid bounds returns 0.
test_cmd='shift 1; echo "$?"'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd' sh arg1"

# ==============================================================================
# The 'sh' Utility Exit Status
# ==============================================================================
# REQUIREMENT: SHALL-EXIT-STATUS-623:
# The following exit values shall be
# returned...
# REQUIREMENT: SHALL-EXIT-STATUS-143:
# Otherwise, the shell shall terminate in the same manner as for an exit
# command with no operands, unless the last command the shell invoked was
# executed without forking, in which case the wait status seen by the parent
# process of the shell shall be the wait status of the last command the shell
# invoked.

# A shell script that just runs a command that fails inherits that exit status.
echo "exit 42" > tmp_script.sh
assert_stdout "42" \
    "$TARGET_SHELL tmp_script.sh; echo \"\$?\""


# ==============================================================================
# Signals and Exit Status
# ==============================================================================
# REQUIREMENT: SHALL-2-8-2-261:
# Otherwise, if the command terminated due to the receipt of a signal, the
# shell shall assign it an exit status greater than 128.
# REQUIREMENT: SHALL-2-8-2-262:
# The exit status shall identify, in an implementation-defined manner, which
# signal terminated the command.

# If we send a SIGKILL (9) to a background sleep, wait should return 128 + 9
# (137 on most systems, but strictly > 128).
test_cmd='
sleep 10 &
pid=$!
kill -9 $pid >/dev/null 2>&1
wait $pid
status=$?
[ $status -gt 128 ] && echo "signal_exit"
'
assert_stdout "signal_exit" \
    "$TARGET_SHELL -c '$test_cmd'"

report
