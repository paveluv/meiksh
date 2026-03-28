# Test: SHALL-19-24-03-006
# Obligation: "Variables that were unset at the time they were output do not
#   have a value at the time at which the saved output is reinput to the shell."
# Verify readonly -p runs successfully (condition clause, minimal test).

readonly -p > /dev/null
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: readonly -p failed" >&2
    exit 1
fi

exit 0
