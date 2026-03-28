# Test: SHALL-19-19-03-001
# Obligation: "The shell shall tokenize the contents of the file, parse the
#   tokens, and execute the resulting commands in the current environment."

# Dot sources a file in the current environment
tmpfile="$TMPDIR/dot_test_$$.sh"
printf '%s\n' 'DOT_VAR=sourced_value' > "$tmpfile"
. "$tmpfile"
rm -f "$tmpfile"
if [ "$DOT_VAR" != "sourced_value" ]; then
    printf '%s\n' "FAIL: dot did not execute in current environment" >&2
    exit 1
fi

exit 0
