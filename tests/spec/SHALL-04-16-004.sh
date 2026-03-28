# Test: SHALL-04-16-004
# Obligation: "A pathname that contains at least one non-<slash> character and
#   that ends with one or more trailing <slash> characters shall not be resolved
#   successfully unless the last pathname component before the trailing <slash>
#   characters resolves [...] to an existing directory."
# Verifies: Trailing slash on a non-directory pathname causes failure.

# Create a regular file
: > "$TMPDIR/regularfile"

# Attempting to access regularfile/ should fail
if [ -e "$TMPDIR/regularfile/" ] 2>/dev/null; then
    printf 'FAIL: regularfile/ should not resolve (not a directory)\n' >&2
    exit 1
fi

# Create a directory and verify trailing slash works
mkdir "$TMPDIR/realdir"
if [ ! -d "$TMPDIR/realdir/" ]; then
    printf 'FAIL: realdir/ should resolve successfully\n' >&2
    exit 1
fi

# cd to a regular file with trailing slash should fail
(cd "$TMPDIR/regularfile/" 2>/dev/null) && {
    printf 'FAIL: cd to regularfile/ should have failed\n' >&2
    exit 1
}

exit 0
