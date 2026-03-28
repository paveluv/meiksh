# Test: SHALL-19-15-003
# Obligation: "variable assignments preceding the invocation of a special
#   built-in utility affect the current execution environment; this shall not
#   be the case with a regular built-in or other utility."
# Verifies: prefix assignments persist for special built-ins but not for regular.

# Special built-in (export): prefix assignment persists
unset SBI_VAR
SBI_VAR=persisted :
if [ "$SBI_VAR" != "persisted" ]; then
    printf '%s\n' "FAIL: prefix assignment to : did not persist" >&2
    exit 1
fi

# Regular built-in / external command: prefix assignment does NOT persist
unset REG_VAR
REG_VAR=temporary true
if [ -n "$REG_VAR" ]; then
    printf '%s\n' "FAIL: prefix assignment to true persisted: $REG_VAR" >&2
    exit 1
fi

# External command: prefix assignment does NOT persist
unset EXT_VAR
EXT_VAR=temporary sh -c 'exit 0'
if [ -n "$EXT_VAR" ]; then
    printf '%s\n' "FAIL: prefix assignment to sh persisted: $EXT_VAR" >&2
    exit 1
fi

exit 0
