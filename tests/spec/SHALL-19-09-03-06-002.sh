# Test: SHALL-19-09-03-06-002
# Obligation: "First command1 shall be executed. If its exit status is zero,
#   command2 shall be executed ... until a command has a non-zero exit status or
#   there are no more commands left to execute. The commands are expanded only
#   if they are executed."
# Verifies: && short-circuit and deferred expansion.

# Short-circuit: false stops chain
result=""
false && result="ran"
if [ -n "$result" ]; then
    printf '%s\n' "FAIL: command after false && should not run" >&2
    exit 1
fi

# Success continues chain
result=""
true && result="ran"
if [ "$result" != "ran" ]; then
    printf '%s\n' "FAIL: command after true && should run" >&2
    exit 1
fi

# Expansion deferred: variable not expanded when skipped
unset SKIPVAR
false && SKIPVAR=$(printf '%s' "expanded")
if [ -n "$SKIPVAR" ]; then
    printf '%s\n' "FAIL: skipped command was expanded" >&2
    exit 1
fi

exit 0
