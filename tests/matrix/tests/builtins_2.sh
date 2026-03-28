# Test: Special Built-ins (exec, exit)
# Target: tests/matrix/tests/builtins_2.sh
#
# POSIX Shell includes utilities that immediately terminate the current
# execution environment or completely replace the shell process. Here we
# thoroughly test `exec` and `exit`.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Exec Errors and Statuses
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-540: If the exec command fails, a non-
# interactive shell shall exit from the current shell execution envir...
# REQUIREMENT: SHALL-DESCRIPTION-541: If the exec command fails and the shell
# does not exit, any redirections associated with the exec com...
# REQUIREMENT: SHALL-EXIT STATUS-545: If utility is specified and is executed,
# exec shall not return to the shell; rather, the exit status...
# REQUIREMENT: SHALL-EXIT STATUS-546: If utility is specified and an attempt to
# execute it as a non-built
# REQUIREMENT: SHALL-EXIT STATUS-547: If a redirection error occurs (see 2.8.1
# Consequences of Shell Errors), the exit status shall be a v...
# REQUIREMENT: SHALL-ENVIRONMENT VARIABLES-543: The following environment
# variable shall affect the execution of exec:...

test_cmd='
exec /invalid/does/not/exist
echo "survived"
'
assert_exit_code_non_zero \
    "$TARGET_SHELL -c '$test_cmd'"

# ==============================================================================
# The 'exec' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-538: If exec is specified with no operands, any
# redirections associated with the exec command shall be made in the current
# shell execution environment.
# REQUIREMENT: SHALL-DESCRIPTION-539: If exec is specified with a utility
# operand, the shell shall execute a non-built-in utility as described in 2.9.1
# Simple Commands... replacing the shell process...
# REQUIREMENT: SHALL-DESCRIPTION-542: The exec special built-in shall support
# XBD 12.2 Utility Syntax Guidelines....

# `exec` with no operands manipulates the file descriptors permanently for the
# current shell.
test_cmd='exec 3>tmp_fd3.txt; echo "fd3 test" >&3; exec 3>&-; cat tmp_fd3.txt'
assert_stdout "fd3 test" \
    "$TARGET_SHELL -c '$test_cmd'"

# `exec` with operands replaces the shell. We test this by using `exec echo`.
# The `echo "never runs"` should not execute because `exec` consumed the process.
test_cmd='exec printf "%s" "exec test"; echo "never runs"'
assert_stdout "exec test" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# The 'exit' Utility
# ==============================================================================
# REQUIREMENT: SHALL-DESCRIPTION-549: The exit utility shall cause the shell to
# exit from its current execution environment.
# REQUIREMENT: SHALL-DESCRIPTION-550: If the current execution environment is a
# subshell environment, the shell shall exit from the subshell environment.
# REQUIREMENT: SHALL-DESCRIPTION-551: The wait status of the shell or subshell
# shall be determined by the unsigned decimal integer n...
# REQUIREMENT: SHALL-DESCRIPTION-554: No other actions associated with the
# signal, such as execution of trap actions or creation of a core...
# REQUIREMENT: SHALL-DESCRIPTION-555: If n is not specified, the result shall be
# as if n were specified with the current value of the special parameter '?'...

# `exit n` explicitly sets the exit status to `n`.
test_cmd='exit 42'
assert_exit_code 42 \
    "$TARGET_SHELL -c '$test_cmd'"

# `exit` inside a subshell only terminates the subshell, leaving the parent.
test_cmd='(exit 99); echo "$?"'
assert_stdout "99" \
    "$TARGET_SHELL -c '$test_cmd'"

# `exit` with no `n` sets the exit code to the last executed command `$?`.
test_cmd='false; exit'
assert_exit_code 1 \
    "$TARGET_SHELL -c '$test_cmd'"


report
