# Test: SHALL-19-10-02-008
# Obligation: "When the TOKEN is exactly a reserved word, the token identifier
#   for that reserved word shall result. Otherwise, when the TOKEN meets the
#   requirements for a name, the token identifier NAME shall result."
# Verifies: Function names must be valid names.

# Valid function name
my_func() { printf '%s\n' "ok"; }
result=$(my_func)
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: valid function name not accepted" >&2
    exit 1
fi

# Function name with underscore start
_func() { printf '%s\n' "ok"; }
result=$(_func)
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: underscore-prefixed function name not accepted" >&2
    exit 1
fi

exit 0
