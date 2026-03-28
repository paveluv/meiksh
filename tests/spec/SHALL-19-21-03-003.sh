# Test: SHALL-19-21-03-003
# Obligation: "If the exec command fails, a non-interactive shell shall exit
#   from the current shell execution environment"

# exec failure in subshell causes exit from that subshell
result=$(exec /nonexistent_command_$$ 2>/dev/null; printf '%s' "still_here")
if [ "$result" = "still_here" ]; then
    printf '%s\n' "FAIL: exec failure in subshell did not cause exit" >&2
    exit 1
fi

exit 0
