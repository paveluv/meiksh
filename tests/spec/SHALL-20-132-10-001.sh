# reviewed: GPT-5.4
# Also covers: SHALL-20-132-04-005
# SHALL-20-132-10-001
# "When the mask operand is not specified, the umask utility shall write a
#  message to standard output that can later be used as a umask mask operand."
# Verify umask query mode writes to stdout and output is a valid mask operand.

_old=$(umask)
umask 027
_out=$(umask)
umask "$_out" 2>/dev/null
_rc=$?
umask "$_old"

if [ "$_rc" != "0" ]; then
  printf '%s\n' "FAIL: umask output '$_out' not accepted as operand (rc=$_rc)" >&2
  exit 1
fi

exit 0
