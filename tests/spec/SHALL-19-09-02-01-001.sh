# Test: SHALL-19-09-02-01-001
# Obligation: "The exit status of a pipeline shall depend on whether or not the
#   pipefail option ... is enabled and whether or not the pipeline begins with
#   the ! reserved word"
# Verifies: Pipeline exit status with ! prefix and default (no pipefail).

# Without !: exit status is from last command
true | false
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: true|false should exit 1, got $rc" >&2
    exit 1
fi

# With !: logical NOT of last command's status
! true | false
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: ! true|false should exit 0 (NOT of 1), got $rc" >&2
    exit 1
fi

! true | true
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: ! true|true should exit 1 (NOT of 0), got $rc" >&2
    exit 1
fi

exit 0
