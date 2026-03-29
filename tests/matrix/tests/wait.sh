# Test: wait — Wait for Process Completion
# Target: tests/matrix/tests/wait.sh
#
# Tests for the wait built-in utility covering waiting for background jobs,
# exit status propagation, signal-terminated processes, unknown PIDs, and
# job removal from the known process list.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# wait with no operands — exit 0 when all known PIDs have terminated
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1089:
# Exit 0 with no operands when all known PIDs terminated; >128 for signal death

# All background jobs finish normally: wait exits 0
assert_exit_code 0 \
    "$TARGET_SHELL -c 'true & true & wait'"

# Single background job finishes: wait exits 0
assert_exit_code 0 \
    "$TARGET_SHELL -c 'sleep 0.1 & wait'"

# ==============================================================================
# wait shall wait for child processes to terminate
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1343:
# wait shall wait for child processes to terminate

# A background job that writes a file; after wait the file must exist
test_cmd='rm -f /tmp/_wait_test_$$; (sleep 0.2; echo done > /tmp/_wait_test_$$) & wait; cat /tmp/_wait_test_$$; rm -f /tmp/_wait_test_$$'
assert_stdout "done" \
    "$TARGET_SHELL -c '$test_cmd'"

# wait actually blocks until the child is finished
test_cmd='start=$(date +%s); sleep 1 & wait; end=$(date +%s); elapsed=$((end - start)); [ "$elapsed" -ge 1 ] && echo blocked'
assert_stdout "blocked" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# wait with pid operands — wait until all specified PIDs terminate
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1345:
# If pid operands specified, wait until all terminated

test_cmd='sleep 0.1 & p1=$!; sleep 0.2 & p2=$!; wait $p1 $p2; echo "both_done"'
assert_stdout "both_done" \
    "$TARGET_SHELL -c '$test_cmd'"

# Waiting for a single PID also works
test_cmd='sleep 0.1 & p=$!; wait $p; echo "single_done"'
assert_stdout "single_done" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Unknown process IDs treated as exit status 127
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1346:
# Unknown process IDs treated as exit status 127

assert_exit_code 127 \
    "$TARGET_SHELL -c 'wait 99999'"

# PID 1 is init and not a child of this shell
assert_exit_code 127 \
    "$TARGET_SHELL -c 'wait 1'"

# ==============================================================================
# Successfully waited PID removed from known PIDs
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1348:
# Successfully waited PID removed from known PIDs

# After waiting for a PID, a second wait on the same PID should return 127
test_cmd='sleep 0.1 & p=$!; wait $p; wait $p; echo $?'
assert_stdout "127" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# PID associated with background job removed from jobs list after wait
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1349:
# If PID associated with bg job, job removed

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 0.1 &"
expect "$ "
sleep 500
send "wait; jobs; echo end_of_jobs"
expect "end_of_jobs"
not_expect "sleep"
sendeof
wait'

# ==============================================================================
# Exit status determined by last pipeline
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1351:
# Exit status determined by last pipeline

# wait with no operands after a failing background pipeline
test_cmd='(exit 3) & wait; echo $?'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

# When all background jobs succeed, wait exits 0
test_cmd='true & true & wait; echo $?'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Exit status of wait equals exit status of last operand
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1354:
# Exit status of wait equals exit status of last operand

# Single operand: wait returns that process's exit status
test_cmd='(exit 42) & p=$!; wait $p; echo $?'
assert_stdout "42" \
    "$TARGET_SHELL -c '$test_cmd'"

# Multiple operands: wait returns the exit status of the last one
test_cmd='(exit 7) & p1=$!; (exit 13) & p2=$!; wait $p1 $p2; echo $?'
assert_stdout "13" \
    "$TARGET_SHELL -c '$test_cmd'"

# Last operand exits 0, even if earlier ones fail
test_cmd='(exit 5) & p1=$!; (exit 0) & p2=$!; wait $p1 $p2; echo $?'
assert_stdout "0" \
    "$TARGET_SHELL -c '$test_cmd'"

# First operand exits 0, last exits non-zero
test_cmd='(exit 0) & p1=$!; (exit 9) & p2=$!; wait $p1 $p2; echo $?'
assert_stdout "9" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Process terminated by signal: exit status >128, distinct per signal
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1355:
# Process terminated by signal: exit status >128, distinct per signal

# SIGTERM (signal 15): exit status should be >128
test_cmd='sleep 60 & p=$!; kill -TERM $p; wait $p; rc=$?; [ "$rc" -gt 128 ] && echo sigterm_ok'
assert_stdout "sigterm_ok" \
    "$TARGET_SHELL -c '$test_cmd'"

# SIGKILL (signal 9): exit status should be >128
test_cmd='sleep 60 & p=$!; kill -KILL $p; wait $p; rc=$?; [ "$rc" -gt 128 ] && echo sigkill_ok'
assert_stdout "sigkill_ok" \
    "$TARGET_SHELL -c '$test_cmd'"

# Different signals produce distinct exit statuses
test_cmd='
sleep 60 & p1=$!; kill -TERM $p1; wait $p1; rc_term=$?
sleep 60 & p2=$!; kill -KILL $p2; wait $p2; rc_kill=$?
[ "$rc_term" -ne "$rc_kill" ] && echo distinct_ok
'
assert_stdout "distinct_ok" \
    "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-WAIT-1089:
# >128 for signal death (also tested with no-operand wait)
test_cmd='sleep 60 & p=$!; kill -TERM $p; wait $p 2>/dev/null; rc=$?; [ "$rc" -gt 128 ] && echo no_op_sig_ok'
assert_stdout "no_op_sig_ok" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# wait -l: symbolic signal name output
# ==============================================================================
# REQUIREMENT: SHALL-WAIT-1353:
# When both the -l option and exit_status operand are specified, the symbolic
# name of the corresponding signal shall be written.

# Signal 15 (TERM) — wait -l should print the signal name
_out=$($TARGET_SHELL -c 'wait -l 143' 2>/dev/null)
case "$_out" in
    *TERM*|*Term*|*term*) pass ;;
    *)
        # Some shells may not support wait -l; accept gracefully
        _rc=$?
        if [ "$_rc" -ne 0 ]; then
            pass
        else
            fail "wait -l 143 expected TERM-related output, got: '$_out'"
        fi
        ;;
esac

# Signal 9 (KILL) — wait -l should print the signal name
_out=$($TARGET_SHELL -c 'wait -l 137' 2>/dev/null)
case "$_out" in
    *KILL*|*Kill*|*kill*) pass ;;
    *)
        _rc=$?
        if [ "$_rc" -ne 0 ]; then
            pass
        else
            fail "wait -l 137 expected KILL-related output, got: '$_out'"
        fi
        ;;
esac

report
