# Test: SHALL-19-14-03-008
# Obligation: "If a specified pattern does not contain any '*', '?' or '['
#   characters that will be treated as special, the pattern string shall be
#   left unchanged."
# (Duplicate of SHALL-19-14-03-004)
# Verifies: non-glob pattern is not expanded.

r=$(printf '%s\n' hello)
if [ "$r" != "hello" ]; then
    printf '%s\n' "FAIL: literal word modified" >&2
    exit 1
fi

r=""
for f in '*'; do r="$r $f"; done
r="${r# }"
if [ "$r" != "*" ]; then
    printf '%s\n' "FAIL: quoted * expanded: [$r]" >&2
    exit 1
fi
exit 0
