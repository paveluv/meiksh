# Test: Asynchronous Lists
# Target: tests/matrix/tests/asynchronous_lists.sh
#
# POSIX Shell allows executing commands in the background via the '&' operator.
# This suite verifies subshell execution, standard input redirection for
# background jobs, and process ID availability.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Background Execution and $!
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-1-320: If an AND-OR list is terminated by the control
# operator <ampersand> ( '&' ), the shell shall execute the AND-OR list
# asynchronously in a subshell environment.
# REQUIREMENT: SHALL-2-9-3-1-321: This subshell shall execute in the background;
# that is, the shell shall not wait for the subshell to terminate before
# executing the next command (if any); if there are no further commands to
# execute, the shell shall not wait for the subshell to terminate before
# exiting.
# REQUIREMENT: SHALL-2-9-3-1-322: If job control is enabled (see set , -m ), the
# AND-OR list shall become a job-control background job and a job number shall
# be assigned to it.
# REQUIREMENT: SHALL-2-9-3-1-323: If job control is disabled, the AND-OR list
# may become a non-job-control background job, in which case a job number shall
# be assigned to it; if no job number is assigned it shall become a background
# command but not a background job.
# REQUIREMENT: SHALL-2-9-3-1-324: The process ID associated with the
# asynchronous AND-OR list shall become known in the current shell execution
# environment; see 2.13 Shell Execution Environment .
# REQUIREMENT: SHALL-2-9-3-1-325: This process ID shall remain known until any
# one of the following occurs (and, unless otherwise specified, may continue to
# remain known after it occurs).
# REQUIREMENT: SHALL-2-9-3-1-328: If the shell is interactive and the
# asynchronous AND-OR list became a background job, the job number...
# REQUIREMENT: SHALL-2-9-3-1-329: If the shell is interactive and the
# asynchronous AND-OR list did not become a background job, the process ID
# associated with the asynchronous AND-OR list shall be written to standard
# error in an unspecified format.

# We test that `$!` contains the PID of the background job and `wait` can
# successfully wait for it.
test_cmd='
echo "bg_test" > tmp_bg.txt &
bg_pid=$!
wait $bg_pid
cat tmp_bg.txt
'
assert_stdout "bg_test" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing that the background job executes in a subshell and doesn't modify
# the parent environment.
test_cmd='
my_var="parent"
my_var="child" &
wait $!
echo "$my_var"
'
assert_stdout "parent" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Background Standard Input
# ==============================================================================
# REQUIREMENT: SHALL-2-9-3-1-326: If, and only if, job control is disabled, the
# standard input for the subshell in which an asynchronous AND-OR list is
# executed shall initially be assigned to an open file description that behaves
# as if /dev/null had been opened for reading only.
# REQUIREMENT: SHALL-2-9-3-1-327: This initial assignment shall be overridden by
# any explicit redirection of standard input within the AND-OR list.

# In a non-interactive shell (job control disabled), `cat &` should not hang
# waiting for stdin. Its stdin should be set to /dev/null, so it exits
# immediately.
test_cmd='
cat &
wait $!
echo "done"
'
assert_stdout "done" \
    "$TARGET_SHELL -c '$test_cmd'"

# However, an explicit redirection overrides /dev/null.
test_cmd='
echo "redirected" > tmp_in.txt
cat < tmp_in.txt &
wait $!
'
assert_stdout "redirected" \
    "$TARGET_SHELL -c '$test_cmd'"

report
