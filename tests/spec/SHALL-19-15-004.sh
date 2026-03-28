# Test: SHALL-19-15-004
# Obligation: "An error in a special built-in utility may cause a shell
#   executing that utility to abort, while an error in a regular built-in
#   utility shall not cause a shell executing that utility to abort."
# (Duplicate of SHALL-19-15-002)
# Verifies: regular built-in error does not abort shell.

result=$(test -f /nonexistent 2>/dev/null; printf '%s\n' "ok")
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: shell aborted after regular built-in error" >&2
    exit 1
fi
exit 0
