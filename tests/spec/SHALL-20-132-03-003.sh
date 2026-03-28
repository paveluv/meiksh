# reviewed: GPT-5.4
# SHALL-20-132-03-003
# "If the mask operand is not specified, the umask utility shall write to
#  standard output the value of the file mode creation mask of the invoking
#  process."
# Verify umask with no operand prints the current mask.

_old=$(umask)
umask 037
_out=$(umask)
_cur=$(umask)
umask "$_old"

case "$_out" in
  *37*) ;;
  *) printf '%s\n' "FAIL: expected *37* in output, got '$_out'" >&2; exit 1 ;;
esac

case "$_cur" in
  *37*) ;;
  *) printf '%s\n' "FAIL: umask query changed the current mask, got '$_cur'" >&2; exit 1 ;;
esac

exit 0
