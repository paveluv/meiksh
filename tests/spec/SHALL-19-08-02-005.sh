# Test: SHALL-19-08-02-005
# Obligation: "Otherwise, the exit status shall be the value obtained by the
#   equivalent of the WEXITSTATUS macro applied to the status obtained by
#   the wait() function."
# Verifies: normal exit status is value mod 256.

"$SHELL" -c 'exit 0'
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: exit 0 did not produce status 0" >&2
    exit 1
fi

"$SHELL" -c 'exit 255'
if [ $? -ne 255 ]; then
    printf '%s\n' "FAIL: exit 255 did not produce status 255" >&2
    exit 1
fi

"$SHELL" -c 'exit 1'
if [ $? -ne 1 ]; then
    printf '%s\n' "FAIL: exit 1 did not produce status 1" >&2
    exit 1
fi

exit 0
