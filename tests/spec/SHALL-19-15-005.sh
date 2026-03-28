# Test: SHALL-19-15-005
# Obligation: "variable assignments preceding the invocation of a special
#   built-in utility affect the current execution environment; this shall not
#   be the case with a regular built-in or other utility."
# (Duplicate of SHALL-19-15-003)
# Verifies: prefix assignments persist for special built-ins, not for regular.

unset V1
V1=kept :
if [ "$V1" != "kept" ]; then
    printf '%s\n' "FAIL: prefix to : did not persist" >&2
    exit 1
fi

unset V2
V2=gone true
if [ -n "$V2" ]; then
    printf '%s\n' "FAIL: prefix to true persisted" >&2
    exit 1
fi
exit 0
