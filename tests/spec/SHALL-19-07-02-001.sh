# Test: SHALL-19-07-02-001
# Obligation: "If the number is omitted, the redirection shall refer to
#   standard output (file descriptor 1)."
# Verifies: omitted fd number in output redirect defaults to fd 1.

f="$TMPDIR/shall_19_07_02_001_$$"
printf '%s\n' "stdout_data" >"$f"
content=$(cat "$f")
if [ "$content" != "stdout_data" ]; then
    printf '%s\n' "FAIL: >file without fd did not redirect stdout" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
