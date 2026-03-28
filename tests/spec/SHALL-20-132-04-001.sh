# SHALL-20-132-04-001
# "The umask utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify umask accepts -- to end option processing.

_old=$(umask)
umask -- 077
_cur=$(umask)
umask "$_old"

case "$_cur" in
  *77*) ;;
  *) printf '%s\n' "FAIL: 'umask -- 077' did not work, got $_cur" >&2; exit 1 ;;
esac

exit 0
