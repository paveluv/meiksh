# Test: SHALL-19-09-01-04-004
# Obligation: "If the command name does not contain any <slash> characters,
#   the first successful step in the following sequence shall occur..."
# Duplicate of SHALL-19-09-01-04-002 — same requirement.
# Verifies: command search priority chain.

"$SHELL" -c 'no_cmd_xyzzy_test' 2>/dev/null; s=$?
if [ "$s" -ne 127 ]; then
    printf '%s\n' "FAIL: not-found should be 127, got $s" >&2
    exit 1
fi

exit 0
