# Test: SHALL-19-06-02-003
# Obligation: "If the parameter is a name, the expansion shall use the longest
#   valid name (see XBD 3.216 Name), whether or not the variable denoted by
#   that name exists."
# Verifies: unbraced $name uses longest valid name (greedy parsing).

a=short
ab=longer
abc=longest

# $abc should use the longest name "abc", not "a" or "ab"
result="$abc"
if [ "$result" != "longest" ]; then
    printf '%s\n' "FAIL: \$abc gave '$result', expected 'longest'" >&2
    exit 1
fi

# $abc! should parse "abc" as the name (stops at !)
result2=$(eval 'printf "%s\n" "$abc!"')
if [ "$result2" != "longest!" ]; then
    printf '%s\n' "FAIL: \$abc! gave '$result2', expected 'longest!'" >&2
    exit 1
fi

exit 0
