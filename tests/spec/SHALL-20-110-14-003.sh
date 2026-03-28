# SHALL-20-110-14-003
# "The following exit values shall be returned:: The script to be executed
#  consisted solely of zero or more blank lines or comments, or both."
# Verifies: sh exits 0 for empty scripts and comment-only scripts.

# Empty script (zero bytes)
f="$TMPDIR/empty_$$.sh"
: > "$f"
"$MEIKSH" "$f"
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: empty script exited $rc, expected 0" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

# Blank lines only
f="$TMPDIR/blanks_$$.sh"
printf '\n\n\n' > "$f"
"$MEIKSH" "$f"
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: blank-lines-only script exited $rc, expected 0" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

# Comments only
f="$TMPDIR/comments_$$.sh"
printf '# comment one\n# comment two\n' > "$f"
"$MEIKSH" "$f"
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: comment-only script exited $rc, expected 0" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

# Mix of blank lines and comments
f="$TMPDIR/mixed_$$.sh"
printf '\n# comment\n\n# another\n\n' > "$f"
"$MEIKSH" "$f"
rc=$?
if [ "$rc" -ne 0 ]; then
    printf '%s\n' "FAIL: mixed blank/comment script exited $rc, expected 0" >&2
    rm -f "$f"
    exit 1
fi
rm -f "$f"

exit 0
