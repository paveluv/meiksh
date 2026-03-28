# Test: SHALL-19-10-01-003
# Obligation: "If the string consists solely of digits and the delimiter
#   character is one of '<' or '>', the token identifier IO_NUMBER shall result."
# Verifies: Digit tokens before < or > are IO_NUMBER (fd redirections).

tmpf="$TMPDIR/shall-19-10-01-003.$$"
trap 'rm -f "$tmpf"' EXIT

# 2> redirects fd 2 (stderr)
printf '%s\n' "to_stderr" 2>"$tmpf" >&2
# The file should have the stderr output
content=$(cat "$tmpf" 2>/dev/null)
# We wrote to stderr which was redirected to file
printf '%s\n' "stderr_test" >&2 2>"$tmpf"
content=$(cat "$tmpf")
if [ "$content" != "stderr_test" ]; then
    printf '%s\n' "FAIL: IO_NUMBER 2> not recognized" >&2
    exit 1
fi

exit 0
