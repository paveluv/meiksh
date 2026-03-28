# SHALL-20-132-03-002
# "it shall not affect the file mode creation mask of the caller's environment."
# Verify umask in a subshell does not change the parent's mask.

_old=$(umask)
umask 022
(umask 077)
_cur=$(umask)
umask "$_old"

if [ "$_cur" != "0022" ] && [ "$_cur" != "022" ]; then
  printf '%s\n' "FAIL: subshell umask leaked, got $_cur" >&2
  exit 1
fi

exit 0
