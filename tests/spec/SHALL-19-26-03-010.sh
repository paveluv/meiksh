# Test: SHALL-19-26-03-010
# Obligation: "The shell shall disable pathname expansion."

set -f
result=$(printf '%s' *)
set +f
if [ "$result" != "*" ]; then
    printf '%s\n' "FAIL: set -f did not disable pathname expansion" >&2
    exit 1
fi

exit 0
