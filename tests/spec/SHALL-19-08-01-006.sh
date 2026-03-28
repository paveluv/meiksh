# Test: SHALL-19-08-01-006
# Obligation: "Non-Interactive Shell: Other utility (not a special built-in)
#   error -- shall not exit"
# Verifies: non-interactive shell does NOT exit on regular utility error.

result=$("$SHELL" -c 'ls /nonexistent_path_xyzzy 2>/dev/null; echo ALIVE' 2>/dev/null)
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: shell exited after non-special utility error" >&2
        exit 1
        ;;
esac

exit 0
