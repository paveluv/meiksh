# Test: SHALL-19-09-03-08-001
# Obligation: "The control operator '||' denotes an OR List. The format shall be:"
# Verifies: || is recognized as OR list operator.

false || true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: false || true should exit 0" >&2
    exit 1
fi

exit 0
