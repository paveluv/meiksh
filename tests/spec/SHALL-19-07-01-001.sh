# Test: SHALL-19-07-01-001
# Obligation: "Input redirection shall cause the file whose name results from
#   the expansion of word to be opened for reading on the designated file
#   descriptor, or standard input if the file descriptor is not specified."
# Verifies: < opens file for reading on stdin (or specified fd).

f="$TMPDIR/shall_19_07_01_001_$$"
printf '%s\n' "input_data" >"$f"

# Default: read from stdin
result=$(cat <"$f")
if [ "$result" != "input_data" ]; then
    printf '%s\n' "FAIL: <file did not provide input: got '$result'" >&2
    rm -f "$f"
    exit 1
fi

# Explicit fd
result2=$(cat 0<"$f")
if [ "$result2" != "input_data" ]; then
    printf '%s\n' "FAIL: 0<file did not provide input: got '$result2'" >&2
    rm -f "$f"
    exit 1
fi

rm -f "$f"
exit 0
