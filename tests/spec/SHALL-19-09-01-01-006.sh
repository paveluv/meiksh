# Test: SHALL-19-09-01-01-006
# Obligation: "The first word (if any) that is not a variable assignment or
#   redirection shall be expanded..."
# Duplicate of SHALL-19-09-01-01-003 — same requirement.
# Verifies: command name determined from first expanded non-assignment word.

result=$("$SHELL" -c 'C=printf; $C "%s\n" "ok"')
if [ "$result" != "ok" ]; then
    printf '%s\n' "FAIL: expanded word not used as command name" >&2
    exit 1
fi

exit 0
