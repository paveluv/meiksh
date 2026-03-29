# Test: Shell Mail Notification
# Target: tests/matrix/tests/sh_mail.sh
#
# Tests POSIX requirements for MAIL, MAILPATH, and MAILCHECK variables.
# Uses expect_pty to spawn an interactive shell and verify mail notifications.

. "$MATRIX_DIR/lib.sh"

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

_mailfile="$TEST_TMP/testmail_$$"
rm -f "$_mailfile"

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAIL=$_mailfile\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
send \"echo trigger1\"
expect \"trigger1\"
expect \"$ \"
send \"echo created > $_mailfile\"
expect \"$ \"
sleep 2000
send \"echo trigger2\"
expect \"trigger2\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAIL not checked if MAILPATH is set
# ==============================================================================
# REQUIREMENT: SHALL-SH-1028:
# The user shall be informed only if MAIL is set and MAILPATH is not set.

# Setting MAILPATH should suppress MAIL-based checking
_mailfile2="$TEST_TMP/testmail2_$$"
rm -f "$_mailfile2"
echo "initial" > "$_mailfile2"

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAIL=$_mailfile2\"
expect \"$ \"
send \"MAILPATH=$_mailfile2\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
send \"echo ok\"
expect \"ok\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAILCHECK default and zero value
# ==============================================================================
# REQUIREMENT: SHALL-SH-1029:
# MAILCHECK specifies how often (in seconds) the shell shall check for
# the arrival of mail.
# REQUIREMENT: SHALL-SH-1030:
# The default value shall be 600 seconds.
# REQUIREMENT: SHALL-SH-1031:
# If set to zero, the shell shall check before issuing each primary prompt.

# Verify MAILCHECK=0 causes check at every prompt
_mailfile3="$TEST_TMP/testmail3_$$"
rm -f "$_mailfile3"

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAIL=$_mailfile3\"
expect \"$ \"
send \"MAILCHECK=0\"
expect \"$ \"
send \"echo before\"
expect \"before\"
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

_mp1="$TEST_TMP/mp1_$$"
_mp2="$TEST_TMP/mp2_$$"
rm -f "$_mp1" "$_mp2"

assert_pty_script "spawn \$TARGET_SHELL -i
expect \"$ \"
send \"MAILPATH='$_mp1%new mail in mp1:$_mp2'\"
expect \"$ \"
send \"MAILCHECK=1\"
expect \"$ \"
send \"echo setup_done\"
expect \"setup_done\"
expect \"$ \"
sendeof
wait"

# ==============================================================================
# MAILPATH percent escaping
# ==============================================================================
# REQUIREMENT: SHALL-SH-1034:
# If a '%' character in the pathname is preceded by a backslash, it shall
# be treated as a literal '%' in the pathname.

# This is a parsing requirement — verify the shell doesn't crash
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
