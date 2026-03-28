# Test: SHALL-19-09-03-07-001
# Obligation: "The exit status of an AND list shall be the exit status of the
#   last command that is executed in the list."
# Verifies: AND list exit status is from last executed command.

true && true && false
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: true&&true&&false should exit 1, got $rc" >&2
    exit 1
fi

false && true
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: false&&true should exit 1 (true never ran), got $rc" >&2
    exit 1
fi

true && true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: true&&true should exit 0, got $rc" >&2
    exit 1
fi

exit 0
