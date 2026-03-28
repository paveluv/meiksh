# SHALL-20-132-05-003
# "A string specifying the new file mode creation mask... In a symbolic_mode
#  value, the permissions op characters '+' and '-' shall be interpreted
#  relative to the current file mode creation mask; '+' shall cause the bits
#  for the indicated permissions to be cleared in the mask; '-' shall cause the
#  bits for the indicated permissions to be set in the mask."
# Verify symbolic mode with + clears mask bits and - sets mask bits.

_old=$(umask)

# Start with 077, then '+' group-read should clear the group-read bit in mask
umask 077
umask g+r
_out=$(umask -S)
case "$_out" in
  *g=r*|*g=*r*) ;;
  *) printf '%s\n' "FAIL: g+r did not clear group-read bit, got '$_out'" >&2
     umask "$_old"; exit 1 ;;
esac

# '-' should set mask bits (revoke permission)
umask 022
umask o-r
_out2=$(umask -S)
case "$_out2" in
  *o=x*|*o=x) ;;  # other has only x, r was removed
  *o=) ;;          # or no perms at all if w was already masked
  *) printf '%s\n' "FAIL: o-r did not set other-read bit in mask, got '$_out2'" >&2
     umask "$_old"; exit 1 ;;
esac

umask "$_old"
exit 0
