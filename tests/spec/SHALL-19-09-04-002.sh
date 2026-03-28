# Test: SHALL-19-09-04-002
# Obligation: "The exit status of a compound-list shall be the value that the
#   special parameter '?' ... would have immediately after execution of the
#   compound-list."
# Verifies: Compound-list exit status equals $? of last command.

{ true; false; }
rc=$?
if [ "$rc" -ne 1 ]; then
    printf '%s\n' "FAIL: compound-list exit should be 1, got $rc" >&2
    exit 1
fi

{ false; true; }
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: compound-list exit should be 0, got $rc" >&2
    exit 1
fi

exit 0
