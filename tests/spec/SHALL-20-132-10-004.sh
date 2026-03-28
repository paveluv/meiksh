# SHALL-20-132-10-004
# "If a mask operand is specified, there shall be no output written to standard
#  output."
# Verify umask produces no stdout when setting a mask.

_old=$(umask)
_out=$(umask 022)
umask "$_old"

if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: umask 022 produced stdout: '$_out'" >&2
  exit 1
fi

exit 0
