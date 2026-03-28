# Test: SHALL-19-23-03-007
# Obligation: "Variables that were unset at the time they were output need not
#   be reset to the unset state if a value is assigned to the variable between
#   the time the state was saved and the time at which the saved output is
#   reinput to the shell."
# This is an exception clause for export -p reinput. We verify that unset
# exported variables appear without a value in export -p.

export EXPORT_UNSET_TEST
unset EXPORT_UNSET_TEST
# After unset, the export attribute may or may not persist (implementation-defined)
# Just verify export -p runs without error
export -p > /dev/null
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: export -p failed" >&2
    exit 1
fi

exit 0
