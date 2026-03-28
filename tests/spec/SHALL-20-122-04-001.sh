# SHALL-20-122-04-001
# "The time utility shall conform to XBD 12.2 Utility Syntax Guidelines."
# Verify time accepts -- to end option processing.

_err=$("${SHELL:-sh}" -c 'time -p -- true' 2>&1 >/dev/null)
_rc=$?
if [ "$_rc" != "0" ]; then
  printf '%s\n' "FAIL: 'time -p -- true' exited $_rc" >&2
  exit 1
fi
case "$_err" in
  *real*) ;;
  *) printf '%s\n' "FAIL: no timing output from 'time -p -- true'" >&2; exit 1 ;;
esac

exit 0
