# SHALL-18-01-01-04-017
# "When an attempt is made to create a file that already exists, the utility
#  shall take the action indicated in Actions when Creating a File that
#  Already Exists corresponding to the type of the file"
# Verify > on existing regular file truncates it (RF action).

tmpf="$TMPDIR/shall_18_04_017_$$"
printf '%s\n' "old content" > "$tmpf"

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "new" > "'"$tmpf"'"'
content=$(cat "$tmpf")
rm -f "$tmpf"

if [ "$content" != "new" ]; then
  printf '%s\n' "FAIL: expected 'new', got '$content'" >&2
  exit 1
fi

exit 0
