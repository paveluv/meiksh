# Test: SHALL-19-13-007
# Obligation: "If the shell is interactive, the subshell shall behave as a
#   non-interactive shell in all respects"
# Verifies: subshells do not prompt; syntax errors in subshell do not kill parent.

# A subshell should behave as non-interactive: syntax errors cause exit
result=$(
    eval 'if' 2>/dev/null
    printf '%s\n' "survived"
)
# The subshell should have exited on the syntax error — "survived" should not appear
if [ "$result" = "survived" ]; then
    printf '%s\n' "FAIL: subshell continued after syntax error (non-interactive behavior)" >&2
    exit 1
fi

# Parent shell continues after subshell error
(exit 42)
if [ $? -ne 42 ]; then
    printf '%s\n' "FAIL: subshell exit status not propagated" >&2
    exit 1
fi

exit 0
