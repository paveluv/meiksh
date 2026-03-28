# Test: SHALL-19-08-01-002
# Obligation: "Non-Interactive Shell: Shell language syntax error -- shall exit"
# Verifies: non-interactive shell exits on syntax error.

"$SHELL" -c 'if then fi; echo SHOULD_NOT_REACH' >/dev/null 2>&1
status=$?
if [ "$status" -eq 0 ]; then
    printf '%s\n' "FAIL: non-interactive shell did not exit on syntax error" >&2
    exit 1
fi

# Ensure the command after the syntax error was NOT executed
result=$("$SHELL" -c 'if then fi; echo REACHED' 2>/dev/null)
case "$result" in
    *REACHED*)
        printf '%s\n' "FAIL: command after syntax error was executed" >&2
        exit 1
        ;;
esac

exit 0
