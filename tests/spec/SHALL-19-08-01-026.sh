# Test: SHALL-19-08-01-026
# Obligation: "If any of the errors shown as \"shall exit\" ... occur in a
#   subshell environment, the shell shall ... exit from the subshell
#   environment with a non-zero status and continue in the environment from
#   which that subshell environment was invoked."
# Verifies: fatal errors in subshell kill only the subshell; parent continues.

result=$("$SHELL" -c '(readonly X=1; X=2) 2>/dev/null; echo ALIVE')
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: parent shell should survive subshell error" >&2
        exit 1
        ;;
esac

# Check subshell returned non-zero
"$SHELL" -c '(exit 0; if then fi) 2>/dev/null; exit $?'
status=$?
if [ "$status" -eq 0 ]; then
    printf '%s\n' "FAIL: subshell with syntax error should exit non-zero" >&2
    exit 1
fi

exit 0
