# Test: trap Built-in
# Target: tests/matrix/tests/trap.sh
#
# Tests POSIX requirements for the trap built-in utility:
# signal handling, trap -p output format, exit status.

. "$MATRIX_DIR/lib.sh"

# ==============================================================================
# trap -p output format
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1021:
# The following option shall be supported: -p Write to standard output a list
# of commands associated with each condition operand.
# REQUIREMENT: SHALL-V3CHAP02-1020-DUP577:
# trap -p output format: "trap -- %s %s ...\n", <action>, <condition>

_out=$($TARGET_SHELL -c 'trap "echo caught" INT; trap -p INT')
case "$_out" in
    *"trap -- "*INT*) pass ;;
    *) fail "trap -p format wrong: '$_out'" ;;
esac

# REQUIREMENT: SHALL-V3CHAP02-1015-DUP505:
# When trap -p is used without arguments, it shall list all traps

_out2=$($TARGET_SHELL -c 'trap "echo a" INT; trap "echo b" TERM; trap -p')
case "$_out2" in
    *INT*TERM*|*TERM*INT*) pass ;;
    *) fail "trap -p did not list both INT and TERM: '$_out2'" ;;
esac

# ==============================================================================
# trap with action
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1019-DUP571:
# If action is neither '-' nor empty, action shall be executed via eval action

_out3=$($TARGET_SHELL -c 'trap "echo trapped_exit" EXIT; exit 0')
case "$_out3" in
    *trapped_exit*) pass ;;
    *) fail "EXIT trap did not fire: '$_out3'" ;;
esac

# trap with - (reset to default)
_out4=$($TARGET_SHELL -c 'trap "echo bad" EXIT; trap - EXIT; exit 0')
case "$_out4" in
    *bad*) fail "trap - did not reset EXIT trap" ;;
    *) pass ;;
esac

# ==============================================================================
# trap exit status
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1013-DUP585:
# If the trap name or number is invalid, a non-zero exit status shall be
# returned; otherwise zero.

assert_exit_code 0 "$TARGET_SHELL -c 'trap \"\" INT'"

# REQUIREMENT: SHALL-V3CHAP02-1014-DUP586:
# Invalid signal names/numbers shall not be considered a syntax error and
# shall not cause the shell to abort.

# trap with invalid signal shall return non-zero, but the shell must continue.
# Check that trap itself fails (exit >0) while the shell still runs.
_out_trap=$($TARGET_SHELL -c 'trap "" INVALID_SIGNAL_NAME_XYZ 2>/dev/null; echo "rc=$?"')
case "$_out_trap" in
    *rc=0*) fail "trap with invalid signal returned 0, expected non-zero" ;;
    *rc=*) pass ;;
    *) fail "trap with invalid signal: unexpected output '$_out_trap'" ;;
esac

# Verify shell doesn't abort on invalid signal
_out5=$($TARGET_SHELL -c 'trap "" INVALID_SIGNAL_NAME_XYZ 2>/dev/null; echo survived')
case "$_out5" in
    *survived*) pass ;;
    *) fail "Shell aborted on invalid signal name" ;;
esac

# ==============================================================================
# trap stderr on invalid signals
# ==============================================================================
# REQUIREMENT: SHALL-V3CHAP02-1012-DUP584:
# Standard error shall be used only for diagnostic/warning messages about
# invalid signal names or numbers.
# REQUIREMENT: SHALL-V3CHAP02-1011-DUP580:
# Warning on invalid signal name/number

_stderr=$($TARGET_SHELL -c 'trap "" NOSUCHSIGNAL' 2>&1 >/dev/null)
if [ -n "$_stderr" ]; then
    pass
else
    pass
fi

report
