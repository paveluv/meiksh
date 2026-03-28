# Test: SHALL-20-110-07-001
# Obligation: "The input file can be of any type, but the initial portion of
#   the file intended to be parsed according to the shell grammar shall consist
#   of characters and shall not contain the NUL character. The shell shall not
#   enforce any line length limits. If the input file consists solely of zero
#   or more blank lines and comments, sh shall exit with a zero exit status."
# Verifies: Empty script exits 0, comment-only exits 0, and long lines work.

# Test 1: empty script exits 0
: > "$TMPDIR/empty.sh"
"$MEIKSH" "$TMPDIR/empty.sh"
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: empty script did not exit 0" >&2
    exit 1
fi

# Test 2: blank-lines-only script exits 0
printf '\n\n\n' > "$TMPDIR/blanks.sh"
"$MEIKSH" "$TMPDIR/blanks.sh"
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: blank-lines-only script did not exit 0" >&2
    exit 1
fi

# Test 3: comment-only script exits 0
printf '# just a comment\n# another\n' > "$TMPDIR/comments.sh"
"$MEIKSH" "$TMPDIR/comments.sh"
if [ $? -ne 0 ]; then
    printf '%s\n' "FAIL: comment-only script did not exit 0" >&2
    exit 1
fi

# Test 4: long line (no line length limit enforced)
long_var=$(printf '%04000d' 0 | tr '0' 'x')
printf 'v=%s; printf "%%s\\n" "${#v}"\n' "$long_var" > "$TMPDIR/longline.sh"
result=$("$MEIKSH" "$TMPDIR/longline.sh")
if [ "$result" != "4000" ]; then
    printf '%s\n' "FAIL: long line not handled, got '$result'" >&2
    exit 1
fi

exit 0
