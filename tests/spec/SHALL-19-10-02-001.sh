# Test: SHALL-19-10-02-001
# Obligation: "When the TOKEN is exactly a reserved word, the token identifier
#   for that reserved word shall result. Otherwise, the token WORD shall be
#   returned. ... quoted strings cannot be recognized as reserved words."
# Verifies: Reserved words not recognized when quoted.

# Quoted 'if' should be treated as a command word, not reserved
# This should fail with "command not found", not a syntax error
result=$("if" 2>&1)
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s\n' "FAIL: quoted 'if' should not be a valid command" >&2
    exit 1
fi

# Backslash-quoted reserved word should not be recognized
result=$(\if 2>&1)
rc=$?
if [ "$rc" -eq 0 ]; then
    printf '%s\n' "FAIL: backslash-quoted if should not be recognized as reserved" >&2
    exit 1
fi

exit 0
