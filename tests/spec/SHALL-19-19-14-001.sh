# Test: SHALL-19-19-14-001
# Obligation: "Otherwise, return the value of the last command executed, or a
#   zero exit status if no command is executed."

# Dot returns exit status of last command in file
tmpfile="$TMPDIR/dot_exit_$$.sh"
printf '%s\n' 'true' > "$tmpfile"
. "$tmpfile"
st=$?
rm -f "$tmpfile"
if [ "$st" -ne 0 ]; then
    printf '%s\n' "FAIL: dot file ending with true did not return 0, got $st" >&2
    exit 1
fi

tmpfile="$TMPDIR/dot_exit2_$$.sh"
printf '%s\n' 'false' > "$tmpfile"
. "$tmpfile"
st=$?
rm -f "$tmpfile"
if [ "$st" -eq 0 ]; then
    printf '%s\n' "FAIL: dot file ending with false returned 0" >&2
    exit 1
fi

# Empty file returns 0
tmpfile="$TMPDIR/dot_empty_$$.sh"
printf '' > "$tmpfile"
. "$tmpfile"
st=$?
rm -f "$tmpfile"
if [ "$st" -ne 0 ]; then
    printf '%s\n' "FAIL: dot with empty file did not return 0, got $st" >&2
    exit 1
fi

exit 0
