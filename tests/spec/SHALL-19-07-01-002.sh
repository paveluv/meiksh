# Test: SHALL-19-07-01-002
# Obligation: "If the number is omitted, the redirection shall refer to
#   standard input (file descriptor 0)."
# Verifies: omitted fd number in input redirect defaults to fd 0.

f="$TMPDIR/shall_19_07_01_002_$$"
printf '%s\n' "default_stdin" >"$f"

result=$(cat <"$f")
if [ "$result" != "default_stdin" ]; then
    printf '%s\n' "FAIL: < without fd number did not default to stdin" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
