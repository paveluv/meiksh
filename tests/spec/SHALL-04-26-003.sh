# Test: SHALL-04-26-003
# Obligation: "When a variable assignment is done, the variable shall be
#   created if it did not already exist. If value is not specified, the
#   variable shall be given a null value."
# Verifies: Assignment auto-creates variables and empty assignment gives
#   null (empty) value distinct from unset.

# Auto-creation: variable does not exist, then does after assignment
unset NEWVAR 2>/dev/null
NEWVAR=created
if [ "$NEWVAR" != "created" ]; then
    echo "FAIL: NEWVAR should be 'created', got '$NEWVAR'" >&2
    exit 1
fi

# Null value: VAR= sets variable to empty string
NULLVAR=
if [ "${NULLVAR+set}" != "set" ]; then
    echo "FAIL: NULLVAR= should set variable (should be 'set')" >&2
    exit 1
fi
if [ -n "$NULLVAR" ]; then
    echo "FAIL: NULLVAR= should give null value, got '$NULLVAR'" >&2
    exit 1
fi

# Distinction: set-to-null vs unset
unset UNSETVAR 2>/dev/null
if [ "${UNSETVAR+set}" = "set" ]; then
    echo "FAIL: UNSETVAR should be unset" >&2
    exit 1
fi

# ${var-default} returns default for unset, empty for null
unset U 2>/dev/null
N=
if [ "${U-fallback}" != "fallback" ]; then
    echo "FAIL: unset var should use default" >&2
    exit 1
fi
if [ "${N-fallback}" != "" ]; then
    echo "FAIL: null var should return empty, not default" >&2
    exit 1
fi

exit 0
