# Test: SHALL-19-08-01-016
# Obligation: "Non-Interactive Shell: Variable assignment error -- shall exit"
# Verifies: non-interactive shell exits on variable assignment error
#   (assigning to readonly variable).

result=$("$SHELL" -c 'readonly X=1; X=2; echo ALIVE' 2>/dev/null)
case "$result" in
    *ALIVE*)
        printf '%s\n' "FAIL: shell did not exit after readonly assignment error" >&2
        exit 1
        ;;
esac

exit 0
