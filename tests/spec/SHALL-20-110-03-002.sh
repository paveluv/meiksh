# reviewed: GPT-5.4
# SHALL-20-110-03-002
# "Pathname expansion shall not fail due to the size of a file."
# Verifies: glob matching works regardless of file size.

SH="${MEIKSH:-${SHELL:-sh}}"
dir="$TMPDIR/glob_test_$$"
mkdir -p "$dir"

# Create a large file and a small file
dd if=/dev/zero of="$dir/bigfile.dat" bs=1024 count=1024 2>/dev/null
printf 'x' > "$dir/smallfile.dat"

out=$("$SH" -c "printf '%s\n' $dir/*.dat | wc -l")
rm -rf "$dir"

out=$(printf '%s\n' "$out" | tr -d ' ')
if [ "$out" -lt 2 ]; then
  printf '%s\n' "FAIL: glob matched $out files, expected 2" >&2; exit 1
fi

exit 0
