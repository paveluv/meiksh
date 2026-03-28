# Test: SHALL-19-09-01-02-007
# Obligation: "If no command name results, variable assignments shall affect
#   the current execution environment."
# Duplicate of SHALL-19-09-01-02-002 — same requirement.
# Verifies: assignment-only commands persist.

result=$("$SHELL" -c 'V=ok; printf "%s\n" "$V"')
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: no-command assignment not persistent" >&2
    exit 1
fi

exit 0
