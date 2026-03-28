# Test: SHALL-19-09-01-02-010
# Obligation: "If the command name is a special built-in utility, variable
#   assignments shall affect the current execution environment before the
#   utility is executed and remain in effect when the command completes"
# Duplicate of SHALL-19-09-01-02-005 — same requirement.
# Verifies: prefix assignments to special builtins persist.

result=$("$SHELL" -c 'V=hi eval "printf \"%s\n\" \"\$V\""; printf "%s\n" "$V"')
expected='hi
hi'
if [ "$result" != "$expected" ]; then
    printf '%s\n' "FAIL: special builtin assignment should persist" >&2
    exit 1
fi

exit 0
