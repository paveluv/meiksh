# Test: SHALL-19-22-03-003
# Obligation: "If n is not specified, the result shall be as if n were specified
#   with the current value of the special parameter '?'"

# exit without n uses $?
(true; exit)
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: exit after true did not return 0" >&2
    exit 1
fi

(false; exit)
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: exit after false returned 0" >&2
    exit 1
fi

exit 0
