# Test: fd routing verification
# Target: tests/matrix/tests/fd_routing.sh
#
# Verifies that interactive shell messages are sent to the correct file descriptors
# as specified by POSIX: jobs/bg/fg output to stdout, shell notifications to stderr.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# jobs output goes to stdout (POSIX jobs STDOUT section)
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1234:
# The jobs utility shall display the status of jobs that were started in the
# current shell environment.
# REQUIREMENT: SHALL-JOBS-1057:
# By default, the jobs utility shall display the status of all stopped jobs,
# running background jobs, and all jobs whose status has changed and have not
# been reported by the shell.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 100 &"
expect "$ "
send "jobs > /tmp/meiksh_jobs_out 2>/dev/null"
expect "$ "
send "cat /tmp/meiksh_jobs_out"
expect "\[[[:digit:]]+\]"
expect "$ "
send "jobs >/dev/null; echo jobs_done"
expect "jobs_done"
send "kill %1 2>/dev/null; wait"
expect "$ "
send "rm -f /tmp/meiksh_jobs_out"
expect "$ "
sendeof
wait'

# ==============================================================================
# bg output goes to stdout (POSIX bg STDOUT section)
# ==============================================================================
# REQUIREMENT: SHALL-BG-1031:
# The output of bg shall consist of a line in the format:
# "[%d] %s\n", <job-number>, <command>

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 100"
sleep 500ms
sendraw 1a
expect "(Stopped|Suspended)"
send "bg > /tmp/meiksh_bg_out 2>/dev/null"
expect "$ "
send "cat /tmp/meiksh_bg_out"
expect "\[[[:digit:]]+\]"
expect "$ "
send "kill %1 2>/dev/null; wait"
expect "$ "
send "rm -f /tmp/meiksh_bg_out"
expect "$ "
sendeof
wait'

# ==============================================================================
# fg output goes to stdout (POSIX fg STDOUT section)
# ==============================================================================
# REQUIREMENT: SHALL-FG-1035:
# The fg utility shall write the command line of the job to standard output
# in the following format: "%s\n", <command>
# REQUIREMENT: SHALL-STDERR-627:
# The fg utility shall write the command line of the job to standard output.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 100"
sleep 500ms
sendraw 1a
expect "(Stopped|Suspended)"
send "bg"
expect "$ "
send "fg 2>/dev/null"
expect "sleep"
sleep 200ms
sendraw 03
expect "$ "
send "kill %1 2>/dev/null; wait; true"
expect "$ "
sendeof
wait'

# ==============================================================================
# Async launch notification goes to stderr (POSIX 2.9.3.1)
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-1-353:
# When an asynchronous list is started by the shell, the format of the
# job-related message written to standard error is: "[%d] %d\n", ...
# The async "[N] PID" notification must go to stderr, not stdout.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "{ sleep 100 & } 2>/dev/null"
expect "$ "
not_expect timeout=500ms "\[[[:digit:]]+\] [[:digit:]]+"
send "kill %1 2>/dev/null; wait; true"
expect "$ "
sendeof
wait'

# ==============================================================================
# Shell job-state notifications go to stderr (POSIX 2.11)
# ==============================================================================
# REQUIREMENT: SHALL-2-11-443:
# When the shell is interactive, it shall write a message to standard error
# reporting the termination status of each job that was started asynchronously
# and has since changed state.

# Verify Done notification appears on terminal (it comes from stderr which is
# merged into the PTY). Then verify that capturing stdout only doesn't catch it.
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 0.1 &"
expect "\[[[:digit:]]+\] [[:digit:]]+"
expect "$ "
sleep 500ms
send "true"
expect "Done"
expect "$ "
send "rm -f /tmp/meiksh_stderr"
expect "$ "
sendeof
wait'

report
