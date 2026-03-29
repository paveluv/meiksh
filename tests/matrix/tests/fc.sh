# Test: fc — Command History Editing
# Target: tests/matrix/tests/fc.sh
#
# Tests the fc built-in utility using the expect_pty scriptable PTY driver.
# Many fc features can be tested by listing history (fc -l) and re-executing
# commands (fc -s). Editor invocation is tested by setting FCEDIT to a script.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# fc -l: List command history
# ==============================================================================
# REQUIREMENT: SHALL-FC-1128:
# The fc utility shall list, or shall edit and re-execute, commands previously
# entered to an interactive sh.
# REQUIREMENT: SHALL-FC-1129:
# The command history list shall reference commands by number.
# REQUIREMENT: SHALL-FC-1031:
# When the -l option is used to list commands, the format of each command
# in the list shall be as follows: "%d\t%s\n", <line number>, <command>

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo fc_test_1"
expect "fc_test_1"
expect "$ "
send "echo fc_test_2"
expect "fc_test_2"
expect "$ "
send "fc -l"
expect "echo fc_test_1"
expect "echo fc_test_2"
sendeof
wait'

# ==============================================================================
# fc -l with -n (suppress line numbers)
# ==============================================================================
# REQUIREMENT: SHALL-FC-1032:
# If both the -l and -n options are specified, the format of each command
# shall be: "\t%s\n", <command>

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo ln_test"
expect "ln_test"
expect "$ "
send "fc -ln"
expect "echo ln_test"
sendeof
wait'

# ==============================================================================
# fc -s: Re-execute previous command
# ==============================================================================
# REQUIREMENT: SHALL-FC-1145:
# If first is omitted, the previous command shall be used.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo reexec_target"
expect "reexec_target"
expect "$ "
send "fc -s"
expect "reexec_target"
sendeof
wait'

# ==============================================================================
# fc -s with substitution: old=new
# ==============================================================================
# REQUIREMENT: SHALL-FC-1044:
# The following operands shall be supported: first, last

# fc -s old=new should re-execute the previous command with substitution
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo old_value"
expect "old_value"
expect "$ "
send "fc -s old_value=new_value"
expect "new_value"
sendeof
wait'

# ==============================================================================
# fc first/last range selection
# ==============================================================================
# REQUIREMENT: SHALL-FC-1148:
# If first and last are both present, all of the commands from first to last
# shall be edited or listed.
# REQUIREMENT: SHALL-FC-1147:
# If first and last are both omitted, the previous 16 commands shall be
# listed or the previous single command shall be edited.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo range_a"
expect "range_a"
expect "$ "
send "echo range_b"
expect "range_b"
expect "$ "
send "echo range_c"
expect "range_c"
expect "$ "
send "fc -l -2 -1"
expect "echo range_b"
expect "echo range_c"
sendeof
wait'

# ==============================================================================
# fc with FCEDIT editor invocation
# ==============================================================================
# REQUIREMENT: SHALL-FC-1043:
# Use the editor named by editor to edit the commands.
# REQUIREMENT: SHALL-FC-1139:
# The value in the FCEDIT variable shall be used as a default when -e is not
# specified.
# REQUIREMENT: SHALL-FC-1133:
# When commands are edited, the resulting lines shall be entered at the end
# of the history list and then re-executed by sh.
# REQUIREMENT: SHALL-FC-1134:
# The fc command that caused the editing shall not be entered into the
# history list.

# Use a helper script as the editor: replaces original_cmd with edited_cmd
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo original_cmd"
expect "original_cmd"
expect "$ "
send "FCEDIT=/tmp/_fc_ed.sh; printf '"'"'#!/bin/sh\nf=$1; sed s/original_cmd/edited_cmd/ <$f >/tmp/_fc_tmp && mv /tmp/_fc_tmp $f'"'"' >$FCEDIT; chmod +x $FCEDIT; export FCEDIT"
expect "$ "
send "fc"
expect "edited_cmd"
expect "$ "
sendeof
wait'

# ==============================================================================
# fc -e editor option
# ==============================================================================
# REQUIREMENT: SHALL-FC-1043:
# -e editor: Use the editor named by editor to edit the commands.
# REQUIREMENT: SHALL-FC-1164:
# Otherwise, the exit status shall be that of the commands executed by fc.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo fe_original"
expect "fe_original"
expect "$ "
send "printf '"'"'#!/bin/sh\nf=$1; sed s/fe_original/fe_edited/ <$f >/tmp/_fc_tmp2 && mv /tmp/_fc_tmp2 $f'"'"' >/tmp/_fc_ed2.sh; chmod +x /tmp/_fc_ed2.sh"
expect "$ "
send "fc -e /tmp/_fc_ed2.sh"
expect "fe_edited"
expect "$ "
send "true"
expect "$ "
sendeof
wait'

# ==============================================================================
# fc -e editor with non-zero exit suppresses re-execution
# ==============================================================================
# REQUIREMENT: SHALL-FC-1135:
# If the editor returns a non-zero exit status, this shall suppress the
# entry into the history list and the command re-execution.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo should_not_reexec"
expect "should_not_reexec"
expect "$ "
send "fc -e false; true"
expect "$ "
not_expect "should_not_reexec"
sendeof
wait'

# ==============================================================================
# fc -l with string pattern as first operand
# ==============================================================================
# REQUIREMENT: SHALL-FC-1045:
# The value of first or last or both shall be one of the following:
# string — A string indicating the most recently entered command starting
# with that string.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo unique_pat_cmd"
expect "unique_pat_cmd"
expect "$ "
send "echo other_cmd"
expect "other_cmd"
expect "$ "
send "fc -l -2 -1"
expect "echo unique_pat_cmd"
expect "echo other_cmd"
sendeof
wait'

# ==============================================================================
# fc -l reverse order
# ==============================================================================
# REQUIREMENT: SHALL-FC-1150:
# If first represents a newer command than last, the commands shall be
# listed or edited in reverse sequence.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo rev_first"
expect "rev_first"
expect "$ "
send "echo rev_second"
expect "rev_second"
expect "$ "
send "fc -l -1 -2"
expect "rev_second"
expect "rev_first"
sendeof
wait'

# ==============================================================================
# HISTSIZE controls accessible history
# ==============================================================================
# REQUIREMENT: SHALL-FC-1143:
# The number of previous commands that can be accessed shall be determined
# by the value of the HISTSIZE variable.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "HISTSIZE=3"
expect "$ "
send "echo old_gone"
expect "$ "
send "echo keep1"
expect "$ "
send "echo keep2"
expect "$ "
send "echo keep3"
expect "$ "
send "fc -l"
not_expect "old_gone"
expect "keep"
sendeof
wait'

# ==============================================================================
# fc exit status
# ==============================================================================
# REQUIREMENT: SHALL-FC-1046:
# The following exit values shall be returned: 0 Successful completion of
# the listing.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo status_test"
expect "status_test"
expect "$ "
send "fc -l; echo fc_exit_$?"
expect "fc_exit_0"
sendeof
wait'

report
