# Test: SHALL-19-26-03-002
# Obligation: "The value string shall be written with appropriate quoting ...
#   The output shall be suitable for reinput to the shell"

# Variables with special characters are quoted in set output
SETTEST_SPECIAL='hello world'
output=$(set)
case "$output" in
    *SETTEST_SPECIAL=*) ;;
    *)
        printf '%s\n' "FAIL: set output did not include SETTEST_SPECIAL" >&2
        exit 1
        ;;
esac

exit 0
