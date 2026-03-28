# SHALL-18-01-01-04-013
# "The last data access, last data modification, and last file status change
#  timestamps of the file shall be updated as specified in XBD 4.12 File
#  Times Update."
# (Duplicate of 04-006) Verify timestamps are recent on new file.

tmpf="$TMPDIR/shall_18_04_013_$$"
rm -f "$tmpf"

before=$(date +%s)
"${MEIKSH:-meiksh}" -c ': > "'"$tmpf"'"'
after=$(date +%s)

if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi

mtime=$(stat -f %m "$tmpf" 2>/dev/null || stat -c %Y "$tmpf" 2>/dev/null)
rm -f "$tmpf"

if [ -z "$mtime" ]; then
  printf '%s\n' "FAIL: could not stat mtime" >&2
  exit 1
fi

before=$((before - 2))
after=$((after + 2))
if [ "$mtime" -lt "$before" ] || [ "$mtime" -gt "$after" ]; then
  printf '%s\n' "FAIL: mtime $mtime not in range [$before, $after]" >&2
  exit 1
fi

exit 0
