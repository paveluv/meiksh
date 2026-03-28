# Test: SHALL-19-08-01-010
# Obligation: "Non-Interactive Shell: Redirection error with compound commands
#   -- shall not exit"
# Verifies: non-interactive shell does NOT exit on redirection error with
#   compound command.

result=$("$SHELL" -c '{ echo inside; } > /no/such/dir/file 2>/dev/null; echo ALIVE')
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: shell exited after redir error on compound command" >&2
        exit 1
        ;;
esac

exit 0
