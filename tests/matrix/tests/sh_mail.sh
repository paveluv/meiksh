# Test: Shell Mail Notification
# Target: tests/matrix/tests/sh_mail.sh
#
# Tests POSIX requirements for MAIL, MAILPATH, and MAILCHECK variables.
# Uses expect_pty to spawn an interactive shell and verify mail notifications.

. "$MATRIX_DIR/lib.sh"

# Use filenames that don't contain "mail" so notification-matching regexes
# won't false-positive on the command echo.
_mbox1="$TEST_TMP/mbox1_$$"
_mbox2="$TEST_TMP/mbox2_$$"
_mbox3="$TEST_TMP/mbox3_$$"
_mp1="$TEST_TMP/mp1_$$"
_mp2="$TEST_TMP/mp2_$$"
rm -f "$_mbox1" "$_mbox2" "$_mbox3" "$_mp1" "$_mp2"

# ==============================================================================
# MAIL notification on file creation
# ==============================================================================
# REQUIREMENT: SHALL-SH-1025:
# If MAIL is set, the shell shall inform the user if the file named by
# the variable is created or if its modification time has changed.
# REQUIREMENT: SHALL-SH-1026:
# Informing the user shall be accomplished by writing a string of
# unspecified format to standard error prior to the writing of the next
# primary prompt string.
# REQUIREMENT: SHALL-SH-1027:
# Such check shall be performed only after the completion of the interval
# defined by the MAILCHECK variable after the last such check.

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAIL=$_mbox1\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
sleep 1500ms
send \"echo created > $_mbox1\"
expect timeout=5s \"(mail|Mail|MAIL|you have)\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAIL not checked if MAILPATH is set
# ==============================================================================
# REQUIREMENT: SHALL-SH-1028:
# The user shall be informed only if MAIL is set and MAILPATH is not set.

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAILPATH=/tmp/nonexistent_$$\"
expect \"$ \"
send \"MAIL=$_mbox2\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
sleep 1500ms
send \"echo data > $_mbox2\"
not_expect timeout=3s \"(mail|Mail|MAIL|you have)\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAILCHECK=0 checks at every prompt
# ==============================================================================
# REQUIREMENT: SHALL-SH-1029:
# MAILCHECK specifies how often (in seconds) the shell shall check for
# the arrival of mail.
# REQUIREMENT: SHALL-SH-1031:
# If set to zero, the shell shall check before issuing each primary prompt.

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAIL=$_mbox3\"
expect \"$ \"
send \"MAILCHECK=0\"
expect \"$ \"
sleep 1500ms
send \"echo data > $_mbox3\"
expect timeout=5s \"(mail|Mail|MAIL|you have)\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAILPATH with multiple paths and custom messages
# ==============================================================================
# REQUIREMENT: SHALL-SH-1032:
# If MAILPATH is set, the shell shall inform the user if any of the files
# named by the variable are created or if any of their modification times
# change.
# REQUIREMENT: SHALL-SH-1033:
# Each pathname can be followed by '%' and a string that shall be subjected
# to parameter expansion and written to standard error.

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAILPATH='$_mp1%custom msg here:$_mp2'\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
sleep 1500ms
send \"echo data > $_mp1\"
expect timeout=5s \"custom msg here\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAILPATH percent escaping
# ==============================================================================
# REQUIREMENT: SHALL-SH-1034:
# If a '%' character in the pathname is preceded by a backslash, it shall
# be treated as a literal '%' in the pathname.

# Parsing requirement — verify the shell accepts the syntax without error
assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAILPATH='/tmp/file\\%name'\"
expect \"$ \"
send \"echo ok\"
expect \"ok\"
expect \"$ \"
sendeof
wait"

report
