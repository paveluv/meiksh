# Test: SHALL-19-21-03-002
# Obligation: "If exec is specified with a utility operand, the shell shall
#   execute a non-built-in utility ... with utility as the command name and the
#   argument operands (if any) as the command arguments."

# exec with utility replaces shell process (test in subshell)
result=$(exec printf '%s' "replaced")
if [ "$result" != "replaced" ]; then
    printf '%s\n' "FAIL: exec with utility did not execute command" >&2
    exit 1
fi

exit 0
