# reviewed: GPT-5.4
# SHALL-20-132-11-001
# "The standard error shall be used only for diagnostic messages."
# Verifies docs/posix/utilities/umask.html#tag_20_132_11:
# successful umask queries and successful umask updates do not write stderr.

_old=$(umask)
_err=$(umask 2>&1 >/dev/null)
if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: umask wrote to stderr when querying mask: '$_err'" >&2
  exit 1
fi

_err=$(umask 022 2>&1 >/dev/null)
umask "$_old"

if [ -n "$_err" ]; then
  printf '%s\n' "FAIL: umask wrote to stderr on success: '$_err'" >&2
  exit 1
fi

exit 0
