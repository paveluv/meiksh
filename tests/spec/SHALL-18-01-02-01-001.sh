# Test: SHALL-18-01-02-01-001
# Obligation: "Integer variables and constants ... shall be implemented as
#   equivalent to the ISO C standard signed long data type ... All variables
#   shall be initialized to zero if they are not otherwise assigned."
# Verifies: Uninitialized variables are zero in arithmetic; signed long range.

# Uninitialized variable in arithmetic context evaluates to 0
unset _test_uninit_var_18
r=$(( _test_uninit_var_18 + 5 ))
if [ "$r" != "5" ]; then
    printf '%s\n' "FAIL: uninit var should be 0, got $(( _test_uninit_var_18 ))" >&2
    exit 1
fi

# Signed long: at least 32-bit (-2147483648 to 2147483647)
r=$(( 2147483647 ))
if [ "$r" != "2147483647" ]; then
    printf '%s\n' "FAIL: 2147483647 not representable" >&2; exit 1
fi

r=$(( -2147483647 ))
if [ "$r" != "-2147483647" ]; then
    printf '%s\n' "FAIL: -2147483647 not representable" >&2; exit 1
fi

exit 0
