# SHALL-20-132-03-003
# "If the mask operand is not specified, the umask utility shall write to
#  standard output the value of the file mode creation mask of the invoking
#  process."
# Verify umask with no operand prints the current mask.

_old=$(umask)
umask 037
_out=$(umask)
umask "$_old"

case "$_out" in
  *37*) ;;
  *) printf '%s\n' "FAIL: expected *37* in output, got '$_out'" >&2; exit 1 ;;
esac

exit 0
