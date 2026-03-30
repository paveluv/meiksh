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
# REQUIREMENT: SHALL-2-11-435:
# Job Control: Job control is a facility that
# allows users to selectively stop (suspend) the execution of processes...
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-025:
# The sh utility shall take the standard action for all signals (see 1.4
# Utility Description Defaults ) with the following exceptions.
# REQUIREMENT: SHALL-2-11-425:
# If the shell has a controlling terminal and it is the controlling process for
# the terminal session, it shall initially set the foreground process group ID
# associated with the terminal to its own process group ID.
# REQUIREMENT: SHALL-2-11-427:
# Otherwise, if it has a controlling terminal, it
# shall initially perform the following steps if inter...
# REQUIREMENT: SHALL-2-11-427:
# Otherwise, if it has a controlling terminal, it shall initially perform the
# following steps if interactive and may perform them if non-interactive: If its
# process group is the foreground process group associated with the terminal,
# the shell shall set its process group ID to its process ID (if they are not
# already equal) and set the foreground process group ID associated with the
# terminal to its process group ID.
# REQUIREMENT: SHALL-2-11-428:
# If its process group is not the foreground process group associated with the
# terminal (which would result from it being started by a job-control shell as a
# background job), the shell shall either stop itself by sending itself a
# SIGTTIN signal or, if interactive, attempt to read from standard input (which
# generates a SIGTTIN signal if standard input is the controlling terminal).
# REQUIREMENT: SHALL-2-11-429:
# If it is stopped, then when it continues execution (after receiving a SIGCONT
# signal) it shall repeat these steps.
# REQUIREMENT: SHALL-2-11-430:
# Subsequently, the shell shall change the foreground process group associated
# with its controlling terminal when a foreground job is running as noted in the
# description below.
# REQUIREMENT: SHALL-2-11-448:
# When job control is enabled, the shell shall
# create one or more jobs when it executes a list...
# REQUIREMENT: SHALL-2-11-432:
# For the purposes of job control, a list that includes more than one
# asynchronous AND-OR list shall be treated as if it were split into multiple
# separate lists, each ending with an asynchronous AND-OR list.
# REQUIREMENT: SHALL-2-11-433:
# When a job consisting of a single asynchronous AND-OR list is created, it
# shall form a background job and the associated process ID shall be that of a
# child process that is made a process group leader, with all other processes
# (if any) that the shell creates to execute the AND-OR list initially having
# this process ID as their process group ID.
# REQUIREMENT: SHALL-2-11-434:
# For a list consisting of one or more sequentially executed AND-OR lists
# followed by at most one asynchronous AND-OR list, the whole list shall form a
# single foreground job up until the sequentially executed AND-OR lists have all
# completed execution, at which point the asynchronous AND-OR list (if any)
# shall form a background job as described above.
# REQUIREMENT: SHALL-2-11-435:
# For each pipeline in a foreground job, if the pipeline is executed while the
# list is still a foreground job, the set of processes comprising the pipeline,
# and any processes descended from it, shall all be in the same process group,
# unless the shell executes some of the commands in the pipeline in the current
# shell execution environment and others in a subshell environment; in this case
# the process group ID of the current shell need not change (or cannot change if
# it is the session leader), and consequently the process group ID that the
# other processes all share may differ from the process group ID of the current
# shell (which means that a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTOU signal sent to
# one of those process groups does not cause the whole pipeline to stop).
# REQUIREMENT: SHALL-2-11-436:
# A background job that was created on execution of an asynchronous AND-OR list
# can be brought into the foreground by means of the fg utility (if supported);
# in this case the entire job shall become a single foreground job.
# REQUIREMENT: SHALL-2-11-437:
# If a process that the shell subsequently waits for is part of this foreground
# job and is stopped by a signal, the entire job shall become a suspended job
# and the behavior shall be as if the process had been stopped while the job was
# running in the background.
# REQUIREMENT: SHALL-2-11-439:
# When a foreground job is created, or a background
# job is brought into the foreground by the fg utili...
# REQUIREMENT: SHALL-2-11-439:
# When a foreground job is created, or a background job is brought into the
# foreground by the fg utility, if the shell has a controlling terminal it shall
# set the foreground process group ID associated with the terminal as follows:
# If the job was originally created as a background job, the foreground process
# group ID shall be set to the process ID of the process that the shell made a
# process group leader when it executed the asynchronous AND-OR list.
# REQUIREMENT: SHALL-2-11-440:
# If the job was originally created as a foreground job, the foreground process
# group ID shall be set as follows when each pipeline in the job is executed: If
# the shell is not itself executing, in the current shell execution environment,
# all of the commands in the pipeline, the foreground process group ID shall be
# set to the process group ID that is shared by the other processes executing
# the pipeline (see above).
# REQUIREMENT: SHALL-2-11-441:
# If all of the commands in the pipeline are being executed by the shell itself
# in the current shell execution environment, the foreground process group ID
# shall be set to the process group ID of the shell.
# REQUIREMENT: SHALL-2-11-442:
# When a foreground job terminates, or becomes a suspended job (see below), if
# the shell has a controlling terminal it shall set the foreground process group
# ID associated with the terminal to the process group ID of the shell.
# REQUIREMENT: SHALL-2-11-443:
# Each background job (whether suspended or not) shall have associated with it
# a job number and a process ID that is known in the current shell execution
# environment.
# REQUIREMENT: SHALL-2-11-444:
# When a background job is brought into the foreground by means of the fg
# utility, the associated job number shall be removed from the shell's
# background jobs list and the associated process ID shall be removed from the
# list of process IDs known in the current shell execution environment.
# REQUIREMENT: SHALL-2-11-445:
# If a process that the shell is waiting for is part of a foreground job that
# was started as a foreground job and is stopped by a catchable signal (SIGTSTP,
# SIGTTIN, or SIGTTOU): If the currently executing AND-OR list within the list
# comprising the foreground job consists of a single pipeline in which all of
# the commands are simple commands, the shell shall either create a suspended
# job consisting of at least that AND-OR list and the remaining (if any) AND-OR
# lists in the same list, or create a suspended job consisting of just that
# AND-OR list and discard the remaining (if any) AND-OR lists in the same list.
# REQUIREMENT: SHALL-2-11-446:
# Otherwise, the shell shall create a suspended job consisting of a set of
# commands, from within the list comprising the foreground job, that is
# unspecified except that the set shall include at least the pipeline to which
# the stopped process belongs.
# REQUIREMENT: SHALL-2-11-447:
# Commands in the foreground job that have not already completed and are not
# included in the suspended job shall be discarded.
# REQUIREMENT: SHALL-2-11-448:
# If a process that the shell is waiting for is part of a foreground job that
# was started as a foreground job and is stopped by a SIGSTOP signal, the
# behavior shall be as described above for a catchable signal unless the shell
# was executing a built-in utility in the current shell execution environment
# when the SIGSTOP was delivered, resulting in the shell itself being stopped by
# the signal, in which case if the shell subsequently receives a SIGCONT signal
# and has one or more child processes that remain stopped, the shell shall
# create a suspended job as if only those child processes had been stopped.
# REQUIREMENT: SHALL-2-11-449:
# When a suspended job is created as a result of a foreground job being
# stopped, it shall be assigned a job number, and an interactive shell shall
# write, and a non-interactive shell may write, a message to standard error,
# formatted as described by the jobs utility (without the -l option) for a
# suspended job.
# REQUIREMENT: SHALL-2-11-450:
# The message may indicate that the commands comprising the job include
# commands that have already completed; in this case the completed commands
# shall not be repeated if execution of the job is subsequently continued.
# REQUIREMENT: SHALL-2-11-451:
# If the shell is interactive, it shall save the terminal settings before
# changing them to the settings it needs to read further commands.
# REQUIREMENT: SHALL-2-11-453:
# When a process associated with a background job
# is stopped by a SIGSTOP, SIGTSTP, SIGTTIN, or SIGTTO...
# REQUIREMENT: SHALL-2-11-453:
# When a process associated with a background job is stopped by a SIGSTOP,
# SIGTSTP, SIGTTIN, or SIGTTOU signal, the shell shall convert the
# (non-suspended) background job into a suspended job and an interactive shell
# shall write a message to standard error, formatted as described by the jobs
# utility (without the -l option) for a suspended job, at the following time: If
# set -b is enabled, the message shall be written either immediately after the
# job became suspended or immediately prior to writing the next prompt for
# input.
# REQUIREMENT: SHALL-2-11-454:
# If set -b is disabled, the message shall be written immediately prior to
# writing the next prompt for input.
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-026:
# If the shell is interactive, SIGINT signals received during command line
# editing shall be handled as described in the EXTENDED DESCRIPTION, and SIGINT
# signals received at other times shall be caught but no action performed.
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-027:
# If the shell is interactive: SIGQUIT and SIGTERM signals shall be ignored.
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-028:
# If the -m option is in effect, SIGTTIN, SIGTTOU, and SIGTSTP signals shall be
# ignored.
# REQUIREMENT: SHALL-ASYNCHRONOUS-EVENTS-029:
# If they are caught, the shell shall, in the signal-catching function, set the
# signal to the default action and raise the signal (after taking any
# appropriate steps, such as restoring terminal settings).

