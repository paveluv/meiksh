# SHALL-18-01-01-04-014
# "If the file is a directory, it shall be an empty directory; otherwise,
#  the file shall have length zero."
# (Duplicate of 04-007) Verify new file has zero length.

tmpf="$TMPDIR/shall_18_04_014_$$"
rm -f "$tmpf"

"${SHELL}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi
if [ -s "$tmpf" ]; then
  printf '%s\n' "FAIL: file should have zero length" >&2
  rm -f "$tmpf"
  exit 1
fi
rm -f "$tmpf"

exit 0
