# Test: SHALL-19-08-01-012
# Obligation: "Non-Interactive Shell: Redirection error with function
#   execution -- shall not exit"
# Verifies: non-interactive shell does NOT exit on redirection error with
#   function call.

result=$("$SHELL" -c 'f() { echo inside; }; f > /no/such/dir/file 2>/dev/null; echo ALIVE')
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: shell exited after redir error on function" >&2
        exit 1
        ;;
esac

exit 0
