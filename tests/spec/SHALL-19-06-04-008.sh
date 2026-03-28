# Test: SHALL-19-06-04-008
# Obligation: "All changes to variables in an arithmetic expression shall be in
#   effect after the arithmetic expansion, as in the parameter expansion
#   '${x=value}'."
# Verifies: variable assignments in arithmetic persist.

unset x
: $((x = 42))
if [ "$x" != "42" ]; then
    printf '%s\n' "FAIL: x not set to 42 after \$((x=42)): got '$x'" >&2
    exit 1
fi

# Compound assignment
: $((x += 8))
if [ "$x" != "50" ]; then
    printf '%s\n' "FAIL: x not 50 after \$((x+=8)): got '$x'" >&2
    exit 1
fi

exit 0
