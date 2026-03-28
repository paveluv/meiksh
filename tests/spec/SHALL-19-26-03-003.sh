# Test: SHALL-19-26-03-003
# Obligation: "When options are specified, they shall set or unset attributes
#   of the shell. When arguments are specified, they cause positional
#   parameters to be set or unset."

# Options set shell attributes
set -f
# Pathname expansion should be disabled
result=$(printf '%s' *)
set +f
# With -f, * should be literal
if [ "$result" != "*" ]; then
    printf '%s\n' "FAIL: set -f did not disable pathname expansion" >&2
    exit 1
fi

# Arguments set positional parameters
set -- a b c
if [ "$1" != "a" ] || [ "$2" != "b" ] || [ "$3" != "c" ]; then
    printf '%s\n' "FAIL: set -- did not set positional params" >&2
    exit 1
fi

exit 0
