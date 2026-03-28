# Test: SHALL-19-22-03-004
# Obligation: "A trap action on EXIT shall be executed before the shell
#   terminates, except when the exit utility is invoked in that trap action
#   itself, in which case the shell shall exit immediately."

# EXIT trap fires before shell terminates
tmpfile="$TMPDIR/exit_trap_$$.txt"
(
    trap 'printf "%s" "trapped" > '"$tmpfile" EXIT
    exit 0
)
content=$(cat "$tmpfile" 2>/dev/null)
rm -f "$tmpfile"
if [ "$content" != "trapped" ]; then
    printf '%s\n' "FAIL: EXIT trap did not fire before termination" >&2
    exit 1
fi

# exit in EXIT trap causes immediate exit (no infinite loop)
tmpfile2="$TMPDIR/exit_trap2_$$.txt"
(
    trap 'printf "%s" "once" >> '"$tmpfile2"'; exit 0' EXIT
    exit 0
)
content=$(cat "$tmpfile2" 2>/dev/null)
rm -f "$tmpfile2"
if [ "$content" != "once" ]; then
    printf '%s\n' "FAIL: exit in EXIT trap did not cause immediate exit, got '$content'" >&2
    exit 1
fi

exit 0
