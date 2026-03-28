# Test: SHALL-19-09-03-02-001
# Obligation: "If an AND-OR list is terminated by the control operator <ampersand>
#   ('&'), the shell shall execute the AND-OR list asynchronously in a subshell
#   environment."
# Verifies: & runs command asynchronously in subshell.

tmpf="$TMPDIR/shall-19-09-03-02-001.$$"
trap 'rm -f "$tmpf"' EXIT

printf '%s\n' "done" >"$tmpf" &
wait
content=$(cat "$tmpf")
if [ "$content" != "done" ]; then
    printf '%s\n' "FAIL: async command did not execute" >&2
    exit 1
fi

# Variable set in & subshell should not leak
ASYNC_VAR=before
ASYNC_VAR=after &
wait
if [ "$ASYNC_VAR" != "before" ]; then
    printf '%s\n' "FAIL: variable from & subshell leaked to parent" >&2
    exit 1
fi

exit 0
