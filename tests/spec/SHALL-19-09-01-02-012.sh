# Test: SHALL-19-09-01-02-012
# Obligation: "If any of the variable assignments attempt to assign a value
#   to a variable for which the readonly attribute is set in the current
#   shell environment ... a variable assignment error shall occur."
# Verifies: assigning to readonly variable produces error.

"$SHELL" -c 'readonly RO=1; RO=2' 2>/dev/null
status=$?
if [ "$status" -eq 0 ]; then
    printf '%s\n' "FAIL: assigning to readonly var should produce error" >&2
    exit 1
fi

# Also with prefix assignment
"$SHELL" -c 'readonly RO=1; RO=2 true' 2>/dev/null
status=$?
if [ "$status" -eq 0 ]; then
    printf '%s\n' "FAIL: prefix readonly assignment should produce error" >&2
    exit 1
fi

exit 0
