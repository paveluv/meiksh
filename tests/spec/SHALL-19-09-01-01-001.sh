# Test: SHALL-19-09-01-01-001
# Obligation: "When a given simple command is required to be executed ...
#   the following expansions, assignments, and redirections shall all be
#   performed from the beginning of the command text to the end"
# Verifies: simple command processing order — assignments, expansion,
#   redirections are performed when command is executed.

# Bypassed commands should not expand or assign
X_TEST_001=original
result=$("$SHELL" -c '
X_TEST_001=original
export X_TEST_001
false && X_TEST_001=changed
printf "%s\n" "$X_TEST_001"
')
if [ "$result" != "original" ]; then
    printf '%s\n' "FAIL: bypassed assignment should not execute" >&2
    exit 1
fi

exit 0
