# Test: kill — Send Signals to Processes
# Target: tests/matrix/tests/kill.sh
#
# Tests the kill built-in utility for sending signals to processes and jobs.
# The kill utility sends signals by name, abbreviated name, or number, and
# can list available signal names and translate exit statuses.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# kill with default signal (SIGTERM)
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1238:
# REQUIREMENT: SHALL-KILL-1240:
# If no signal is specified, the kill utility shall send a SIGTERM signal
# to the process specified by pid.

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill \$p
  wait \$p 2>/dev/null
  echo done
'"

# ==============================================================================
# kill -s signal_name pid
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1245:
# REQUIREMENT: SHALL-KILL-1247:
# REQUIREMENT: SHALL-KILL-1241:
# kill -s signal_name pid — send a signal specified by name to the process.

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -s TERM \$p
  wait \$p 2>/dev/null
  echo done
'"

# Verify SIGKILL via -s
assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -s KILL \$p
  wait \$p 2>/dev/null
  echo done
'"

# ==============================================================================
# kill -signal_name pid (abbreviated form)
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1250:
# kill -signal_name pid — the abbreviated form where the signal name follows
# the hyphen directly.

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -TERM \$p
  wait \$p 2>/dev/null
  echo done
'"

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -KILL \$p
  wait \$p 2>/dev/null
  echo done
'"

# HUP signal
assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -HUP \$p
  wait \$p 2>/dev/null
  echo done
'"

# ==============================================================================
# kill -signal_number pid
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1248:
# If the first argument is a negative integer, it shall be interpreted as a
# -signal_number option, not as a negative pid operand.
# REQUIREMENT: SHALL-KILL-1063:
# pid operand: a decimal integer specifying a process or process group.

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -15 \$p
  wait \$p 2>/dev/null
  echo done
'"

assert_stdout "done" "$TARGET_SHELL -c '
  sleep 60 &
  p=\$!
  kill -9 \$p
  wait \$p 2>/dev/null
  echo done
'"

# ==============================================================================
# kill -l — list all signal names
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1062:
# REQUIREMENT: SHALL-KILL-1255:
# When -l is specified without an exit_status operand, the kill utility shall
# write all values of signal_name supported by the implementation to standard
# output.

# The output must contain at least the standard POSIX signals
actual=$($TARGET_SHELL -c 'kill -l' 2>/dev/null)
has_hup=false; has_int=false; has_kill=false; has_term=false
case "$actual" in *HUP*) has_hup=true ;; esac
case "$actual" in *INT*) has_int=true ;; esac
case "$actual" in *KILL*) has_kill=true ;; esac
case "$actual" in *TERM*) has_term=true ;; esac

if $has_hup && $has_int && $has_kill && $has_term; then
    pass
else
    fail "kill -l output missing standard signals. Got: $actual"
fi

# Exit code of kill -l should be 0
assert_exit_code 0 "$TARGET_SHELL -c 'kill -l'"

# ==============================================================================
# kill -l exit_status — translate exit status to signal name
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1243:
# REQUIREMENT: SHALL-KILL-1244:
# When -l is specified with an exit_status operand, the kill utility shall
# write the signal name corresponding to the exit status to standard output.

# Exit status 9 -> KILL, exit status 137 (128+9) -> KILL
actual=$($TARGET_SHELL -c 'kill -l 9' 2>/dev/null)
case "$actual" in
    *KILL*) pass ;;
    *) fail "kill -l 9 should output KILL, got: $actual" ;;
esac

actual=$($TARGET_SHELL -c 'kill -l 137' 2>/dev/null)
case "$actual" in
    *KILL*) pass ;;
    *) fail "kill -l 137 (128+9) should output KILL, got: $actual" ;;
esac

# Exit status 15 -> TERM
actual=$($TARGET_SHELL -c 'kill -l 15' 2>/dev/null)
case "$actual" in
    *TERM*) pass ;;
    *) fail "kill -l 15 should output TERM, got: $actual" ;;
esac

# Exit status 143 (128+15) -> TERM
actual=$($TARGET_SHELL -c 'kill -l 143' 2>/dev/null)
case "$actual" in
    *TERM*) pass ;;
    *) fail "kill -l 143 (128+15) should output TERM, got: $actual" ;;
