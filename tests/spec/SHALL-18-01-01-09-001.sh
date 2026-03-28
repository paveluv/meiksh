# Test: SHALL-18-01-01-09-001
# Obligation: "When the current working directory is to be changed, unless
#   the utility or function description states otherwise, the operation shall
#   succeed unless a call to the chdir() function would fail when invoked
#   with the new working directory pathname as its argument."
# Verifies: cd to valid dir succeeds; cd to nonexistent dir fails.

d="$TMPDIR/shall_18_01_01_09_001_$$"
mkdir -p "$d/sub"

cd "$d/sub" || { printf '%s\n' "FAIL: cd to valid directory failed" >&2; exit 1; }

if [ "$(pwd)" != "$d/sub" ]; then
    printf '%s\n' "FAIL: pwd after cd does not match target" >&2
    exit 1
fi

if cd "$d/nonexistent" 2>/dev/null; then
    printf '%s\n' "FAIL: cd to nonexistent directory should have failed" >&2
    exit 1
fi

rm -rf "$d"
exit 0
