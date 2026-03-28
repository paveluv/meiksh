# Test: SHALL-19-08-02-003
# Obligation: "Otherwise, if the command name is found, but it is not an
#   executable utility, the exit status shall be 126."
# Verifies: exit status 126 for found-but-not-executable file.

f="$TMPDIR/not_exec_$$"
printf '%s\n' '#!/bin/sh' 'echo hello' > "$f"
chmod -x "$f"
"$SHELL" -c "'$f'" 2>/dev/null
status=$?
rm -f "$f"
if [ "$status" -ne 126 ]; then
    printf '%s\n' "FAIL: not-executable exit status is $status, expected 126" >&2
    exit 1
fi

exit 0
