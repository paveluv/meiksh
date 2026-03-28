# Test: SHALL-19-09-04-10-001
# Obligation: "The exit status of the while loop shall be the exit status of
#   the last compound-list-2 executed, or zero if none was executed."
# Verifies: while loop exit status.

while false; do echo hi; done
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: while(false) should exit 0 (body never ran), got $rc" >&2
    exit 1
fi

n=0
while [ "$n" -lt 3 ]; do n=$((n + 1)); false; done
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: while with false body should exit 1, got $rc" >&2
    exit 1
fi

exit 0
