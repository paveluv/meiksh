# Test: SHALL-19-24-03-005
# Obligation: "Variables with values at the time they were output do not have
#   the readonly attribute set."
# This is a condition for the reinput guarantee. We verify readonly -p format.

readonly RO_FMT_TEST="hello world"
output=$(readonly -p)
case "$output" in
    *'readonly RO_FMT_TEST='*) ;;
    *)
        printf '%s\n' "FAIL: readonly -p format incorrect" >&2
        exit 1
        ;;
esac

exit 0
