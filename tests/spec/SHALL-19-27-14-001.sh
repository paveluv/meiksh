# Test: SHALL-19-27-14-001
# Obligation: "If the n operand is invalid or is greater than \"$#\", ...
#   a non-zero exit status shall be returned and a warning message shall be
#   written to standard error. Otherwise, zero shall be returned."

# Valid shift returns 0
set -- a b c
shift
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: valid shift did not return 0" >&2
    exit 1
fi

# n > $# returns non-zero and writes to stderr
set -- a
err=$( (shift 5) 2>&1)
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: shift 5 with 1 param should return non-zero" >&2
    exit 1
fi
if [ -z "$err" ]; then
    printf '%s\n' "FAIL: shift 5 with 1 param should write warning to stderr" >&2
    exit 1
fi

exit 0
