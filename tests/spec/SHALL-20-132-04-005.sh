# SHALL-20-132-04-005
# "The default output style is unspecified, but shall be recognized on a
#  subsequent invocation of umask on the same system as a mask operand to
#  restore the previous file mode creation mask."
# Verify round-trip: umask output can restore the mask.

_old=$(umask)
umask 037
_saved=$(umask)
umask 022
umask "$_saved"
_restored=$(umask)
umask "$_old"

if [ "$_saved" != "$_restored" ]; then
  printf '%s\n' "FAIL: round-trip failed, saved=$_saved restored=$_restored" >&2
  exit 1
fi

exit 0
