# Test: SHALL-19-09-03-04-002
# Obligation: "Each AND-OR list shall be expanded and executed in the order
#   specified."
# Verifies: Side effects of earlier commands visible to later expansions.

X=hello
X=world; result=$X
if [ "$result" != "world" ]; then
    printf '%s\n' "FAIL: expansion did not see prior assignment" >&2
    exit 1
fi

exit 0
