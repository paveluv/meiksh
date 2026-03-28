# Test: SHALL-19-09-03-02-003
# Obligation: "The process ID associated with the asynchronous AND-OR list shall
#   become known in the current shell execution environment"
# Verifies: $! is set after backgrounding a command.

true &
if [ -z "$!" ]; then
    printf '%s\n' "FAIL: \$! not set after backgrounding" >&2
    exit 1
fi
wait

# $! should be a numeric PID
case "$!" in
    *[!0-9]*) printf '%s\n' "FAIL: \$! is not numeric: $!" >&2; exit 1 ;;
esac

exit 0
