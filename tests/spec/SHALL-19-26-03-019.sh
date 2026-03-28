# Test: SHALL-19-26-03-019
# Obligation: "The command set -- without argument shall unset all positional
#   parameters and set the special parameter '#' to zero."

set -- a b c
if [ "$#" -ne 3 ]; then
    printf '%s\n' "FAIL: setup failed" >&2
    exit 1
fi

set --
if [ "$#" -ne 0 ]; then
    printf '%s\n' "FAIL: set -- did not clear positional params, \$#=$#" >&2
    exit 1
fi

# set -- also prevents listing when first arg starts with - or +
set -- -foo
if [ "$1" != "-foo" ]; then
    printf '%s\n' "FAIL: set -- -foo did not set \$1 to -foo" >&2
    exit 1
fi

exit 0
