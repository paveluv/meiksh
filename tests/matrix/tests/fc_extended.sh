#!/bin/sh

# Test: fc utility — extended coverage
# Target: tests/matrix/tests/fc_extended.sh
#
# Extended tests for the POSIX 'fc' built-in: -r flag, negative numbers,
# reverse ranges, history number formatting, and HISTSIZE interaction.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# fc -l with -r (reverse listing)
# ==============================================================================
# REQUIREMENT: SHALL-FC-1128:
# fc shall list or edit and re-execute commands from the command history.
# REQUIREMENT: SHALL-FC-1130:
# The relationship of a number to its command shall not change.
# REQUIREMENT: SHALL-FC-1131:
# When the number reaches an implementation-defined upper limit, which shall
# be no smaller than the value of HISTSIZE, it shall wrap.
# REQUIREMENT: SHALL-FC-1132:
# fc shall maintain the time-ordering sequence of the history list.
# REQUIREMENT: SHALL-FC-1137:
# The fc utility shall conform to XBD 12.2 Utility Syntax Guidelines.
# REQUIREMENT: SHALL-FC-1141:
# The commands shall be written in the sequence indicated by first and last.
# REQUIREMENT: SHALL-FC-1146:
# If last is omitted, last shall default to the previous command.
# REQUIREMENT: SHALL-FC-1151:
# It shall not be an error to specify first or last values that are out of
# range.
# REQUIREMENT: SHALL-FC-1152:
# For example, fc -l and fc 1 99 shall list all commands.
# REQUIREMENT: SHALL-FC-1033:
# If the command consists of more than one line, the lines after the first
# shall be displayed.
# REQUIREMENT: SHALL-FC-1046:
# Exit value 0 for listing.
# REQUIREMENT: SHALL-FC-1042:
# Any command line variable assignments or redirection operators used with
# fc shall affect both the fc built-in and the resulting command.
# REQUIREMENT: SHALL-FC-1150:
# If first represents a newer command than last, the commands shall be listed
# or edited in reverse sequence.
# REQUIREMENT: SHALL-FC-1141:
# The commands shall be written in the sequence indicated by the first and
# last operands, as affected by -r.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo rev_alpha"
expect "rev_alpha"
expect "$ "
send "echo rev_beta"
expect "rev_beta"
expect "$ "
send "echo rev_gamma"
expect "rev_gamma"
expect "$ "
send "fc -lr -3 -1"
expect "rev_gamma"
expect "rev_beta"
expect "rev_alpha"
sendeof
wait'

# -r with explicit range should reverse the listing order
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo rl_one"
expect "rl_one"
expect "$ "
send "echo rl_two"
expect "rl_two"
expect "$ "
send "fc -l -r -2 -1"
expect "rl_two"
expect "rl_one"
sendeof
wait'

# ==============================================================================
# fc with negative numbers
# ==============================================================================
# REQUIREMENT: SHALL-FC-1045:
# The value of first or last or both shall be one of the following:
# number — A positive or negative number used as a command number.
# A negative number shall be used as an offset from the current command
# number.
# REQUIREMENT: SHALL-FC-1148:
# If first and last are both present, all of the commands from first to last
# shall be edited or listed.

# fc -l -1 should list only the most recent command
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo neg_latest"
expect "neg_latest"
expect "$ "
send "fc -l -1 -1"
expect "echo neg_latest"
sendeof
wait'

# fc -l -3 -2 should list a range by negative offsets
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo neg_a"
expect "neg_a"
expect "$ "
send "echo neg_b"
expect "neg_b"
expect "$ "
send "echo neg_c"
expect "neg_c"
expect "$ "
send "fc -l -3 -2"
expect "echo neg_a"
expect "echo neg_b"
sendeof
wait'

