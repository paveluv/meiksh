# Test: Consequences of Shell Errors
# Target: tests/matrix/tests/error_consequences.sh
#
# POSIX strictly defines when a shell must exit due to an error (especially
# in non-interactive shells). This suite validates these critical safety nets.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# Non-Interactive Shell Exits
# ==============================================================================
# REQUIREMENT: SHALL-2-8-1-229: Certain errors shall cause the shell to write a
# diagnostic message to standard error and exit...
# REQUIREMENT: SHALL-2-8-1-230: shall exit
# REQUIREMENT: SHALL-2-8-1-232: shall exit 1.
# REQUIREMENT: SHALL-2-8-1-236: shall exit 4
# REQUIREMENT: SHALL-2-8-1-244: Notes: The shell shall exit only if the special
# built-in utility is executed directly.
# REQUIREMENT: SHALL-2-8-1-246: If the exec command fails, a non-interactive
# shell shall exit from the current shell execution environment; an interactive
# shell may exit from a subshell environment but shall not exit if the current
# shell environment is not a subshell environment.
# REQUIREMENT: SHALL-2-8-1-249: When this option is on, when any command fails
# (for any of the reasons listed in 2.8.1 Consequences of Shell Errors or by
# returning an exit status greater than zero), the shell immediately shall exit,
# as if by executing the exit special built-in utility with no arguments, with
# the following exceptions: The failure of any individual command in a multi-
# command pipeline, or of any subshell environments in which command
# substitution was performed during word expansion, shall not cause the shell to
# exit.
# REQUIREMENT: SHALL-2-8-1-250: If the command_string operand is an empty
# string, sh shall exit with a zero exit status.
# REQUIREMENT: SHALL-2-8-1-254: If an unrecoverable read error occurs when
# reading commands, other than from the file operand of the dot special built-
# in, the shell shall execute no further commands (including any already
# successfully read but not yet executed) other than any specified in a
# previously defined EXIT trap action.
# REQUIREMENT: SHALL-2-8-1-255: An unrecoverable read error while reading from
# the file operand of the dot special built-in shall be treated as a special
# built-in utility error.

# 1. Syntax Error: The shell MUST exit immediately on a syntax error.
echo 'if true; echo "no_then"; fi; echo "survived"' > tmp_err1.sh
assert_exit_code_non_zero \
    "$TARGET_SHELL tmp_err1.sh"
# Ensure it actually didn't run the `echo "survived"`
assert_stdout "" \
    "$TARGET_SHELL tmp_err1.sh 2>/dev/null"

# 2. Variable assignment error on special built-ins:
# Attempting to assign to a readonly variable before a special built-in
# must cause the non-interactive shell to exit.
echo 'readonly RO_VAR=1; RO_VAR=2 export OTHER=3; echo "survived"' > tmp_err2.sh
assert_stdout "" \
    "$TARGET_SHELL tmp_err2.sh 2>/dev/null"

# 3. Special built-in utility error:
# Providing an invalid option to a special built-in (like `set -Z`) must cause
# exit.
echo 'set -Z; echo "survived"' > tmp_err3.sh
assert_stdout "" \
    "$TARGET_SHELL tmp_err3.sh 2>/dev/null"

# ==============================================================================
# Interactive Shells and The 'command' Utility
# ==============================================================================
# REQUIREMENT: SHALL-2-8-1-251: The shell shall exit only if the special built-
# in utility is executed directly.
# REQUIREMENT: SHALL-2-8-1-252: If it is executed via the command utility, the
# shell shall not exit.

# Using `command set -Z` should NOT cause the shell to exit, even in
# non-interactive mode.
echo 'command set -Z; echo "survived"' > tmp_err4.sh
assert_stdout "survived" \
    "$TARGET_SHELL tmp_err4.sh 2>/dev/null"

# ==============================================================================
# Subshell Environments
# ==============================================================================
# REQUIREMENT: SHALL-2-8-1-256: If any of the errors shown as "shall exit" or
# "may exit" occur in a subshell environment, the shell shall (respectively,
# may) exit from the subshell environment with a non-zero status and continue in
# the environment from which that subshell environment was invoked.

# A syntax error in a subshell or a special built-in error in a subshell
# only terminates the subshell, not the parent shell.
test_cmd='
( set -Z ) 2>/dev/null
echo "parent survived"
'
assert_stdout "parent survived" \
    "$TARGET_SHELL -c '$test_cmd'"


# ==============================================================================
# Non-Exiting Errors
# ==============================================================================
# REQUIREMENT: SHALL-2-8-2-258: The exit status of a command shall be determined
# as follows:...
# REQUIREMENT: SHALL-2-8-2-263: Otherwise, the exit status shall be the value
# obtained by the equivalent of the WEXITSTATUS macro applied to the status
# obtained by the wait () function (as defined in the System Interfaces volume
# of POSIX.1-2024).
# REQUIREMENT: SHALL-2-8-1-231: shall not exit.
# REQUIREMENT: SHALL-2-8-1-233: Certain errors shall cause the shell to write a
# diagnostic message to standard error and exit as shown in the following table:
# Error Non-Interactive Shell Interactive Shell Shell Diagnostic Message
# Required Shell language syntax error shall exit shall not exit yes Special
# built-in utility error shall exit 1 shall not exit no 2 Other utility (not a
# special built-in) error shall not exit shall not exit no 3 Redirection error
# with special built-in utilities shall exit shall not exit yes Redirection
# error with compound commands shall not exit shall not exit yes Redirection
# error with function execution shall not exit shall not exit yes Redirection
# error with other utilities (not special built-ins) shall not exit shall not
# exit yes Variable assignment error shall exit shall not exit yes Expansion
# error shall exit shall not exit yes Command not found may exit shall not exit
# yes Unrecoverable read error when reading commands shall exit 4 shall exit 4
# yes Notes: The shell shall exit only if the special built-in utility is
# executed directly.
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-234: shall not exit
# REQUIREMENT: SHALL-2-8-1-253: The shell is not required to write a diagnostic
# message, but the utility itself shall write a diagnostic message if required
# to do so.
# REQUIREMENT: SHALL-2-8-1-257: In all of the cases shown in the table where an
# interactive shell is required not to exit and a non-interactive shell is
# required to exit, an interactive shell shall not perform any further
# processing of the command in which the error occurred.

# Errors on regular utilities or missing utilities do NOT cause the shell to
# exit.
test_cmd='
missing_command_123 2>/dev/null
echo "survived missing command"
'
assert_stdout "survived missing command" \
    "$TARGET_SHELL -c '$test_cmd'"


report
