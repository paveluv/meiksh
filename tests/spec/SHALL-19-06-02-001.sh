# Test: SHALL-19-06-02-001
# Obligation: "Any '}' escaped by a <backslash> or within a quoted string, and
#   characters in embedded arithmetic expansions, command substitutions, and
#   variable expansions, shall not be examined in determining the matching '}'."
# Verifies: nested/quoted } inside ${...} does not terminate the expansion.

# Escaped } inside parameter expansion
x='hello}'
result="${x}"
if [ "$result" != 'hello}' ]; then
    printf '%s\n' "FAIL: value with } not handled correctly" >&2
    exit 1
fi

# Nested command substitution containing }
val="inner}"
result2="${val%\}}"
if [ "$result2" != "inner" ]; then
    printf '%s\n' "FAIL: escaped } in pattern not handled: got '$result2'" >&2
    exit 1
fi

# Nested arithmetic inside parameter expansion word
x=""
result3=${x:-$((1+2))}
if [ "$result3" != "3" ]; then
    printf '%s\n' "FAIL: nested arithmetic in param expansion: got '$result3'" >&2
    exit 1
fi

exit 0
