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
# REQUIREMENT: SHALL-2-11-090:
# Job Control: Job control is a facility that
# allows users to selectively stop (suspend) the execution of processes...
# REQUIREMENT: SHALL-ASYNCHRONOUS EVENTS-025:
# The sh utility shall take the standard action for all signals (see 1.4
# Utility Description Defaults ) with the following exceptions.
# REQUIREMENT: SHALL-2-11-425:
# If the shell has a controlling terminal and it is the controlling process for
# the terminal session, it shall initially set the foreground process group ID
# associated with the terminal to its own process group ID.
# REQUIREMENT: SHALL-2-11-426:
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
# REQUIREMENT: SHALL-2-11-431:
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
# REQUIREMENT: SHALL-2-11-438:
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
# REQUIREMENT: SHALL-2-11-452:
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
# REQUIREMENT: SHALL-ASYNCHRONOUS EVENTS-026:
# If the shell is interactive, SIGINT signals received during command line
# editing shall be handled as described in the EXTENDED DESCRIPTION, and SIGINT
# signals received at other times shall be caught but no action performed.
# REQUIREMENT: SHALL-ASYNCHRONOUS EVENTS-027:
# If the shell is interactive: SIGQUIT and SIGTERM signals shall be ignored.
# REQUIREMENT: SHALL-ASYNCHRONOUS EVENTS-028:
# If the -m option is in effect, SIGTTIN, SIGTTOU, and SIGTSTP signals shall be
# ignored.
# REQUIREMENT: SHALL-ASYNCHRONOUS EVENTS-029:
# If they are caught, the shell shall, in the signal-catching function, set the
# signal to the default action and raise the signal (after taking any
# appropriate steps, such as restoring terminal settings).

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


# ==============================================================================
# Job Control Subsystem
# ==============================================================================
# REQUIREMENT: SHALL-Command History List-031:
# When the sh utility is being used interactively, it shall maintain a list of
# commands previously entered from the terminal in the file named by the
# HISTFILE environment variable.
# REQUIREMENT: SHALL-DESCRIPTION-602:
# A user shall explicitly exit to leave the interactive shell.
# REQUIREMENT: SHALL-2-11-455:
# The fg and bg utilities shall send a SIGCONT signal to the process group of
# the process(es) whose stopped wait status caused the shell to suspend the job.
# REQUIREMENT: SHALL-2-11-456:
# If the shell has a controlling terminal, the fg utility shall send the
# SIGCONT signal after it has set the foreground process group ID associated
# with the terminal (see above).
# REQUIREMENT: SHALL-2-11-457:
# If the fg utility is used from an interactive shell to bring into the
# foreground a suspended job that was created from a foreground job, before it
# sends the SIGCONT signal the fg utility shall restore the terminal settings to
# the ones that the shell saved when the job was suspended.
# REQUIREMENT: SHALL-2-11-458:
# When a background job completes or is terminated
# by a signal, an interactive shell shall write a mes...
# REQUIREMENT: SHALL-2-11-459:
# When a background job completes or is terminated by a signal, an interactive
# shell shall write a message to standard error, formatted as described by the
# jobs utility (without the -l option) for a job that completed or was
# terminated by a signal, respectively, at the following time: If set -b is
# enabled, the message shall be written immediately after the job completes or
# is terminated.
# REQUIREMENT: SHALL-2-11-460:
# If set -b is disabled, the message shall be
# written immediately prior to writing the next prompt for...
# REQUIREMENT: SHALL-2-12-461:
# If job control is disabled (see the description of set -m ) when the shell
# executes an asynchronous AND-OR list, the commands in the list shall inherit
# from the shell a signal action of ignored (SIG_IGN) for the SIGINT and SIGQUIT
# signals.
# REQUIREMENT: SHALL-2-12-462:
# In all other cases, commands executed by the shell shall inherit the same
# signal actions as those inherited by the shell from its parent unless a signal
# action is modified by the trap special built-in (see trap )
# REQUIREMENT: SHALL-2-12-463:
# When a signal for which a trap has been set is received while the shell is
# waiting for the completion of a utility executing a foreground command, the
# trap associated with that signal shall not be executed until after the
# foreground command has completed.
# REQUIREMENT: SHALL-2-12-464:
# When the shell is waiting, by means of the wait utility, for asynchronous
# commands to complete, the reception of a signal for which a trap has been set
# shall cause the wait utility to return immediately with an exit status >128,
# immediately after which the trap associated with that signal shall be taken.
# REQUIREMENT: SHALL-2-13-465:
# See 2.9.3.1 Asynchronous AND-OR Lists Shell aliases; see 2.3.1 Alias
# Substitution Utilities other than the special built-ins (see 2.15 Special
# Built-In Utilities ) shall be invoked in a separate environment that consists
# of the following.
# REQUIREMENT: SHALL-2-13-466:
# The initial value of these objects shall be the same as that for the parent
# shell, except as noted below.
# REQUIREMENT: SHALL-2-13-467:
# If the utility is a shell script, traps caught by the shell shall be set to
# the default values and traps ignored by the shell shall be set to be ignored
# by the utility; if the utility is not a shell script, the trap actions
# (default or ignore) shall be mapped into the appropriate signal handling
# actions for the utility
# REQUIREMENT: SHALL-2-13-468:
# Variables with the export attribute, along with those explicitly exported for
# the duration of the command, shall be passed to the utility environment
# variables
# REQUIREMENT: SHALL-2-13-471:
# A subshell environment shall be created as a duplicate of the shell
# environment, except that: Unless specified otherwise (see trap ), traps that
# are not being ignored shall be set to the default action.
# REQUIREMENT: SHALL-2-13-472:
# If the shell is interactive, the subshell shall behave as a non-interactive
# shell in all respects except: The expansion of the special parameter '-' may
# continue to indicate that it is interactive.
# REQUIREMENT: SHALL-2-15-506:
# The following "special built-in" utilities shall be supported in the shell
# command language.
# REQUIREMENT: SHALL-2-15-507:
# The output of each command, if any, shall be written to standard output,
# subject to the normal redirection and piping possible with all commands.
# REQUIREMENT: SHALL-2-15-508:
# An implementation may choose to make any utility a built-in; however, the
# special built-in utilities described here differ from regular built-in
# utilities in two respects: An error in a special built-in utility may cause a
# shell executing that utility to abort, while an error in a regular built-in
# utility shall not cause a shell executing that utility to abort. (See 2.8.1
# Consequences of Shell Errors for the consequences of errors on interactive and
# non-interactive shells.) If a special built-in utility encountering an error
# does not abort the shell, its exit value shall be non-zero.
# zero.
# REQUIREMENT: SHALL-2-15-509:
# (See 2.8.1 Consequences of Shell Errors for the consequences of errors on
# interactive and non-interactive shells.) If a special built-in utility
# encountering an error does not abort the shell, its exit value shall be
# non-zero.
# REQUIREMENT: SHALL-2-15-510:
# As described in 2.9.1 Simple Commands , variable assignments preceding the
# invocation of a special built-in utility affect the current execution
# environment; this shall not be the case with a regular built-in or other
# utility.
# REQUIREMENT: SHALL-2-15-511:
# For those that are not, the requirement in 1.4 Utility Description Defaults
# that "--" be recognized as a first argument to be discarded does not apply and
# a conforming application shall not use that argument.

report
