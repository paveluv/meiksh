# SHALL-18-01-01-04-003
# "The user ID of the file shall be set to the effective user ID of the
#  calling process."
# Verify file created by redirection is owned by current user.

tmpf="$TMPDIR/shall_18_04_003_$$"
rm -f "$tmpf"

"${SHELL}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi

file_owner=$(ls -ln "$tmpf" | awk '{print $3}')
my_uid=$(id -u)
rm -f "$tmpf"

if [ "$file_owner" != "$my_uid" ]; then
  printf '%s\n' "FAIL: file uid=$file_owner, expected $my_uid" >&2
  exit 1
fi

exit 0
