# SHALL-18-01-01-04-011
# "The group ID of the file shall be set to the effective group ID of the
#  calling process or the group ID of the directory in which the file is
#  being created."
# (Duplicate of 04-004) Verify group ownership.

tmpf="$TMPDIR/shall_18_04_011_$$"
rm -f "$tmpf"

"${MEIKSH:-meiksh}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi

file_gid=$(ls -ln "$tmpf" | awk '{print $4}')
my_gid=$(id -g)
dir_gid=$(ls -ldn "$TMPDIR" | awk '{print $4}')
rm -f "$tmpf"

if [ "$file_gid" != "$my_gid" ] && [ "$file_gid" != "$dir_gid" ]; then
  printf '%s\n' "FAIL: file gid=$file_gid, expected $my_gid or $dir_gid" >&2
  exit 1
fi

exit 0
