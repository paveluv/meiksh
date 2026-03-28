# Test: SHALL-19-08-02-008
# Obligation: "Otherwise, if the command terminated due to the receipt of a
#   signal, the shell shall assign it an exit status greater than 128."
# Duplicate of SHALL-19-08-02-004 — same requirement.
# Verifies: signal death exit status > 128.

"$SHELL" -c 'kill -TERM $$' 2>/dev/null
status=$?
if [ "$status" -le 128 ]; then
    printf '%s\n' "FAIL: signal death status is $status, expected > 128" >&2
    exit 1
fi

exit 0
