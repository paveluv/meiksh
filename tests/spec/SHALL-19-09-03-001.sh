# Test: SHALL-19-09-03-001
# Obligation: "The operators '&&' and '||' shall have equal precedence and shall
#   be evaluated with left associativity."
# Verifies: && and || are left-associative with equal precedence.

# false || echo bar && echo baz => (false || echo bar) && echo baz
# Should print both "bar" and "baz"
result=$(false || printf '%s\n' bar && printf '%s\n' baz)
expected="bar
baz"
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: left associativity not honored" >&2
    exit 1
fi

# true || echo bar && echo baz => (true || echo bar) && echo baz
# "bar" is skipped, "baz" runs
result=$(true || printf '%s\n' bar && printf '%s\n' baz)
if [ "$result" != "baz" ]; then
    printf '%s\n' "FAIL: true || X && Y did not skip X" >&2
    exit 1
fi

exit 0
