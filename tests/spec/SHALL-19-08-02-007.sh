# Test: SHALL-19-08-02-007
# Obligation: "Otherwise, if the command name is found, but it is not an
#   executable utility, the exit status shall be 126."
# Duplicate of SHALL-19-08-02-003 — same requirement.
# Verifies: exit status 126 for not-executable.

f="$TMPDIR/noexec_dup_$$"
printf '%s\n' 'echo hi' > "$f"
chmod -x "$f"
"$SHELL" -c "'$f'" 2>/dev/null
status=$?
rm -f "$f"
if [ "$status" -ne 126 ]; then
    printf '%s\n' "FAIL: not-executable status is $status, expected 126" >&2
    exit 1
fi

exit 0
