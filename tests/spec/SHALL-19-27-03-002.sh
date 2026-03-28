# Test: SHALL-19-27-03-002
# Obligation: "The value n shall be an unsigned decimal integer less than or
#   equal to the value of the special parameter '#'. If n is not given, it
#   shall be assumed to be 1. If n is 0, the positional and special parameters
#   are not changed."

# Default n is 1
set -- a b c
shift
if [ "$#" -ne 2 ] || [ "$1" != "b" ]; then
    printf '%s\n' "FAIL: shift without n did not default to 1" >&2
    exit 1
fi

# n=0 is a no-op
set -- a b c
shift 0
if [ "$#" -ne 3 ] || [ "$1" != "a" ]; then
    printf '%s\n' "FAIL: shift 0 was not a no-op" >&2
    exit 1
fi

# n > $# is an error
set -- a b
(shift 5) 2>/dev/null
st=$?
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: shift n>$# should return non-zero" >&2
    exit 1
fi

exit 0
