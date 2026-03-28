# Test: SHALL-19-14-03-003
# Obligation: "If a specified pattern contains any '*', '?' or '[' characters
#   that will be treated as special, it shall be matched against existing
#   filenames and pathnames... If the pattern matches any existing filenames or
#   pathnames, the pattern shall be replaced with those filenames and pathnames,
#   sorted according to the collating sequence... If the pattern does not match
#   any existing filenames or pathnames, the pattern string shall be left
#   unchanged."
# Verifies: glob matches existing files sorted; no-match leaves pattern literal.

mkdir -p "$TMPDIR/globtest"
: > "$TMPDIR/globtest/aaa"
: > "$TMPDIR/globtest/bbb"
: > "$TMPDIR/globtest/ccc"

cd "$TMPDIR/globtest"

# Pattern matches files, result is sorted
result=""
for f in *; do
    result="$result $f"
done
result="${result# }"
if [ "$result" != "aaa bbb ccc" ]; then
    printf '%s\n' "FAIL: glob result not sorted: [$result]" >&2
    exit 1
fi

# No-match: pattern left unchanged
result=""
for f in zzz_no_match_*; do
    result="$result $f"
done
result="${result# }"
if [ "$result" != "zzz_no_match_*" ]; then
    printf '%s\n' "FAIL: no-match glob was not left unchanged: [$result]" >&2
    exit 1
fi

rm -rf "$TMPDIR/globtest"
exit 0
