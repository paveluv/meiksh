# Test: SHALL-19-09-04-02-001
# Obligation: "The exit status of a grouping command shall be the exit status
#   of compound-list."
# Verifies: Grouping command exit status matches inner compound-list.

{ true; false; }
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: brace group exit should be 1, got $rc" >&2
    exit 1
fi

(exit 42)
rc=$?
if [ "$rc" -ne 42 ]; then
    printf '%s\n' "FAIL: subshell (exit 42) should be 42, got $rc" >&2
    exit 1
fi

{ true; }
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: { true; } should exit 0, got $rc" >&2
    exit 1
fi

exit 0
