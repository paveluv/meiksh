# Test: Shell Execution Environment (Subshells)
# Target: tests/matrix/tests/subshells.sh
#
# POSIX separates the execution environment of the main shell from subshells.
# Here we verify that subshells start as a duplicate of the environment but
# any modifications are completely isolated from the parent.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Subshell Parsing and Status
# ==============================================================================
# REQUIREMENT: SHALL-2-9-4-1-346: The format for grouping commands is as
# follows: (compound-list)
# REQUIREMENT: SHALL-Exit Status-348: The exit status of a grouping command
# shall be the exit status of compound-list....

test_cmd='
( false )
exit $?
'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Subshell Isolation
# ==============================================================================
# REQUIREMENT: SHALL-2-13-470: A subshell environment shall be created as a
# duplicate of the shell environment, except that:...
# REQUIREMENT: SHALL-2-13-473: Changes made to the subshell environment shall
# not affect the shell environment.
# REQUIREMENT: SHALL-2-13-474: Command substitution, commands that are grouped
# with parentheses, and asynchronous AND-OR lists shall be executed in a
# subshell environment.

# Parent variable is visible in subshell, but subshell changes do not propagate.
test_cmd='var="parent"; (echo "$var"; var="child"; echo "$var"); echo "$var"'
assert_stdout "parent
child
parent" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing command substitution isolation.
test_cmd='var="parent"; output=$(var="child"; echo "$var"); echo "$output $var"'
assert_stdout "child parent" \
    "$TARGET_SHELL -c '$test_cmd'"

# Testing asynchronous AND-OR list isolation.
test_cmd='var="parent"; { var="child"; echo "$var" > tmp_sub.txt; } & wait; echo "$var"; cat tmp_sub.txt'
assert_stdout "parent
child" \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# Current Environment Execution
# ==============================================================================
# REQUIREMENT: SHALL-2-13-475: Except where otherwise stated, all other commands
# shall be executed in the current shell environment.

# Group commands with `{ ...; }` execute in the current environment.
test_cmd='var="parent"; { var="child"; echo "$var"; }; echo "$var"'
assert_stdout "child
child" \
    "$TARGET_SHELL -c '$test_cmd'"

# Control structures like `if` execute in the current environment.
test_cmd='var="parent"; if true; then var="child"; fi; echo "$var"'
assert_stdout "child" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Utility Invocations
# ==============================================================================
# REQUIREMENT: SHALL-2-13-469: The environment of the shell process shall not be
# changed by the utility unless explicitly specified...

# Invoking `cd` via `env` or an external utility does not change the parent's
# working directory, but the built-in `cd` does.
test_cmd='cd /tmp; parent="$PWD"; env cd /; echo "$PWD" = "$parent"'
assert_stdout "/tmp = /tmp" \
    "$TARGET_SHELL -c '$test_cmd'"


report
