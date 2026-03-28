# Test: SHALL-19-13-001
# Obligation: "Utilities other than the special built-ins shall be invoked in
#   a separate environment that consists of the following."
# Verifies: external utility runs in a child process and cannot alter parent state.

MY_VAR=parent
sh -c 'MY_VAR=child'
if [ "$MY_VAR" != "parent" ]; then
    printf '%s\n' "FAIL: external cmd altered parent MY_VAR" >&2
    exit 1
fi
exit 0
