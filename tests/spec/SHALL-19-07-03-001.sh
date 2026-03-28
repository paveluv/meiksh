# Test: SHALL-19-07-03-001
# Obligation: "Appended output redirection shall cause the file [...] to be
#   opened for output [...] opened as if [...] with the O_APPEND flag set. If
#   the file does not exist, it shall be created."
# Verifies: >> appends without truncating and creates if needed.

f="$TMPDIR/shall_19_07_03_001_$$"
rm -f "$f"

# Create via >>
printf '%s\n' "line1" >>"$f"
if [ ! -f "$f" ]; then
    printf '%s\n' "FAIL: >> did not create file" >&2
    exit 1
fi

# Append
printf '%s\n' "line2" >>"$f"
expected=$(printf 'line1\nline2')
content=$(cat "$f")
if [ "$content" != "$expected" ]; then
    printf '%s\n' "FAIL: >> did not append correctly" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
