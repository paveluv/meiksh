# Test: SHALL-19-09-03-06-001
# Obligation: "The control operator '&&' denotes an AND list. The format shall be:"
# Verifies: && is recognized as AND list operator.

true && true
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: true && true should exit 0" >&2
    exit 1
fi

exit 0