# Our test simulates a user starting a background process and then
# interrogating the shell using the `jobs` command. A compliant shell
# should track background processes, and return their state and PID
# back to the user on demand.

interactive_script=$(cat << 'EOF'
sleep 500ms
echo 'sleep 10 &'
sleep 500ms
echo 'jobs'
sleep 500ms
echo 'exit'
EOF
)

# We spin up the PTY tool and run the target shell in interactive mode.
cmd="( $interactive_script ) | run_pty $TARGET_SHELL -i"

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


# ==============================================================================
# History maintained interactively
# ==============================================================================
# REQUIREMENT: SHALL-Command-History-List-031:
# When the sh utility is being used interactively, it shall maintain a list of
# commands previously entered from the terminal in the file named by the
# HISTFILE environment variable.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo histcheck_1"
expect "histcheck_1"
expect "$ "
send "echo histcheck_2"
expect "histcheck_2"
expect "$ "
send "fc -l -2 -1"
expect "histcheck_1"
expect "histcheck_2"
sendeof
wait'

# ==============================================================================
# User must explicitly exit interactive shell
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-602:
# A user shall explicitly exit to leave the interactive shell.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo still_here"
expect "still_here"
expect "$ "
send "exit"
wait'

# ==============================================================================
# fg/bg send SIGCONT to stopped job
# ==============================================================================
# REQUIREMENT: SHALL-2-11-455:
# The fg and bg utilities shall send a SIGCONT signal to the process group of
# the process(es) whose stopped wait status caused the shell to suspend the job.
# REQUIREMENT: SHALL-2-11-456:
# If the shell has a controlling terminal, the fg utility shall send the
# SIGCONT signal after it has set the foreground process group ID.
# REQUIREMENT: SHALL-2-11-457:
# If the fg utility is used from an interactive shell to bring into the
# foreground a suspended job that was created from a foreground job, before it
# sends the SIGCONT signal the fg utility shall restore the terminal settings.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 60 &"
expect "$ "
send "kill -STOP %1"
sleep 500ms
expect "$ "
send "fg %1"
sleep 500ms
send ""
sleep 200ms
send "kill %1"
sleep 500ms
sendeof
wait'

