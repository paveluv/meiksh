# SHALL-18-01-01-04-005
# "If the file is a regular file, the permission bits of the file shall be
#  set to: S_IROTH | S_IWOTH | S_IRGRP | S_IWGRP | S_IRUSR | S_IWUSR ...
#  except that the bits specified by the file mode creation mask of the
#  process shall be cleared."
# Verify file created by redirection has permissions = 0666 & ~umask.

tmpf="$TMPDIR/shall_18_04_005_$$"
rm -f "$tmpf"

"${MEIKSH:-meiksh}" -c 'umask 022; : > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi

perms=$(ls -l "$tmpf" | cut -c2-10)
rm -f "$tmpf"

if [ "$perms" != "rw-r--r--" ]; then
  printf '%s\n' "FAIL: expected rw-r--r--, got '$perms'" >&2
  exit 1
fi

exit 0
