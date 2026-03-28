# SHALL-18-01-01-04-012
# "If the file is a regular file, the permission bits of the file shall be
#  set to: S_IROTH | S_IWOTH | S_IRGRP | S_IWGRP | S_IRUSR | S_IWUSR ...
#  except that the bits specified by the file mode creation mask of the
#  process shall be cleared."
# (Duplicate of 04-005) Verify permissions with a different umask.

tmpf="$TMPDIR/shall_18_04_012_$$"
rm -f "$tmpf"

"${MEIKSH:-meiksh}" -c 'umask 077; : > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi

perms=$(ls -l "$tmpf" | cut -c2-10)
rm -f "$tmpf"

if [ "$perms" != "rw-------" ]; then
  printf '%s\n' "FAIL: expected rw-------, got '$perms'" >&2
  exit 1
fi

exit 0
