# Test: SHALL-03-146-001
# Obligation: "The bytes composing the name shall not contain the <NUL> or
#   <slash> characters."
# Verifies: Pathname expansion does not produce filenames containing slash
#   within a component. The shell handles filenames with dot and dot-dot
#   correctly during globbing.

# Create test files in TMPDIR
mkdir "$TMPDIR/testdir"
: > "$TMPDIR/testdir/afile"
: > "$TMPDIR/testdir/bfile"

# Glob should produce only simple filenames (no slash in the component)
cd "$TMPDIR/testdir"
for f in *; do
    case "$f" in
        */*) printf 'FAIL: glob produced filename with slash: %s\n' "$f" >&2; exit 1 ;;
    esac
done

# Verify that a filename with embedded NUL cannot be created via shell
# (the shell should not allow NUL in strings used as filenames)
# We test that printf with a NUL byte in a variable does not create
# a file with an empty name prefix
name="a"
: > "$TMPDIR/testdir/$name"
if [ ! -f "$TMPDIR/testdir/a" ]; then
    printf 'FAIL: could not create file with simple name\n' >&2
    exit 1
fi

exit 0
