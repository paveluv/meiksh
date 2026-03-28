# Test: SHALL-19-08-02-009
# Obligation: "Otherwise, the exit status shall be the value obtained by the
#   equivalent of the WEXITSTATUS macro applied to the status obtained by
#   the wait() function."
# Duplicate of SHALL-19-08-02-005 — same requirement.
# Verifies: normal exit returns WEXITSTATUS.

"$SHELL" -c 'exit 99'
if [ $? -ne 99 ]; then
    printf '%s\n' "FAIL: exit 99 did not produce status 99" >&2
    exit 1
fi

exit 0