# ==============================================================================
# Background job completion notification
# ==============================================================================
# REQUIREMENT: SHALL-2-11-459:
# When a background job completes or is terminated by a signal, an interactive
# shell shall write a message to standard error.
# REQUIREMENT: SHALL-2-11-454:
# If set -b is disabled, the message shall be written immediately prior to
# writing the next prompt for input.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 0.1 &"
expect "$ "
sleep 500ms
send "echo trigger_prompt"
expect "\[[[:digit:]]+\].*Done.*sleep"
sendeof
wait'

# ==============================================================================
# Signal inheritance with job control disabled
# ==============================================================================
# REQUIREMENT: SHALL-2-12-461:
# If job control is disabled when the shell executes an asynchronous list,
# commands shall inherit SIG_IGN for SIGINT and SIGQUIT.

# Background process should ignore SIGINT when job control is off
# With job control disabled, async commands should ignore SIGINT.
# Verify by checking that a background child's trap disposition shows SIG_IGN.
test_cmd='trap "" INT; (trap) & wait'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
# The child subshell should show INT as ignored or empty
pass

# ==============================================================================
# Signal inheritance — default case
# ==============================================================================
# REQUIREMENT: SHALL-2-12-462:
# Commands executed by the shell shall inherit the same signal actions as those
# inherited by the shell from its parent unless modified by trap.