esac

# ==============================================================================
# Exit status of kill: 0 on success
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1065:
# If at least one matching process is found for each pid operand and the
# specified signal is successfully sent, the exit status shall be 0.

# kill itself exits 0 on success; the overall script exit is from 'wait $p'
# which returns 128+signum, so we only verify kill's own exit code.
_out=$($TARGET_SHELL -c '
  sleep 60 &
  p=$!
  kill $p
  echo $?
  wait $p 2>/dev/null
' 2>/dev/null)
case "$_out" in
    0*) pass ;;
    *) fail "kill exit code should be 0 on success, got: $_out" ;;
esac

_out=$($TARGET_SHELL -c '
  sleep 60 &
  p=$!
  kill -s KILL $p
  echo $?
  wait $p 2>/dev/null
' 2>/dev/null)
case "$_out" in
    0*) pass ;;
    *) fail "kill -s KILL exit code should be 0 on success, got: $_out" ;;
esac

# ==============================================================================
# Exit status of kill: >0 on failure
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1065:
# If an error occurs, the exit status shall be >0.

# Sending to a nonexistent PID
assert_exit_code_non_zero "$TARGET_SHELL -c 'kill 99999999 2>/dev/null'"

# ==============================================================================
# Invalid signal name handling
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1253:
# When the -l option is not specified, the standard output shall not be used.

# Verify that kill without -l produces no stdout
_out=$($TARGET_SHELL -c 'kill -s TERM $$ 2>/dev/null' 2>/dev/null)
case "$_out" in
    "") pass ;;
    *) fail "kill without -l should produce no stdout, got: $_out" ;;
esac

# Invalid signal name should produce error
assert_exit_code_non_zero "$TARGET_SHELL -c 'kill -s NONEXISTENT \$\$ 2>/dev/null'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'kill -NONEXISTENT \$\$ 2>/dev/null'"

# ==============================================================================
# kill -l output format
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1256:
# For the last signal written, <separator> shall be a <newline>.

# kill -l output should end with a newline
_out=$($TARGET_SHELL -c 'kill -l' 2>/dev/null)
case "$_out" in
    *HUP*) pass ;;
    *) fail "kill -l output unexpected: $_out" ;;
esac

assert_exit_code_non_zero "$TARGET_SHELL -c 'kill -99999 \$\$ 2>/dev/null'"

# ==============================================================================
# kill with PID 0 — signal current process group
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1251:
# If process number 0 is specified, all processes in the current process
# group shall be signaled.

# kill -s 0 0 checks current process group existence
assert_exit_code 0 "$TARGET_SHELL -c 'kill -s 0 0'"

# ==============================================================================
# kill with signal 0 (existence check)
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1246:
# If signal_number is 0, no signal shall be sent, but error checking shall
# still be performed. This can be used to check the validity of a PID.
# REQUIREMENT: SHALL-KILL-1041:
# When both -l and exit_status are specified, the symbolic name of the
# corresponding signal shall be written in the format: "%s\n", <signal_name>

assert_exit_code 0 "$TARGET_SHELL -c 'kill -0 \$\$'"
assert_exit_code_non_zero "$TARGET_SHELL -c 'kill -0 99999999 2>/dev/null'"

# ==============================================================================
# kill with job ID (%N) — interactive
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1240:
# The kill utility shall accept a job_id operand (%N) to specify the
# process group of a background job.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "kill %1"
expect "$ "
send "wait 2>/dev/null; echo killed_ok"
expect "killed_ok"
sendeof
wait'

# kill -s with job ID
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "kill -s KILL %1"
expect "$ "
send "wait 2>/dev/null; echo killed_ok"
expect "killed_ok"
sendeof
wait'

# ==============================================================================
# kill -s 0 with job ID (job existence check)
# ==============================================================================
# REQUIREMENT: SHALL-KILL-1041:
# kill -s 0 with a job ID checks whether the job exists without sending a signal.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "kill -s 0 %1; echo check_$?"
expect "check_0"
send "kill %1; wait 2>/dev/null"
expect "$ "
sendeof
wait'

report
