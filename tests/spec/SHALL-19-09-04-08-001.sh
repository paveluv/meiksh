# Test: SHALL-19-09-04-08-001
# Obligation: "The exit status of the if command shall be the exit status of
#   the then or else compound-list that was executed, or zero, if none was
#   executed."
# Verifies: if exit status.

if true; then false; fi
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: if true then false should exit 1, got $rc" >&2
    exit 1
fi

if false; then true; fi
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: if false (no else) should exit 0, got $rc" >&2
    exit 1
fi

if false; then true; else false; fi
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: if false else false should exit 1, got $rc" >&2
    exit 1
fi

exit 0
