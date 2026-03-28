# reviewed: GPT-5.4
# SHALL-20-132-10-004
# "If a mask operand is specified, there shall be no output written to standard
#  output."
# Verify umask produces no stdout when setting a mask.

_old=$(umask)
_out=$(umask 022)
_out_s=$(umask -S 022 2>/dev/null)
umask "$_old"

if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: umask 022 produced stdout: '$_out'" >&2
  exit 1
fi

if [ -n "$_out_s" ]; then
  printf '%s\n' "FAIL: umask -S 022 produced stdout: '$_out_s'" >&2
  exit 1
fi

exit 0
