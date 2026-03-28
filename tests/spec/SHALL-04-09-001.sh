# Test: SHALL-04-09-001
# Obligation: "Uppercase and lowercase letters shall retain their unique
#   identities between conforming implementations."
# Verifies: The shell treats uppercase and lowercase filenames as distinct
#   when creating, testing, and globbing files.

mkdir "$TMPDIR/casedir"

: > "$TMPDIR/casedir/File"
: > "$TMPDIR/casedir/file"
: > "$TMPDIR/casedir/FILE"

count=0
for f in "$TMPDIR/casedir"/*; do
    count=$((count + 1))
done

if [ "$count" -ne 3 ]; then
    printf 'FAIL: expected 3 distinct files, got %d (filesystem may be case-insensitive)\n' "$count" >&2
    exit 1
fi

if [ ! -f "$TMPDIR/casedir/File" ]; then
    printf 'FAIL: File not found\n' >&2
    exit 1
fi
if [ ! -f "$TMPDIR/casedir/file" ]; then
    printf 'FAIL: file not found\n' >&2
    exit 1
fi
if [ ! -f "$TMPDIR/casedir/FILE" ]; then
    printf 'FAIL: FILE not found\n' >&2
    exit 1
fi

exit 0
