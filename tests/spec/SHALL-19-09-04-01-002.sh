# Test: SHALL-19-09-04-01-002
# Obligation: "Variable assignments and built-in commands that affect the
#   environment shall not remain in effect after the list finishes."
# Verifies: Subshell environment isolation for builtins.

# export in subshell should not persist
unset SUBVAR
(export SUBVAR=leaked)
if [ -n "$SUBVAR" ]; then
    printf '%s\n' "FAIL: export in subshell leaked to parent" >&2
    exit 1
fi

# Brace group runs in current environment (contrast)
unset BRACEVAR
{ BRACEVAR=set; }
if [ "$BRACEVAR" != "set" ]; then
    printf '%s\n' "FAIL: brace group should run in current environment" >&2
    exit 1
fi

exit 0
