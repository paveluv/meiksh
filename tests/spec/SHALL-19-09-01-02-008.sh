# Test: SHALL-19-09-01-02-008
# Obligation: "If the command name is not a special built-in utility or
#   function, the variable assignments shall be exported for the execution
#   environment of the command and shall not affect the current execution
#   environment."
# Duplicate of SHALL-19-09-01-02-003 — same requirement.
# Verifies: prefix assignments to external commands are temporary.

result=$("$SHELL" -c '
unset TMP_V
TMP_V=yes sh -c "printf \"%s\n\" \"\$TMP_V\""
printf "%s\n" "${TMP_V:-gone}"
')
expected='yes
gone'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: prefix assignment should be temporary" >&2
    exit 1
fi

exit 0
