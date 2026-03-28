# SHALL-20-132-10-003
# "where the three values shall be combinations of letters from the set
#  {r, w, x}; the presence of a letter shall indicate that the corresponding
#  bit is clear in the file mode creation mask."
# Verify symbolic output letters reflect cleared mask bits.

_old=$(umask)
umask 077
_out=$(umask -S)
# mask 077 blocks all group/other, so g= and o= should be empty
case "$_out" in
  u=rwx,g=,o=) ;;
  *) printf '%s\n' "FAIL: mask 077 expected 'u=rwx,g=,o=', got '$_out'" >&2
     umask "$_old"; exit 1 ;;
esac

umask 000
_out2=$(umask -S)
# mask 000 blocks nothing, so all rwx for all
case "$_out2" in
  u=rwx,g=rwx,o=rwx) ;;
  *) printf '%s\n' "FAIL: mask 000 expected 'u=rwx,g=rwx,o=rwx', got '$_out2'" >&2
     umask "$_old"; exit 1 ;;
esac

umask "$_old"
exit 0
