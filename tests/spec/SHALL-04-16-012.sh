# Test: SHALL-04-16-012
# Obligation: "A pathname consisting of a single <slash> shall resolve to the
#   root directory of the process. A null pathname shall not be successfully
#   resolved. [...] more than two leading <slash> characters shall be treated
#   as a single <slash> character."
# Verifies: Single slash resolves to root, null pathname fails, and triple
#   (or more) leading slashes are treated as a single slash.

# Test 1: Single slash resolves to root
if [ ! -d "/" ]; then
    printf 'FAIL: / should resolve to root directory\n' >&2
    exit 1
fi

cd /
result=$(pwd -P)
if [ "$result" != "/" ]; then
    printf 'FAIL: cd / should set PWD to /, got %s\n' "$result" >&2
    exit 1
fi

# Test 2: Null pathname should fail
# An empty variable used as a pathname should cause an error
empty=""
if [ -e "$empty" ] 2>/dev/null; then
    # Some shells return false for test -e "", which is acceptable
    # The key point is that it doesn't resolve to something real
    :
fi

(cd "" 2>/dev/null) && {
    printf 'FAIL: cd with empty pathname should have failed\n' >&2
    exit 1
}

# Test 3: Triple slash treated as single slash
# ///tmp should resolve the same as /tmp (if /tmp exists)
if [ -d "/tmp" ]; then
    if [ ! -d "///tmp" ]; then
        printf 'FAIL: ///tmp should resolve like /tmp\n' >&2
        exit 1
    fi
fi

# Multiple leading slashes (3+) should resolve to root
cd ///
result=$(pwd -P)
if [ "$result" != "/" ]; then
    printf 'FAIL: cd /// should resolve to /, got %s\n' "$result" >&2
    exit 1
fi

exit 0
