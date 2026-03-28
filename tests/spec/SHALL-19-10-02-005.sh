# Test: SHALL-19-10-02-005
# Obligation: "When the TOKEN meets the requirements for a name (see XBD 3.216
#   Name), the token identifier NAME shall result."
# Verifies: Valid name accepted after 'for'; invalid name rejected.

# Valid name in for loop
result=""
for valid_name in a; do result="$valid_name"; done
if [ "$result" != "a" ]; then
    printf '%s\n' "FAIL: valid name in for loop not accepted" >&2
    exit 1
fi

# Invalid name should cause syntax error
eval 'for 123invalid in a; do :; done' 2>/dev/null
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s\n' "FAIL: invalid name in for should be syntax error" >&2
    exit 1
fi

exit 0
