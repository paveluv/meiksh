# SHALL-19-04-002
# "This recognition shall only occur when none of the characters is quoted and
#  when the word is used as:"
# Verify that quoting prevents reserved word recognition.

fail=0

# Quoted 'if' should be treated as a command name, not a reserved word
eval '"if"' >/dev/null 2>&1
# Should fail because "if" as a command doesn't exist (not recognized as keyword)
rc=$?
[ $rc -ne 0 ] || { printf '%s\n' "FAIL: quoted 'if' was recognized as reserved word" >&2; fail=1; }

# Backslash-quoted
eval '\if' >/dev/null 2>&1
rc=$?
[ $rc -ne 0 ] || { printf '%s\n' "FAIL: backslash-quoted if was recognized as reserved word" >&2; fail=1; }

# Single-quoted
eval "'if'" >/dev/null 2>&1
rc=$?
[ $rc -ne 0 ] || { printf '%s\n' "FAIL: single-quoted if recognized as reserved word" >&2; fail=1; }

exit "$fail"
