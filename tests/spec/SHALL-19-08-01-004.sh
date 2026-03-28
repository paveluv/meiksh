# Test: SHALL-19-08-01-004
# Obligation: "Non-Interactive Shell: Special built-in utility error -- shall exit"
# Verifies: non-interactive shell exits on special built-in error.

# 'shift' with too large an argument is a special built-in error
result=$("$SHELL" -c 'shift 999; echo ALIVE' 2>/dev/null)
case "$result" in
    *ALIVE*)
        printf '%s\n' "FAIL: shell did not exit after special builtin error (shift)" >&2
        exit 1
        ;;
esac

exit 0
