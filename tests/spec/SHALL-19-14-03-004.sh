# Test: SHALL-19-14-03-004
# Obligation: "If a specified pattern does not contain any '*', '?' or '['
#   characters that will be treated as special, the pattern string shall be
#   left unchanged."
# Verifies: a word with no special glob characters is not expanded.

mkdir -p "$TMPDIR/noglob"
: > "$TMPDIR/noglob/hello"
cd "$TMPDIR/noglob"

# "hello" with no glob chars -> left as-is (happens to match a file but
# this is just a literal word, no glob expansion attempted)
result=$(printf '%s\n' hello)
if [ "$result" != "hello" ]; then
    printf '%s\n' "FAIL: literal word was modified" >&2
    exit 1
fi

# Quoted glob characters are not special
result=""
for f in '*'; do
    result="$result $f"
done
result="${result# }"
if [ "$result" != "*" ]; then
    printf '%s\n' "FAIL: quoted * was expanded: [$result]" >&2
    exit 1
fi

rm -rf "$TMPDIR/noglob"
exit 0
