# Test: SHALL-19-09-04-04-001
# Obligation: "If there is at least one item in the list of items, the exit
#   status of a for command shall be the exit status of the last compound-list
#   executed. If there are no items, the exit status shall be zero."
# Verifies: for loop exit status.

for x in a b c; do false; done
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: for with items should exit 1, got $rc" >&2
    exit 1
fi

for x in; do echo hi; done
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: for with no items should exit 0, got $rc" >&2
    exit 1
fi

for x in a; do true; done
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: for with true body should exit 0, got $rc" >&2
    exit 1
fi

exit 0
