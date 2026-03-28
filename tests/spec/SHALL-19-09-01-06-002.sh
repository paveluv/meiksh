# Test: SHALL-19-09-01-06-002
# Obligation: "In either case, execution of the utility in the specified
#   environment shall be performed as follows"
# Verifies: non-built-in utilities are executed (intro obligation; test
#   via representative external command).

result=$("$SHELL" -c '/bin/echo "ext_ok"')
if [ "$result" != "ext_ok" ]; then
    printf '%s\n' "FAIL: external command not executed" >&2
    exit 1
fi

exit 0
