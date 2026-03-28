# Test: SHALL-19-14-03-007
# Obligation: "If a specified pattern contains any '*', '?' or '[' characters
#   that will be treated as special, it shall be matched against existing
#   filenames... If the pattern does not match, the pattern string shall be
#   left unchanged."
# (Duplicate of SHALL-19-14-03-003)
# Verifies: glob matches existing files; no-match leaves pattern unchanged.

mkdir -p "$TMPDIR/globdup"
: > "$TMPDIR/globdup/xx"
: > "$TMPDIR/globdup/yy"
cd "$TMPDIR/globdup"

r=""
for f in *; do r="$r $f"; done
r="${r# }"
if [ "$r" != "xx yy" ]; then
    printf '%s\n' "FAIL: glob result [$r] != [xx yy]" >&2
    exit 1
fi

r=""
for f in zzz_*; do r="$r $f"; done
r="${r# }"
if [ "$r" != "zzz_*" ]; then
    printf '%s\n' "FAIL: no-match not preserved: [$r]" >&2
    exit 1
fi

rm -rf "$TMPDIR/globdup"
exit 0
