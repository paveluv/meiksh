# Test: Execution Environment
# Target: tests/matrix/tests/execution_environment.sh
#
# Execution Environments are where the magic happens. Here we test the container
# the shell provides to processes—environment variables, working directories,
# and exit status tracking.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# Script Input Evaluation
# ==============================================================================
# REQUIREMENT: SHALL-INPUT-FILES-020:
# The input file can be of any type, but the initial portion of the file
# intended to be parsed according to the shell grammar (see 2.10.2 Shell Grammar
# Rules ) shall consist of characters and shall not contain the NUL character.
# REQUIREMENT: SHALL-INPUT-FILES-021:
# The shell shall not enforce any line length limits.
# REQUIREMENT: SHALL-INPUT-FILES-022:
# If the input file consists solely of zero or more blank lines and comments,
# sh shall exit with a zero exit status.

test_cmd='

# just a comment


'
echo "$test_cmd" > tmp_empty.sh
assert_exit_code 0 \
    "$TARGET_SHELL tmp_empty.sh"

# ==============================================================================
# The Context: Shell Execution Environment
# ==============================================================================
# REQUIREMENT: SHALL-2-8-1-246:
# Shell Execution Environment: The shell execution
# environment includes variables, working directory, file descriptors...

# We ask the shell to look around and verify it knows exactly where it's
# standing. When invoked by our test harness, `$PWD` should match its output
# when it executes `pwd`.
test_cmd='pwd'
assert_stdout "$PWD" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The Outcome: Exit Statuses
# ==============================================================================
# REQUIREMENT: SHALL-2-8-2-259:
# Exit Status for Commands: The exit status of a
# command is that of the last command executed...

# Truth and falsehood in the shell are defined by exit codes. `true` yields 0,
# and `false` yields 1 (or any non-zero value). The shell must dutifully
# capture and return these exit codes when it finishes executing.
test_cmd_true='true'
assert_exit_code 0 \
    "$TARGET_SHELL -c '$test_cmd_true'"

test_cmd_false='false'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd_false'"


# ==============================================================================
# Simple Commands: Variables and Redirections
# ==============================================================================
# REQUIREMENT: SHALL-2-9-1-4-287:
# Simple Commands: A simple command is a sequence
# of optional variable assignments and redirections...

# Before a command executes, the shell evaluates any leading variable
# assignments. These assignments live and die with the command they prefix.
# Here, we export `FOO=bar` explicitly to the subshell `sh -c` to prove it
# successfully captured and transmitted the environment variable.
test_cmd='FOO=bar sh -c "echo \$FOO"'
assert_stdout "bar" \
    "$TARGET_SHELL -c '$test_cmd'"


report
