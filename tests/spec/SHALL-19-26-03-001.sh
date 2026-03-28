# Test: SHALL-19-26-03-001
# Obligation: "If no options or arguments are specified, set shall write the
#   names and values of all shell variables in the collation sequence of the
#   current locale."

# set with no args outputs variables
SETTEST_VAR=hello
output=$(set)
case "$output" in
    *SETTEST_VAR=*) ;;
    *)
        printf '%s\n' "FAIL: set without args did not list SETTEST_VAR" >&2
        exit 1
        ;;
esac

exit 0
