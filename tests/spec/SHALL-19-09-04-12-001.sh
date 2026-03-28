# Test: SHALL-19-09-04-12-001
# Obligation: "The exit status of the until loop shall be the exit status of
#   the last compound-list-2 executed, or zero if none was executed."
# Verifies: until loop exit status.

until true; do echo hi; done
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: until(true) should exit 0 (body never ran), got $rc" >&2
    exit 1
fi

n=0
until [ "$n" -ge 3 ]; do n=$((n + 1)); false; done
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: until with false body should exit 1, got $rc" >&2
    exit 1
fi

exit 0
