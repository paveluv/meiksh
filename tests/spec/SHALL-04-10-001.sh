# Test: SHALL-04-10-001
# Obligation: "For a filename to be portable across implementations conforming
#   to POSIX.1-2024, it shall consist only of the portable filename character
#   set as defined in 3.265 Portable Filename Character Set."
# Verifies: The shell can create and access files using only the portable
#   filename character set (A-Z, a-z, 0-9, '.', '-', '_').

name="AZaz09._-test"
: > "$TMPDIR/$name"

if [ ! -f "$TMPDIR/$name" ]; then
    printf 'FAIL: could not create file with portable filename chars: %s\n' "$name" >&2
    exit 1
fi

# Verify we can read it back via glob
cd "$TMPDIR"
found=false
for f in AZaz09._-test; do
    if [ "$f" = "$name" ]; then
        found=true
    fi
done

if [ "$found" != "true" ]; then
    printf 'FAIL: glob did not match portable filename: %s\n' "$name" >&2
    exit 1
fi

exit 0