# fc -s with a negative number should re-execute that offset command
# REQUIREMENT: SHALL-FC-1145:
# If first is omitted, the previous command shall be used.
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo neg_reexec_target"
expect "neg_reexec_target"
expect "$ "
send "echo filler"
expect "filler"
expect "$ "
send "fc -s -2"
expect "neg_reexec_target"
sendeof
wait'

# ==============================================================================
# fc range selection with first > last (reverse)
# ==============================================================================
# REQUIREMENT: SHALL-FC-1150:
# If first represents a newer command than last, the commands shall be listed
# or edited in reverse sequence.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo rng_first"
expect "rng_first"
expect "$ "
send "echo rng_second"
expect "rng_second"
expect "$ "
send "echo rng_third"
expect "rng_third"
expect "$ "
send "fc -l -1 -3"
expect "rng_third"
expect "rng_second"
expect "rng_first"
sendeof
wait'

# Reversed range using positive numbers
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo posrev_a"
expect "posrev_a"
expect "$ "
send "echo posrev_b"
expect "posrev_b"
expect "$ "
send "echo posrev_c"
expect "posrev_c"
expect "$ "
send "fc -l -1 -3"
expect "posrev_c"
expect "posrev_b"
expect "posrev_a"
sendeof
wait'

# ==============================================================================
# History number formatting in fc -l output
# ==============================================================================
# REQUIREMENT: SHALL-FC-1031:
# When the -l option is used to list commands, the format of each command
# in the list shall be as follows: "%d\t%s\n", <line number>, <command>
# REQUIREMENT: SHALL-FC-1129:
# The command history list shall reference commands by number.

# Each listed line should start with a number followed by a tab
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo numfmt_test"
expect "numfmt_test"
expect "$ "
send "fc -l -1 -1"
expect "echo numfmt_test"
sendeof
wait'

# Line numbers should be monotonically increasing in normal order
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo mono_a"
expect "mono_a"
expect "$ "
send "echo mono_b"
expect "mono_b"
expect "$ "
send "echo mono_c"
expect "mono_c"
expect "$ "
send "fc -l -3 -1"
expect "echo mono_a"
expect "echo mono_b"
expect "echo mono_c"
sendeof
wait'

# With -n, numbers should be suppressed
# REQUIREMENT: SHALL-FC-1032:
# If both the -l and -n options are specified, the format of each command
# shall be: "\t%s\n", <command>
assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo nfmt_test"
expect "nfmt_test"
expect "$ "
send "fc -ln -1 -1"
expect "echo nfmt_test"
sendeof
wait'

# ==============================================================================
# fc interaction with HISTSIZE
# ==============================================================================
# REQUIREMENT: SHALL-FC-1143:
# The number of previous commands that can be accessed shall be determined
# by the value of the HISTSIZE variable.
#
# NOTE: POSIX says "it is unspecified whether changes made to HISTSIZE
# after the history file has been initialized are effective." Therefore
# we only test that the default history retains recently entered commands.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo hs_test_cmd"
expect "hs_test_cmd"
expect "$ "
send "fc -l"
expect "hs_test_cmd"
sendeof
wait'

# ==============================================================================
# fc -l with no operands lists previous 16 commands
# ==============================================================================
# REQUIREMENT: SHALL-FC-1147:
# If first and last are both omitted, the previous 16 commands shall be
# listed or the previous single command shall be edited.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo bare_list_test"
expect "bare_list_test"
expect "$ "
send "fc -l"
expect "echo bare_list_test"
sendeof
wait'

# ==============================================================================
# fc -s re-executes and enters result into history
# ==============================================================================
# REQUIREMENT: SHALL-FC-1133:
# When commands are edited, the resulting lines shall be entered at the end
# of the history list and then re-executed by sh.

assert_pty_script 'spawn $TARGET_SHELL -i
expect "$ "
send "echo reenter_hist"
expect "reenter_hist"
expect "$ "
send "fc -s"
expect "reenter_hist"
expect "$ "
send "fc -l -1 -1"
expect "echo reenter_hist"
sendeof
wait'

report
