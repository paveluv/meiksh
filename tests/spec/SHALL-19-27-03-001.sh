# Test: SHALL-19-27-03-001
# Obligation: "The positional parameters shall be shifted. Positional parameter
#   1 shall be assigned the value of parameter (1+n), parameter 2 shall be
#   assigned the value of parameter (2+n), and so on."

set -- a b c d e
shift
if [ "$1" != "b" ] || [ "$2" != "c" ] || [ "$#" -ne 4 ]; then
    printf '%s\n' "FAIL: shift 1, expected b c d e ($#=4), got $1 $2 ($#=$#)" >&2
    exit 1
fi

set -- a b c d e
shift 3
if [ "$1" != "d" ] || [ "$2" != "e" ] || [ "$#" -ne 2 ]; then
    printf '%s\n' "FAIL: shift 3, expected d e ($#=2), got $1 $2 ($#=$#)" >&2
    exit 1
fi

exit 0