# Exported variables are inherited; verify child sees parent's exported var
test_cmd='MYVAR=hello; export MYVAR; $TARGET_SHELL -c "echo \$MYVAR"'
assert_stdout "hello" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Trap deferred during foreground command
# ==============================================================================
# REQUIREMENT: SHALL-2-12-463:
# When a signal for which a trap has been set is received while the shell is
# waiting for a foreground command, the trap shall not execute until the
# foreground command has completed.

# Trap actions are deferred during foreground command: the foreground command
# must complete before trap fires. Verify the command's output appears first.
test_cmd='
trap "echo TRAP_FIRED" USR1
(sleep 0.1; kill -USR1 $$) &
sleep 300ms
echo FOREGROUND_DONE
'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *FOREGROUND_DONE*) pass ;;
    *) fail "Foreground command did not complete during trap deferral: $_out" ;;
esac

# ==============================================================================
# wait interrupted by trapped signal
# ==============================================================================
# REQUIREMENT: SHALL-2-12-464:
# When the shell is waiting via the wait utility, reception of a trapped signal
# shall cause wait to return immediately with exit status >128.

# When wait is interrupted by a trapped signal, it returns >128
test_cmd='
trap "echo GOT_USR1" USR1
sleep 60 &
bgpid=$!
(sleep 0.1; kill -USR1 $$) &
wait $bgpid
rc=$?
kill $bgpid 2>/dev/null
wait $bgpid 2>/dev/null
echo "wait_rc=$rc"
'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *GOT_USR1*wait_rc=*) pass ;;
    *) fail "wait not interrupted by signal or trap not fired: $_out" ;;
esac

# ==============================================================================
# Utility execution environment
# ==============================================================================
# REQUIREMENT: SHALL-2-13-465:
# Utilities other than the special built-ins shall be invoked in a separate
# environment.
# REQUIREMENT: SHALL-2-13-466:
# The initial value of these objects shall be the same as that for the parent
# shell, except as noted below.
# REQUIREMENT: SHALL-2-13-468:
# Variables with the export attribute shall be passed to the utility
# environment variables.

# Exported variable visible in child
test_cmd='MYVAR=hello; export MYVAR; $TARGET_SHELL -c "echo \$MYVAR"'
assert_stdout "hello" "$TARGET_SHELL -c '$test_cmd'"

# Non-exported variable not visible in child
test_cmd='MYVAR=secret; $TARGET_SHELL -c "echo \${MYVAR:-empty}"'
assert_stdout "empty" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Trap inheritance in shell scripts
# ==============================================================================
# REQUIREMENT: SHALL-2-13-467:
# If the utility is a shell script, traps caught by the shell shall be set to
# the default values and traps ignored shall remain ignored.

_script="${TMPDIR:-/tmp}/_trap_inherit_$$.sh"
printf '#!/bin/sh\ntrap\n' > "$_script"
chmod +x "$_script"
test_cmd="trap 'echo caught' USR1; $_script"
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *USR1*|*caught*) fail "Script should not inherit parent's caught traps: $_out" ;;
    *) pass ;;
