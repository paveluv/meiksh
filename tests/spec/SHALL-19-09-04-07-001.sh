# Test: SHALL-19-09-04-07-001
# Obligation: "The if command shall execute a compound-list and use its exit
#   status to determine whether to execute another compound-list."
# Verifies: if uses condition's exit status for branching.

result=""
if true; then
    result="yes"
fi
if [ "$result" != "yes" ]; then
    printf '%s\n' "FAIL: if true should execute then clause" >&2
    exit 1
fi

result=""
if false; then
    result="yes"
fi
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: if false should not execute then clause" >&2
    exit 1
fi

exit 0
