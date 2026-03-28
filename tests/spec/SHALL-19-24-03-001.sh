# Test: SHALL-19-24-03-001
# Obligation: "The variables whose names are specified shall be given the
#   readonly attribute. The values of variables with the readonly attribute
#   cannot be changed by subsequent assignment ... nor can those variables be
#   unset by the unset utility."

# readonly prevents reassignment
RO_VAR=initial
readonly RO_VAR
(RO_VAR=changed 2>/dev/null)
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: reassignment to readonly var should fail" >&2
    exit 1
fi

# readonly with name=value sets and locks
readonly RO_VAR2=locked
if [ "$RO_VAR2" != "locked" ]; then
    printf '%s\n' "FAIL: readonly name=value did not set value" >&2
    exit 1
fi

# unset of readonly fails
(unset RO_VAR 2>/dev/null)
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: unset of readonly var should fail" >&2
    exit 1
fi

exit 0
