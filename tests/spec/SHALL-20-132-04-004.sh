# SHALL-20-132-04-004
# "Produce symbolic output."
# Verify umask -S produces symbolic format u=...,g=...,o=...

_old=$(umask)
umask 022
_out=$(umask -S)
umask "$_old"

case "$_out" in
  u=rwx,g=rx,o=rx) ;;
  *) printf '%s\n' "FAIL: expected 'u=rwx,g=rx,o=rx', got '$_out'" >&2; exit 1 ;;
esac

exit 0
