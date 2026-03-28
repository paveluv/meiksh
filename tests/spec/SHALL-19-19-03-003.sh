# Test: SHALL-19-19-03-003
# Obligation: "The dot special built-in shall support XBD 12.2 Utility Syntax
#   Guidelines, except for Guidelines 1 and 2."

# Dot command works (basic syntax check - it's a single-char command name)
tmpfile="$TMPDIR/dot_syntax_$$.sh"
printf '%s\n' 'DOT_SYN=ok' > "$tmpfile"
. "$tmpfile"
rm -f "$tmpfile"
if [ "$DOT_SYN" != "ok" ]; then
    printf '%s\n' "FAIL: dot command did not work" >&2
    exit 1
fi

exit 0
