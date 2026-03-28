# Test: SHALL-19-26-03-014
# Obligation: "When the shell tries to expand ... an unset parameter other than
#   the '@' and '*' special parameters, it shall write a message to standard
#   error and the expansion shall fail"

# set -u causes error on unset variable expansion
unset UNSET_TEST_VAR
err=$( (set -u; printf '%s' "$UNSET_TEST_VAR") 2>&1)
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: set -u did not fail on unset var expansion" >&2
    exit 1
fi
if [ -z "$err" ]; then
    printf '%s\n' "FAIL: set -u did not write message to stderr" >&2
    exit 1
fi

# $@ and $* are exempt
result=$(set -u; printf '%s' "$@")
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: set -u should not fail on \$@" >&2
    exit 1
fi

exit 0
