# Test: Shell History Management
# Target: tests/matrix/tests/sh_history.sh
#
# Tests POSIX requirements for command history: HISTFILE, HISTSIZE,
# FCEDIT, history list management.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# FCEDIT default editor
# ==============================================================================
# REQUIREMENT: SHALL-SH-1019:
# The FCEDIT variable, when expanded by the shell, shall determine the
# default value for the -e editor option's editor option-argument.
# REQUIREMENT: SHALL-SH-1020:
# If FCEDIT is null or unset, ed shall be used as the editor.

# Verify FCEDIT can be set and fc uses it
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTSIZE=100"
expect "$ "
send "echo fcedit_test"
expect "fcedit_test"
expect "$ "
send "FCEDIT=true fc -e true"
expect "$ "
sendeof
wait'

# ==============================================================================
# History list minimum size
# ==============================================================================
# REQUIREMENT: SHALL-SH-1024:
# The maximum number of commands in the history list is unspecified, but
# shall be at least 128.

# Set HISTSIZE to 128 and verify we can recall commands
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTSIZE=128"
expect "$ "
send "echo hist_min_size"
expect "hist_min_size"
expect "$ "
send "fc -l -1 -1"
expect "echo hist_min_size"
expect "$ "
sendeof
wait'

# ==============================================================================
# History deletion order
# ==============================================================================
# REQUIREMENT: SHALL-SH-1023:
# As entries are deleted from the history file, they shall be deleted
# oldest first.

# Add several commands with small HISTSIZE, verify oldest are dropped
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTSIZE=3"
expect "$ "
send "echo cmd1"
expect "cmd1"
expect "$ "
send "echo cmd2"
expect "cmd2"
expect "$ "
send "echo cmd3"
expect "cmd3"
expect "$ "
send "echo cmd4"
expect "cmd4"
expect "$ "
send "fc -l -2 -1"
expect "cmd4"
not_expect "cmd1"
expect "$ "
sendeof
wait'

# ==============================================================================
# History file fallback
# ==============================================================================
# REQUIREMENT: SHALL-SH-1021-DUP621:
# If the shell cannot obtain both read and write access to, or create,
# the history file, it shall use an unspecified mechanism that allows
# the history to operate properly.

# Set HISTFILE to an unwritable location; history should still work
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTFILE=/dev/null/impossible"
expect "$ "
send "echo hist_works"
expect "hist_works"
expect "$ "
sendeof
wait'

# ==============================================================================
# Commands treated as strings, not filenames
# ==============================================================================
# REQUIREMENT: SHALL-SH-1048:
# Commands in the command history shall be treated as strings, not as
# filenames.

# A command with special glob characters should be recalled literally
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTSIZE=100"
expect "$ "
send "echo *.txt"
expect "$ "
send "fc -l -1 -1"
expect "echo \*\.txt"
expect "$ "
sendeof
wait'

report
