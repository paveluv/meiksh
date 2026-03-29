# Test: bg/fg — Interactive Job Control Utilities
# Target: tests/matrix/tests/bg_fg.sh
#
# Tests the bg and fg built-in utilities using the expect_pty scriptable PTY
# driver. These require a real controlling terminal with job control enabled.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# bg: Resume a suspended job in the background
# ==============================================================================
# REQUIREMENT: SHALL-BG-1062:
# If job control is enabled, the bg utility shall resume the execution of a
# currently suspended job in the background.
# REQUIREMENT: SHALL-BG-1031:
# The output of bg shall consist of a line in the format:
# "[%d] %s\n", <job-number>, <command>
# REQUIREMENT: SHALL-BG-1065:
# If no job_id operand is given, the most recently suspended job shall be used.
# REQUIREMENT: SHALL-JOBS-1237:
# The implementation may substitute the string Suspended in place of Stopped.

# Start a sleep, suspend it with Ctrl-Z, then bg it
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60"
sleep 500
sendraw 1a
expect_glob "{Stopped,Suspended}"
send "bg"
expect "sleep 60"
expect "$ "
send "kill %1"
expect "$ "
sendeof
wait'

# ==============================================================================
# bg: Job already running in background has no effect
# ==============================================================================
# REQUIREMENT: SHALL-BG-1063:
# If the job specified by job_id is already a running background job, the bg
# utility shall have no effect and shall exit successfully.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "bg %1; echo bg_exit_$?"
expect "bg_exit_0"
send "kill %1 2>/dev/null; wait"
expect "$ "
sendeof
wait'

# ==============================================================================
# bg: With explicit job_id operand
# ==============================================================================
# REQUIREMENT: SHALL-BG-1029:
# The following operand shall be supported: job_id

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60"
sleep 500
sendraw 1a
expect_glob "{Stopped,Suspended}"
send "bg %1"
expect "sleep 60"
expect "$ "
send "kill %1 2>/dev/null; wait; true"
expect "$ "
sendeof
wait'

# ==============================================================================
# fg: Bring a background job to the foreground
# ==============================================================================
# REQUIREMENT: SHALL-FG-1166:
# If job control is enabled, the fg utility shall move a background job into
# the foreground.
# REQUIREMENT: SHALL-FG-1035:
# The fg utility shall write the command line of the job to standard output
# in the following format: "%s\n", <command>
# REQUIREMENT: SHALL-STDERR-627:
# The fg utility shall write the command line of the job to standard output.
# REQUIREMENT: SHALL-FG-1169:
# If no job_id operand is given, the job most recently suspended, placed in
# the background, or run as a background job shall be used.

# Start a background job, fg it, then Ctrl-C to terminate
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "fg"
expect "sleep 60"
sleep 200
sendraw 03
expect "$ "
sendeof
wait'

# ==============================================================================
# fg: With explicit job_id
# ==============================================================================
# REQUIREMENT: SHALL-FG-1047:
# The following operand shall be supported: job_id

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "fg %1"
expect "sleep 60"
sleep 200
sendraw 03
expect "$ "
sendeof
wait'

# ==============================================================================
# fg: Removes job from known process list
# ==============================================================================
# REQUIREMENT: SHALL-FG-1167:
# Using fg to place a job into the foreground shall remove its process ID
# from the list of those known in the current shell execution environment.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "fg"
expect "sleep 60"
sleep 200
sendraw 03
expect "$ "
send "jobs"
expect "$ "
not_expect "sleep 60"
sendeof
wait'

# ==============================================================================
# Suspend with Ctrl-Z and resume cycle
# ==============================================================================
# Tests the full suspend -> bg -> fg cycle

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60"
sleep 500
sendraw 1a
expect_glob "{Stopped,Suspended}"
send "bg"
expect "sleep 60"
expect "$ "
send "fg"
expect "sleep 60"
sleep 200
sendraw 03
expect "$ "
sendeof
wait'

# ==============================================================================
# bg: Error when job control is disabled
# ==============================================================================
# REQUIREMENT: SHALL-BG-1069:
# If job control is disabled, the bg utility shall exit with an error.
# REQUIREMENT: SHALL-FG-1173:
# If job control is disabled, the fg utility shall exit with an error.

# Run in non-interactive mode where job control is off by default
assert_exit_code_non_zero "$TARGET_SHELL -c 'bg 2>/dev/null'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'fg 2>/dev/null'"

report
