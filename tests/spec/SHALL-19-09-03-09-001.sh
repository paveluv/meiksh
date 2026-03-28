# Test: SHALL-19-09-03-09-001
# Obligation: "The exit status of an OR list shall be the exit status of the
#   last command that is executed in the list."
# Verifies: OR list exit status is from last executed command.

false || false || true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: false||false||true should exit 0, got $rc" >&2
    exit 1
fi

true || false
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: true||false should exit 0 (false never ran), got $rc" >&2
    exit 1
fi

false || false
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: false||false should exit 1, got $rc" >&2
    exit 1
fi

exit 0
