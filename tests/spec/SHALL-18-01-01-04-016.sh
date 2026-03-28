# SHALL-18-01-01-04-016
# "Unless otherwise specified, the file created shall be a regular file."
# (Duplicate of 04-009) Verify redirection creates regular file.

tmpf="$TMPDIR/shall_18_04_016_$$"
rm -f "$tmpf"

"${SHELL}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created or not regular" >&2
  exit 1
fi
rm -f "$tmpf"

exit 0
