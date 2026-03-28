# Test: SHALL-19-19-03-002
# Obligation: "If file does not contain a <slash>, the shell shall use the
#   search path specified by PATH to find the directory containing file.
#   Unlike normal command search, however, the file searched for by the dot
#   utility need not be executable."

# Create a non-executable file in a PATH directory
tmpdir="$TMPDIR/dot_path_test_$$"
mkdir -p "$tmpdir"
printf '%s\n' 'DOT_PATH_VAR=found_it' > "$tmpdir/dot_test_file"
chmod 644 "$tmpdir/dot_test_file"

OLD_PATH="$PATH"
PATH="$tmpdir:$PATH"
. dot_test_file
PATH="$OLD_PATH"
rm -rf "$tmpdir"

if [ "$DOT_PATH_VAR" != "found_it" ]; then
    printf '%s\n' "FAIL: dot did not find non-executable file via PATH" >&2
    exit 1
fi

exit 0
