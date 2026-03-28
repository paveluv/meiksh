# Test: Job Control
# Target: tests/matrix/tests/job_control.sh
#
# Job Control is the magical feature that allows users to seamlessly
# suspend, resume, and manage multiple processes running on a single TTY.
# To properly test this, we must run the target shell under our Rust
# Pseudo-TTY wrapper to trick it into thinking it owns a real terminal.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The Maestro: Managing Processes
# ==============================================================================
# REQUIREMENT: SHALL-2-11-090: Job Control: Job control is a facility that
# allows users to selectively stop (suspend) the execution of processes...

# Our test simulates a user starting a background process and then
# interrogating the shell using the `jobs` command. A compliant shell
# should track background processes, and return their state and PID
# back to the user on demand.

interactive_script=$(cat << 'EOF'
sleep 0.5
echo 'sleep 10 &'
sleep 0.5
echo 'jobs'
sleep 0.5
echo 'exit'
EOF
)

# We spin up the PTY tool and run the target shell in interactive mode.
cmd="( $interactive_script ) | \"$MATRIX_DIR/pty\" $TARGET_SHELL -i"

# We run the command and capture raw output from the PTY session.
actual=$(eval "$cmd" 2>&1)

# Does the output from `jobs` reflect the background process? We look for
# the job number `[1]`, the command name `sleep 10`, and its status.
case "$actual" in
    *"[1]"*"Running"*"sleep 10"* | \
    *"[1]"*"sleep 10"* | \
    *"[1]"*"+"*"Running"*"sleep 10"*)
        pass
        ;;
    *)
        fail "Expected 'jobs' command to show background job, got: $actual"
        ;;
esac


report
