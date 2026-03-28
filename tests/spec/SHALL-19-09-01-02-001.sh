# Test: SHALL-19-09-01-02-001
# Obligation: "Variable assignments shall be performed as follows:"
# Verifies: variable assignment rules are applied (intro obligation; test
#   representative cases).

# No command name: assignment persists
result=$("$SHELL" -c 'X=hello; printf "%s\n" "$X"')
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: standalone assignment did not persist" >&2
    exit 1
fi

exit 0
