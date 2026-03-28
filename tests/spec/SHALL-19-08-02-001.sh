# Test: SHALL-19-08-02-001
# Obligation: "The exit status of a command shall be determined as follows:"
# Verifies: exit status determination chain is applied (covers the overall
#   rule by testing representative cases from sub-obligations).

# Normal exit: exit status is the value passed to exit
"$SHELL" -c 'exit 42'
if [ $? -ne 42 ]; then
    printf '%s\n' "FAIL: exit 42 did not produce status 42" >&2
    exit 1
fi

# Command not found: 127
"$SHELL" -c 'nonexistent_cmd_xyzzy_test' 2>/dev/null
if [ $? -ne 127 ]; then
    printf '%s\n' "FAIL: command not found did not produce status 127" >&2
    exit 1
fi

exit 0
