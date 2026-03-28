# Test: SHALL-19-08-01-014
# Obligation: "Non-Interactive Shell: Redirection error with other utilities
#   (not special built-ins) -- shall not exit"
# Verifies: non-interactive shell does NOT exit on redirection error with
#   regular utility.

result=$("$SHELL" -c 'echo hello > /no/such/dir/file 2>/dev/null; echo ALIVE')
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: shell exited after redir error on regular utility" >&2
        exit 1
        ;;
esac

exit 0
