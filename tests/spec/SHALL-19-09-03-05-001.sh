# Test: SHALL-19-09-03-05-001
# Obligation: "The exit status of a sequential AND-OR list shall be the exit
#   status of the last pipeline in the AND-OR list that is executed."
# Verifies: Exit status reflects last executed pipeline in AND-OR list.

# true && false: last executed is false
true && false
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: true&&false should exit 1, got $rc" >&2
    exit 1
fi

# false || true: last executed is true
false || true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: false||true should exit 0, got $rc" >&2
    exit 1
fi

# false && true: last executed is false (true skipped)
false && true
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: false&&true should exit 1, got $rc" >&2
    exit 1
fi

exit 0
