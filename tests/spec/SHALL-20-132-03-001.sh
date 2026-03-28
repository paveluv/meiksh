# SHALL-20-132-03-001
# "The umask utility shall set the file mode creation mask of the current shell
#  execution environment to the value specified by the mask operand."
# Verify umask sets the mask and it affects file creation.

_old=$(umask)
umask 077
_f="${TMPDIR}/test_umask_$$"
: > "$_f"
_perms=$(ls -l "$_f" | cut -c2-10)
rm -f "$_f"
umask "$_old"

case "$_perms" in
  rw-------) ;;
  *) printf '%s\n' "FAIL: expected rw-------, got $_perms" >&2; exit 1 ;;
esac

exit 0
