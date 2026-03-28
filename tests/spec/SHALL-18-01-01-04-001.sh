# SHALL-18-01-01-04-001
# "If a file that does not exist is to be written, it shall be created as
#  described below, unless the utility description states otherwise."
# Verify output redirection creates a nonexistent file.

tmpf="$TMPDIR/shall_18_04_001_$$"
rm -f "$tmpf"

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "created" > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file was not created by redirection" >&2
  exit 1
fi
content=$(cat "$tmpf")
rm -f "$tmpf"
if [ "$content" != "created" ]; then
  printf '%s\n' "FAIL: file content wrong, got '$content'" >&2
  exit 1
fi

exit 0
