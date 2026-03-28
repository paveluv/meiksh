# SHALL-20-132-14-002
# "The following exit values shall be returned: 0 - The file mode creation mask
#  was successfully changed, or no mask operand was supplied."
# Verify umask exits 0 on successful set and query.

umask 022
_rc=$?
if [ "$_rc" != "0" ]; then
  printf '%s\n' "FAIL: umask set exit $_rc, expected 0" >&2; exit 1
fi

umask >/dev/null
_rc=$?
if [ "$_rc" != "0" ]; then
  printf '%s\n' "FAIL: umask query exit $_rc, expected 0" >&2; exit 1
fi

exit 0
