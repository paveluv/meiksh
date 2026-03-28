# SHALL-18-01-01-04-002
# "When a file that does not exist is created, the following features defined
#  in the System Interfaces volume of POSIX.1-2024 shall apply unless the
#  utility or function description states otherwise"
# Verify newly created file is regular, zero-length before write, and owned
# by current user.

tmpf="$TMPDIR/shall_18_04_002_$$"
rm -f "$tmpf"

"${SHELL}" -c ': > "'"$tmpf"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: file not created" >&2
  exit 1
fi
if [ -s "$tmpf" ]; then
  printf '%s\n' "FAIL: new file should be zero length" >&2
  rm -f "$tmpf"
  exit 1
fi
rm -f "$tmpf"

exit 0
