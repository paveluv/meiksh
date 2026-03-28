# Test: SHALL-19-09-03-08-002
# Obligation: "First, command1 shall be executed. If its exit status is non-zero,
#   command2 shall be executed, and so on, until a command has a zero exit status
#   or there are no more commands left to execute."
# Verifies: || short-circuit evaluation.

# Success stops chain
result=""
true || result="ran"
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: command after true || should not run" >&2
    exit 1
fi

# Failure continues chain
result=""
false || result="ran"
if [ "$result" != "ran" ]; then
    printf '%s\n' "FAIL: command after false || should run" >&2
    exit 1
fi

exit 0
