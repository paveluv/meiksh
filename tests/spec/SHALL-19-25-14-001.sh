# Test: SHALL-19-25-14-001
# Obligation: "The exit status shall be n, if specified ... If n is not
#   specified, the result shall be as if n were specified with the current
#   value of the special parameter '?'"

# return with explicit n
f1() { return 42; }
f1
if [ $? -ne 42 ]; then
    printf '%s\n' "FAIL: return 42 did not produce status 42" >&2
    exit 1
fi

# return without n uses $?
f2() { true; return; }
f2
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: return after true did not return 0" >&2
    exit 1
fi

f3() { false; return; }
f3
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: return after false returned 0" >&2
    exit 1
fi

# return with n=0
f4() { return 0; }
f4
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: return 0 did not return 0" >&2
    exit 1
fi

exit 0
