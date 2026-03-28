# Test: SHALL-19-13-009
# Obligation: "If the shell is interactive, the subshell shall behave as a
#   non-interactive shell in all respects"
# (Duplicate of SHALL-19-13-007)
# Verifies: subshell treats syntax error as fatal (non-interactive behavior).

result=$(eval 'if' 2>/dev/null; printf '%s\n' "survived")
if [ "$result" = "survived" ]; then
    printf '%s\n' "FAIL: subshell continued after syntax error" >&2
    exit 1
fi
exit 0
