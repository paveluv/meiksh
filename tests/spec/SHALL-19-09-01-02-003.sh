# Test: SHALL-19-09-01-02-003
# Obligation: "If the command name is not a special built-in utility or
#   function, the variable assignments shall be exported for the execution
#   environment of the command and shall not affect the current execution
#   environment."
# Verifies: prefix assignments to external commands are temporary.

result=$("$SHELL" -c '
unset MY_TEMP
MY_TEMP=exported sh -c "printf \"%s\n\" \"\$MY_TEMP\""
printf "%s\n" "${MY_TEMP:-unset}"
')
expected='exported
unset'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: prefix assignment to external cmd should be temporary" >&2
    printf '%s\n' "  got: $result" >&2
    exit 1
fi

exit 0
