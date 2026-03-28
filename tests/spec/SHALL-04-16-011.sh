# Test: SHALL-04-16-011
# Obligation: "The special filename dot shall refer to the directory specified
#   by its predecessor. The special filename dot-dot shall refer to the parent
#   directory of its predecessor directory. As a special case, in the root
#   directory, dot-dot may refer to the root directory itself."
# Verifies: '.' refers to current directory, '..' refers to parent, and
#   '/.' and '/..' both refer to root.

# Create test structure
mkdir -p "$TMPDIR/parent/child"
: > "$TMPDIR/parent/child/marker"

# Test dot: child/. should be same as child
cd "$TMPDIR/parent/child"
if [ ! -f "./marker" ]; then
    printf 'FAIL: ./marker not found in child directory\n' >&2
    exit 1
fi

# Test dot in path component
if [ ! -f "$TMPDIR/parent/child/./marker" ]; then
    printf 'FAIL: child/./marker not found\n' >&2
    exit 1
fi

# Test dot-dot: from child, .. should be parent
cd "$TMPDIR/parent/child"
cd ..
result=$(pwd -P)
expected=$(cd "$TMPDIR/parent" && pwd -P)
if [ "$result" != "$expected" ]; then
    printf 'FAIL: .. from child should be parent, got %s expected %s\n' "$result" "$expected" >&2
    exit 1
fi

# Test dot-dot at root: /.. should still be /
cd /
cd ..
result=$(pwd -P)
if [ "$result" != "/" ]; then
    printf 'FAIL: /.. should resolve to /, got %s\n' "$result" >&2
    exit 1
fi

exit 0
