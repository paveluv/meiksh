# Test: SHALL-19-07-07-001
# Obligation: "[n]<>word shall cause the file whose name is the expansion
#   of word to be opened for both reading and writing on the file descriptor
#   denoted by n, or standard input if n is not specified. If the file does
#   not exist, it shall be created."
# Verifies: <> opens for read+write and creates file if absent.

f="$TMPDIR/rw_test_$$"
rm -f "$f"

# File should be created if it does not exist
exec 3<>"$f"
printf '%s\n' "readwrite" >&3
exec 3>&-

content=$(cat "$f")
if [ "$content" != "readwrite" ]; then
    printf '%s\n' "FAIL: <> did not create and write to file" >&2
    exit 1
fi

# Open existing file for read+write (should not truncate)
printf '%s\n' "original" > "$f"
exec 3<>"$f"
read -r line <&3
exec 3>&-
if [ "$line" != "original" ]; then
    printf '%s\n' "FAIL: <> truncated existing file or could not read" >&2
    exit 1
fi

rm -f "$f"
exit 0
