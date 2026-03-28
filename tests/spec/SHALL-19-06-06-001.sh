# Test: SHALL-19-06-06-001
# Obligation: "After field splitting, if set -f is not in effect, each field in
#   the resulting command line shall be expanded using the algorithm described
#   in 2.14 Pattern Matching Notation."
# Verifies: pathname expansion occurs after field splitting; set -f disables it.

# Create test files in TMPDIR
dir="$TMPDIR/shall_19_06_06_001_$$"
mkdir -p "$dir"
touch "$dir/aaa" "$dir/bbb" "$dir/ccc"

# Pathname expansion should expand the glob
count_args() { printf '%s\n' "$#"; }
n=$(count_args "$dir"/*)
if [ "$n" -lt "3" ]; then
    printf '%s\n' "FAIL: glob '$dir/*' expanded to $n files, expected >=3" >&2
    rm -rf "$dir"
    exit 1
fi

# set -f disables pathname expansion
set -f
n2=$(count_args "$dir"/*)
set +f
if [ "$n2" != "1" ]; then
    printf '%s\n' "FAIL: set -f did not disable glob: got $n2 fields" >&2
    rm -rf "$dir"
    exit 1
fi

rm -rf "$dir"
exit 0
