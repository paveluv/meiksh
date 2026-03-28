# Test: SHALL-19-06-010
# Obligation: "When the expansions in this section are performed other than in
#   the context of preparing a command for execution, they shall be carried out
#   in the current shell execution environment."
# Verifies: expansion side effects in non-command contexts persist in current shell.

unset myvar
# Assignment context (not a command): ${var:=default} should persist
: ${myvar:=persisted}
if [ "$myvar" != "persisted" ]; then
    printf '%s\n' "FAIL: \${myvar:=persisted} in assignment context did not persist" >&2
    exit 1
fi

# Arithmetic in assignment context: side effect persists
unset counter
: $((counter = 42))
if [ "$counter" != "42" ]; then
    printf '%s\n' "FAIL: arithmetic assignment in non-command context did not persist" >&2
    exit 1
fi

exit 0
