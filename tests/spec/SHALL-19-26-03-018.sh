# Test: SHALL-19-26-03-018
# Obligation: "The remaining arguments shall be assigned in order to the
#   positional parameters. The special parameter '#' shall be set to reflect
#   the number of positional parameters. All positional parameters shall be
#   unset before any new values are assigned."

set -- a b c
if [ "$#" -ne 3 ]; then
    printf '%s\n' "FAIL: \$# not 3 after set -- a b c, got $#" >&2
    exit 1
fi
if [ "$1" != "a" ] || [ "$2" != "b" ] || [ "$3" != "c" ]; then
    printf '%s\n' "FAIL: positional params not set correctly" >&2
    exit 1
fi

# Old params are unset before new assignment
set -- x y
if [ "$#" -ne 2 ]; then
    printf '%s\n' "FAIL: \$# not 2 after set -- x y, got $#" >&2
    exit 1
fi
if [ "$1" != "x" ] || [ "$2" != "y" ]; then
    printf '%s\n' "FAIL: positional params not replaced" >&2
    exit 1
fi

exit 0
