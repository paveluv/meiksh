# Test: SHALL-04-16-002
# Obligation: "If the pathname begins with a <slash>, the predecessor of the
#   first filename in the pathname shall be taken to be the root directory of
#   the process. If the pathname does not begin with a <slash>, the predecessor
#   of the first filename of the pathname shall be taken to be [...] the
#   current working directory of the process."
# Verifies: Absolute paths resolve from root, relative paths resolve from CWD.

# Test absolute path: /tmp or TMPDIR should be accessible as absolute
if [ ! -d "$TMPDIR" ]; then
    printf 'FAIL: TMPDIR is not accessible as absolute path\n' >&2
    exit 1
fi

# Create a subdir and file for relative path test
mkdir "$TMPDIR/subdir"
: > "$TMPDIR/subdir/relfile"

# Test relative path resolution from CWD
cd "$TMPDIR"
if [ ! -f "subdir/relfile" ]; then
    printf 'FAIL: relative path subdir/relfile not found from CWD=%s\n' "$TMPDIR" >&2
    exit 1
fi

# Test that same file is accessible via absolute path
if [ ! -f "$TMPDIR/subdir/relfile" ]; then
    printf 'FAIL: absolute path to relfile not found\n' >&2
    exit 1
fi

# Change CWD and verify relative resolution changes
cd "$TMPDIR/subdir"
if [ ! -f "relfile" ]; then
    printf 'FAIL: relative path relfile not found from CWD=%s/subdir\n' "$TMPDIR" >&2
    exit 1
fi

# From subdir, the old relative path should not work
cd "$TMPDIR/subdir"
if [ -f "subdir/relfile" ] 2>/dev/null; then
    printf 'FAIL: relative path subdir/relfile should not resolve from subdir\n' >&2
    exit 1
fi

exit 0
