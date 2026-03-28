# Test: SHALL-19-08-01-023
# Obligation: "The shell shall exit only if the special built-in utility is
#   executed directly. If it is executed via the command utility, the shell
#   shall not exit."
# Verifies: 'command' suppresses special built-in exit-on-error.

result=$("$SHELL" -c 'command shift 999 2>/dev/null; echo ALIVE')
case "$result" in
    *ALIVE*)
        ;;
    *)
        printf '%s\n' "FAIL: 'command shift' should not cause shell to exit" >&2
        exit 1
        ;;
esac

exit 0
