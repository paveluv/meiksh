# Test: SHALL-19-06-02-002
# Obligation: "The value, if any, of parameter shall be substituted."
# Verifies: basic ${parameter} substitution works.

myvar=hello
result="${myvar}"
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: \${myvar} gave '$result', expected 'hello'" >&2
    exit 1
fi

# Unbraced form
result2="$myvar"
if [ "$result2" != "hello" ]; then
    printf '%s\n' "FAIL: \$myvar gave '$result2', expected 'hello'" >&2
    exit 1
fi

# Special parameter
if [ "$?" != "0" ]; then
    printf '%s\n' "FAIL: \$? not substituted correctly" >&2
    exit 1
fi

# Positional parameter
set -- argval
if [ "$1" != "argval" ]; then
    printf '%s\n' "FAIL: \$1 not substituted correctly" >&2
    exit 1
fi

exit 0
