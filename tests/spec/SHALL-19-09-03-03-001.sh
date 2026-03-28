# Test: SHALL-19-09-03-03-001
# Obligation: "The exit status of an asynchronous AND-OR list shall be zero."
# Verifies: $? is 0 immediately after launching an async list.

false &
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: exit status after & should be 0, got $rc" >&2
    exit 1
fi
wait

exit 0
