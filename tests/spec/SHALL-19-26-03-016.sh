# Test: SHALL-19-26-03-016
# Obligation: "The shell shall write to standard error a trace for each command
#   after it expands the command and before it executes it."

# set -x produces trace output on stderr
err=$( (set -x; printf '%s' "traced" >/dev/null) 2>&1)
case "$err" in
    *printf*traced*|*'+ printf'*)
        ;;
    *)
        printf '%s\n' "FAIL: set -x did not produce trace on stderr, got: $err" >&2
        exit 1
        ;;
esac

exit 0
