# Test: SHALL-19-08-01-008
# Obligation: "Non-Interactive Shell: Redirection error with special built-in
#   utilities -- shall exit"
# Verifies: non-interactive shell exits on redirection error with special builtin.

result=$("$SHELL" -c ': > /no/such/dir/file; echo ALIVE' 2>/dev/null)
case "$result" in
    *ALIVE*)
        printf '%s\n' "FAIL: shell did not exit after redir error on special builtin" >&2
        exit 1
        ;;
esac

exit 0
