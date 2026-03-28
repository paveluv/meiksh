# SHALL-18-01-01-04-007
# "If the file is a directory, it shall be an empty directory; otherwise,
#  the file shall have length zero."
# Verify newly created file has length zero.

tmpf="$TMPDIR/shall_18_04_007_$$"
rm -f "$tmpf"

"${SHELL}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi
if [ -s "$tmpf" ]; then
  printf '%s\n' "FAIL: new file should have zero length" >&2
  rm -f "$tmpf"
  exit 1
fi
rm -f "$tmpf"

exit 0
