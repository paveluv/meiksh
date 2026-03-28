# SHALL-19-06-004
# "Pathname expansion shall be performed, unless set -f is in effect."
# Verify pathname expansion and that set -f disables it.

fail=0

# Create temp files for globbing
dir="$TMPDIR/glob_test_$$"
mkdir -p "$dir"
: > "$dir/file_a"
: > "$dir/file_b"

# Pathname expansion should match
count=0
for f in "$dir"/file_*; do count=$((count+1)); done
[ "$count" = "2" ] || { printf '%s\n' "FAIL: glob matched $count, expected 2" >&2; fail=1; }

# set -f disables pathname expansion
set -f
result=
for f in "$dir"/file_*; do result="$f"; done
[ "$result" = "$dir/file_*" ] || { printf '%s\n' "FAIL: set -f did not disable glob" >&2; fail=1; }
set +f

rm -rf "$dir"

exit "$fail"
