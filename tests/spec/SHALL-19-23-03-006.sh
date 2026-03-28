# Test: SHALL-19-23-03-006
# Obligation: "Read-only variables with values cannot be reset."
# This is an exception clause - not directly testable as a behavior to enforce,
# but we verify readonly exported vars appear in export -p.

export EXPORT_RO_TEST=frozen
readonly EXPORT_RO_TEST
output=$(export -p)
case "$output" in
    *EXPORT_RO_TEST*) ;;
    *)
        printf '%s\n' "FAIL: readonly exported var not in export -p" >&2
        exit 1
        ;;
esac

exit 0
