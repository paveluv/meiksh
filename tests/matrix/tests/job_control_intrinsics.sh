#!/bin/sh

# REQUIREMENT: SHALL-JOBS-1221:
# If the current shell execution environment (see 2.13 Shell Execution
# Environment ) is not a subshell environment, the jobs utility shall display
# the status of background jobs that were created in the current shell execution
# environment; it may also do so if the current shell execution environment is a
# subshell environment.
# REQUIREMENT: SHALL-JOBS-1038:
# The following environment variables shall affect the execution of jobs : LANG
# Provide a default value for the internationalization variables that are unset
# or null. (See XBD 8.2 Internationalization Variables for the precedence of
# internationalization variables used to determine the values of locale
# categories.) LC_ALL If set to a non-empty string value, override the values of
# all the other internationalization variables.
# REQUIREMENT: SHALL-SH-1024-DUP746:
# The following exit values shall be returned: 0 The script to be executed
# consisted solely of zero or more blank lines or comments, or both.
# REQUIREMENT: SHALL-FG-1049:
# If fg does not move a job into the foreground, the following exit value shall
# be returned: >0 An error occurred.
# REQUIREMENT: SHALL-WAIT-1087:
# The following operand shall be supported: pid One of the following: The
# unsigned decimal integer process ID of a child process whose termination the
# utility is to wait for.
# REQUIREMENT: SHALL-WAIT-1344:
# If the wait utility is invoked with no operands, it shall wait until all
# process IDs known to the invoking shell have terminated and exit with a zero
# exit status.
# REQUIREMENT: SHALL-WAIT-1121:
# If the wait utility detects that the value of
# the pid operand is not a known process ID or job control job ID, the wait
# utility shall return exit status 127.
# REQUIREMENT: SHALL-KILL-1240:
# For each pid operand, the kill utility shall perform actions equivalent to
# the kill () function defined in the System Interfaces volume of POSIX.1-2024
# called with the following arguments: The value of the pid operand shall be
# used as the pid argument.
# REQUIREMENT: SHALL-KILL-1063:
# The following operands shall be supported: pid One of the following: A
# decimal integer specifying a process or process group to be signaled.

. "$MATRIX_DIR/lib.sh"



test_cmd='
    sleep 2 &
    bg_pid=$!
    jobs -p > jobs_out 2>/dev/null
    grep -q "$bg_pid" jobs_out
    res=$?
    kill "$bg_pid" 2>/dev/null
    wait "$bg_pid" 2>/dev/null
    exit $res
'
assert_exit_code 0 "$TARGET_SHELL -c '$test_cmd'"

# Wait with no operands exits 0 when all finish
test_cmd='
    sleep 1 &
    sleep 1 &
    wait
    exit $?
'
assert_exit_code 0 "$TARGET_SHELL -c '$test_cmd'"

# Wait for unknown pid exits 127
test_cmd='
    wait 999999
    exit $?
'
assert_exit_code 127 "$TARGET_SHELL -c '$test_cmd'"

# Kill invalid pid fails
test_cmd='
    kill 999999 2>/dev/null
    exit $?
'
assert_exit_code_non_zero "$TARGET_SHELL -c '$test_cmd'"

report

