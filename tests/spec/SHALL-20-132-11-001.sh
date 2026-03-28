# SHALL-20-132-11-001
# "The standard error shall be used only for diagnostic messages."
# Verify umask does not write to stderr on success.

_old=$(umask)
_err=$(umask 022 2>&1 >/dev/null)
umask "$_old"

if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: umask wrote to stderr on success: '$_err'" >&2
  exit 1
fi

exit 0
