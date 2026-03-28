# Test: SHALL-19-15-006
# Obligation: "Some of the special built-ins are described as conforming to XBD
#   12.2 Utility Syntax Guidelines. For those that are not, the requirement
#   ...that '--' be recognized as a first argument to be discarded does not
#   apply"
# Verifies: set (which conforms to syntax guidelines) handles -- correctly.

# set -- handles -- to end option processing
set -- a b c
if [ "$1" != "a" ] || [ "$2" != "b" ] || [ "$3" != "c" ]; then
    printf '%s\n' "FAIL: set -- a b c did not set positional params" >&2
    exit 1
fi

# set -- with no args clears positional parameters
set --
if [ $# -ne 0 ]; then
    printf '%s\n' "FAIL: set -- did not clear positional params" >&2
    exit 1
fi

exit 0
