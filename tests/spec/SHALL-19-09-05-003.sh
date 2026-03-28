# Test: SHALL-19-09-05-003
# Obligation: "When a function is executed, it shall have the syntax-error
#   properties described for special built-in utilities"
# Verifies: Syntax error in function body causes non-interactive shell to exit.

# Run in a subshell to avoid killing ourselves
msg=$(
    eval 'badsyntax() { if; }' 2>&1
)
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s\n' "FAIL: syntax error in function should cause nonzero exit" >&2
    exit 1
fi

exit 0
