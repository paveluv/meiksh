# SHALL-18-01-01-04-009
# "Unless otherwise specified, the file created shall be a regular file."
# Verify redirection creates a regular file.

tmpf="$TMPDIR/shall_18_04_009_$$"
rm -f "$tmpf"

"${MEIKSH:-meiksh}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created or not regular" >&2
  exit 1
fi
rm -f "$tmpf"

exit 0