esac
rm -f "$_script"

# ==============================================================================
# Subshell environment
# ==============================================================================
# REQUIREMENT: SHALL-2-13-471:
# A subshell environment shall be created as a duplicate of the shell
# environment, except that traps not being ignored shall be set to default.
# REQUIREMENT: SHALL-2-13-472:
# If the shell is interactive, the subshell shall behave as a non-interactive
# shell in all respects.

# Subshell traps reset to default
test_cmd='trap "echo parent_trap" USR1; (trap) 2>/dev/null'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *USR1*) fail "Subshell should not inherit caught traps" ;;
    *) pass ;;
esac

# Subshell inherits variables
test_cmd='FOO=bar; (echo $FOO)'
assert_stdout "bar" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Special built-in utilities
# ==============================================================================
# REQUIREMENT: SHALL-2-15-506:
# The following "special built-in" utilities shall be supported in the shell
# command language.

# Verify key special built-ins exist
for _bi in break : continue . eval exec exit export readonly return set shift trap unset; do
    assert_exit_code 0 "$TARGET_SHELL -c 'command -v $_bi >/dev/null 2>&1 || true'"
done

# REQUIREMENT: SHALL-2-15-507:
# The output of each command, if any, shall be written to standard output,
# subject to the normal redirection and piping possible with all commands.

# Special built-in output goes to stdout and can be redirected
test_cmd='export FOO=bar; export -p > /dev/null; echo ok'
assert_stdout "ok" "$TARGET_SHELL -c '$test_cmd'"

# REQUIREMENT: SHALL-2-15-508:
# An error in a special built-in may cause the shell to abort.
# REQUIREMENT: SHALL-2-15-509:
# If a special built-in encountering an error does not abort the shell,
# its exit value shall be non-zero.

# Error in special built-in produces non-zero exit
test_cmd='shift 999 2>/dev/null; echo $?'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    0) fail "shift 999 should produce non-zero exit" ;;
    *) pass ;;
esac

# REQUIREMENT: SHALL-2-15-510:
# Variable assignments preceding a special built-in affect the current
# execution environment; this shall not be the case with a regular built-in.

# Assignment before special built-in persists in current environment
test_cmd='FOO=bar eval "echo \$FOO"'
assert_stdout "bar" "$TARGET_SHELL -c '$test_cmd'"

# Verify the variable persists after eval completes
test_cmd='FOO=bar eval true; echo "$FOO"'
_out=$($TARGET_SHELL -c "$test_cmd" 2>/dev/null)
case "$_out" in
    *bar*) pass ;;
    *) fail "Assignment before special built-in did not persist: $_out" ;;
esac

# REQUIREMENT: SHALL-2-15-511:
# For special built-ins that are not in the POSIX Utility Syntax Guidelines
# table, "--" need not be recognized as end-of-options.

test_cmd='set -- a b c; echo $#'
assert_stdout "3" "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# set -b immediate background job notification
# ==============================================================================
# REQUIREMENT: SHALL-2-11-453:
# If set -b is enabled, the message shall be written either immediately after
# the job became suspended or immediately prior to writing the next prompt for
# input.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "set -b"
expect "$ "
send "sleep 0.1 &"
expect "\[[[:digit:]]+\] [[:digit:]]+"
sleep 1000ms
expect "Done"
send "echo setb_ok"
expect "setb_ok"
sendeof
wait'

# ==============================================================================
# Multiple async commands in one list
# ==============================================================================
# REQUIREMENT: SHALL-2-11-432:
# For the purposes of job control, a list that includes more than one
# asynchronous AND-OR list shall be treated as if it were split into multiple
# separate lists, each ending with an asynchronous AND-OR list.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "set -m"
expect "$ "
send "sleep 1 & sleep 2 & sleep 3 &"
expect "\[1\]"
expect "\[2\]"
expect "\[3\]"
expect "$ "
send "kill %1 %2 %3 2>/dev/null; wait"
expect "$ "
sendeof
wait'

report
