# Test: Interactive Features
# Target: tests/matrix/tests/interactive.sh
#
# Tests interactive shell features using expect_pty to drive a real PTY
# session. POSIX demands specific behaviors from interactive shells—such
# as dynamically evaluating prompt variables (`$PS1`)—so we orchestrate
# full terminal sessions to prove compliance.

. "$MATRIX_DIR/lib.sh"


# ==============================================================================
# The Face of the Shell: Prompt Strings ($PS1)
# ==============================================================================
# REQUIREMENT: SHALL-Command-History-List-031:
# When the sh utility is being used interactively, it shall maintain a list of
# commands previously entered from the terminal in the file named by the
# HISTFILE environment variable.
# REQUIREMENT: SHALL-DESCRIPTION-602:
# A user shall explicitly exit to leave the interactive shell.
# REQUIREMENT: SHALL-2-5-3-085:
# Parameters: PS1: Each time an interactive shell is
# ready to read a command, the value of this variable shall be subjected to
# parameter expansion...

assert_pty_script 'spawn $TARGET_SHELL -i
expect "\\$ "
send "PS1=\"prompt> \""
expect "prompt> "
send "echo interactive-test"
expect "interactive-test"
sendeof
wait'


# ==============================================================================
# Terminal Erase and Kill
# ==============================================================================
# REQUIREMENT: SHALL-RATIONALE-144:
# Early proposals had the following list entry in vi Line Editing Insert Mode :
# \ If followed by the erase or kill character, that character shall be inserted
# into the input line.
# REQUIREMENT: SHALL-RATIONALE-145:
# Otherwise, the <backslash> itself shall be inserted into the input line.

report
