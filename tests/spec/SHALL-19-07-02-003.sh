# Test: SHALL-19-07-02-003
# Obligation: "output redirection shall cause the file [...] to be opened for
#   output [...] If the file does not exist, it shall be created as an empty
#   file; otherwise, it shall be opened as if [...] with the O_TRUNC flag set."
# Verifies: > creates new file or truncates existing.

f="$TMPDIR/shall_19_07_02_003_$$"
rm -f "$f"

# Create new file
printf '%s\n' "new" >"$f"
if [ ! -f "$f" ]; then
    printf '%s\n' "FAIL: > did not create file" >&2
    exit 1
fi
content=$(cat "$f")
if [ "$content" != "new" ]; then
    printf '%s\n' "FAIL: > new file content: got '$content'" >&2
    rm -f "$f"
    exit 1
fi

# Truncate existing file
printf '%s\n' "replaced" >"$f"
content2=$(cat "$f")
if [ "$content2" != "replaced" ]; then
    printf '%s\n' "FAIL: > did not truncate: got '$content2'" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
