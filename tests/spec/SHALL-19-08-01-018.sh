# Test: SHALL-19-08-01-018
# Obligation: "Non-Interactive Shell: Expansion error -- shall exit"
# Verifies: non-interactive shell exits on expansion error.

result=$("$SHELL" -c 'unset X; echo ${X?err}; echo ALIVE' 2>/dev/null)
case "$result" in
    *ALIVE*)
        printf '%s\n' "FAIL: shell did not exit after expansion error" >&2
        exit 1
        ;;
esac

exit 0
