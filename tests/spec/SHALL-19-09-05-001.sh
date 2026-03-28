# Test: SHALL-19-09-05-001
# Obligation: "the application shall ensure that it is a name ... The
#   implementation shall maintain separate name spaces for functions and
#   variables."
# Verifies: Function name is valid name; function and variable namespaces separate.

# Define function with valid name
myfunc() { printf '%s\n' "func"; }
result=$(myfunc)
if [ "$result" != "func" ]; then
    printf '%s\n' "FAIL: function definition/invocation failed" >&2
    exit 1
fi

# Separate namespaces: variable and function with same name coexist
foo=variable
foo() { printf '%s\n' "function"; }
if [ "$foo" != "variable" ]; then
    printf '%s\n' "FAIL: function definition clobbered variable" >&2
    exit 1
fi
result=$(foo)
if [ "$result" != "function" ]; then
    printf '%s\n' "FAIL: function not callable after same-name variable" >&2
    exit 1
fi

exit 0
