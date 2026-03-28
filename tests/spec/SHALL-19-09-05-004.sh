# Test: SHALL-19-09-05-004
# Obligation: "The operands to the command temporarily shall become the
#   positional parameters during the execution of the compound-command; the
#   special parameter '#' also shall be changed ... The special parameter 0
#   shall be unchanged. When the function completes, the values of the
#   positional parameters and the special parameter '#' shall be restored"
# Verifies: Positional params scoped to function; $0 unchanged; restored after.

check_params() {
    if [ "$#" -ne 2 ]; then
        printf '%s\n' "FAIL: \$# should be 2 in function, got $#" >&2
        return 1
    fi
    if [ "$1" != "a" ] || [ "$2" != "b" ]; then
        printf '%s\n' "FAIL: \$1/$2 wrong in function" >&2
        return 1
    fi
}

set -- x y z
check_params a b
rc=$?
if [ "$rc" -ne 0 ]; then
    exit 1
fi

# After function, positional params restored
if [ "$#" -ne 3 ]; then
    printf '%s\n' "FAIL: \$# not restored after function: got $#" >&2
    exit 1
fi
if [ "$1" != "x" ] || [ "$2" != "y" ] || [ "$3" != "z" ]; then
    printf '%s\n' "FAIL: positional params not restored after function" >&2
    exit 1
fi

exit 0
