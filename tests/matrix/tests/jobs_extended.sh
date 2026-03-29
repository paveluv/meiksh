# Test: jobs (extended) — Advanced Job Status Reporting
# Target: tests/matrix/tests/jobs_extended.sh
#
# Extended tests for the jobs built-in utility covering long format output,
# PID-only mode, state display, format verification, job_id operands, and
# default behavior with no arguments.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# jobs -l — long format with PIDs
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1057:
# REQUIREMENT: SHALL-JOBS-1235:
# REQUIREMENT: SHALL-JOBS-1223:
# The jobs -l option shall provide a long listing, including the process
# group ID of each job in addition to the normal information.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "jobs -l"
expect "[1]"
expect "sleep 60"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# The long format must show a numeric PID
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "jobs -l | grep -E \"[0-9]+\" && echo pid_ok"
expect "pid_ok"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs -p — PIDs only
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1060:
# REQUIREMENT: SHALL-JOBS-1221:
# The -p option shall cause jobs to display only the process group leaders'
# process IDs, one per line.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "JPID=$(jobs -p); echo pid_is_$JPID"
expect "pid_is_"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# jobs -p output should be a numeric PID
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "jobs -p | grep -qE \"^[0-9]+$\" && echo numeric_ok"
expect "numeric_ok"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# Job state display: Running
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1039:
# A job that is executing shall be reported with the state "Running".

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "jobs"
expect "Running"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# Job state display: Done
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1222:
# REQUIREMENT: SHALL-JOBS-1039:
# When a background job completes, the shell shall report its status as "Done"
# the next time the user requests job information.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 0.1 &"
sleep 500
send "jobs"
expect "Done"
expect "$ "
sendeof
wait'

# ==============================================================================
# Job state display: Stopped
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1232:
# A job that has been suspended shall be reported with the state "Stopped".

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60"
sleep 500
sendraw 1a
expect "Stopped"
send "jobs"
expect "Stopped"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# Output format: job number, status, command
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1039:
# The default output format shall include: [job_number] current status command

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "jobs"
expect "[1]"
expect "Running"
expect "sleep 60"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs with job_id operand
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1058:
# REQUIREMENT: SHALL-JOBS-1061:
# If job_id is given, the output shall be information about that job only.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "sleep 61 &"
expect "$ "
send "jobs %1"
expect "sleep 60"
not_expect "sleep 61"
send "kill %1 %2; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs with no args lists all jobs
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1225:
# REQUIREMENT: SHALL-JOBS-1227:
# REQUIREMENT: SHALL-JOBS-1221:
# If no job_id operands are given, all current jobs shall be displayed.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "sleep 61 &"
expect "$ "
send "jobs"
expect "sleep 60"
expect "sleep 61"
send "kill %1 %2; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs -l with stopped job shows PID and Stopped state
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1236:
# jobs -l on a stopped job shall show the PID and the Stopped state.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60"
sleep 500
sendraw 1a
expect "Stopped"
send "jobs -l"
expect "Stopped"
expect "sleep 60"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs: current job marker (+)
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1233:
# The current job (most recently suspended or backgrounded) shall be indicated
# with a '+' marker.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "sleep 61 &"
expect "$ "
send "jobs"
expect "+"
send "kill %1 %2; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs: previous job marker (-)
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1234:
# The previous job shall be indicated with a '-' marker.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "sleep 61 &"
expect "$ "
send "jobs"
expect "-"
send "kill %1 %2; wait 2>/dev/null"
expect "$ "
sendeof
wait'

# ==============================================================================
# jobs with no background jobs produces no output
# ==============================================================================
# REQUIREMENT: SHALL-JOBS-1061:
# If there are no current jobs, jobs shall produce no output.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "jobs; echo end_of_jobs"
expect "end_of_jobs"
not_expect "Running"
not_expect "Stopped"
sendeof
wait'

report
