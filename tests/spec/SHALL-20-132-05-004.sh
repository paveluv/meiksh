# reviewed: GPT-5.4
# Also covers: SHALL-20-132-05-001, SHALL-20-132-05-002
# SHALL-20-132-05-004
# "In the octal integer form of mode, the specified bits are set in the file
#  mode creation mask. The file mode creation mask shall be set to the resulting
#  numeric value."
# Verify octal mask is set directly.

_old=$(umask)
umask 0123
_out=$(umask)
umask "$_old"

case "$_out" in
  *123*) ;;
  *) printf '%s\n' "FAIL: expected *123*, got '$_out'" >&2; exit 1 ;;
esac

exit 0
