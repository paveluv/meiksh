# SHALL-20-122-11-001
# "If the utility utility is invoked, the standard error shall be used to write
#  the timing statistics and may be used to write a diagnostic message if the
#  utility terminates abnormally"
# Verify timing stats go to stderr, not stdout.

_out=$("${SHELL:-sh}" -c 'time -p true' 2>/dev/null)
if [ -n "$_out" ]; then
  printf '%s\n' "FAIL: time wrote to stdout: '$_out'" >&2
  exit 1
fi

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
case "$_err" in
  *real*) ;;
  *) printf '%s\n' "FAIL: no timing stats on stderr" >&2; exit 1 ;;
esac

exit 0
