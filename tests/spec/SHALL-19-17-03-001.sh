# Test: SHALL-19-17-03-001
# Obligation: "This utility shall do nothing except return a 0 exit status."

:
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: colon did not return exit status 0" >&2
    exit 1
fi

# With arguments (still returns 0)
: foo bar baz
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: colon with args did not return 0" >&2
    exit 1
fi

# Side effect of expansion occurs
: ${COLON_TEST:=default_value}
if [ "$COLON_TEST" != "default_value" ]; then
    printf '%s\n' "FAIL: colon did not perform side-effect expansion" >&2
    exit 1
fi

exit 0
