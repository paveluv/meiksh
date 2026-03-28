# SHALL-20-122-03-005
# "The precision of the timing shall be no less than the granularity defined for
#  the size of the clock tick unit on the system, but the results shall be
#  reported in terms of standard time units (for example, 0.02 seconds,
#  00:00:00.02, 1m33.75s, 365.21 seconds), not numbers of clock ticks."
# Verify time -p reports values in seconds with a decimal point, not raw ticks.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
_real=$(printf '%s\n' "$_err" | grep '^real ' | sed 's/^real  *//')
case "$_real" in
  *[.]*) ;;  # contains a decimal point - standard time units
  *) printf '%s\n' "FAIL: real time '$_real' not in standard time units" >&2; exit 1 ;;
esac

exit 0
