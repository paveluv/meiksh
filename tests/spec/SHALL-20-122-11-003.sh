# SHALL-20-122-11-003
# "where each floating-point number shall be expressed in seconds. The precision
#  used may be less than the default six digits of %f, but shall be sufficiently
#  precise to accommodate the size of the clock tick on the system... The number
#  of digits following the radix character shall be no less than one"
# Verify at least one digit after the decimal point in time -p output.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
_real=$(printf '%s\n' "$_err" | grep '^real ' | sed 's/^real  *//')

case "$_real" in
  *.*[0-9]*) ;;  # has at least one digit after decimal
  *) printf '%s\n' "FAIL: real '$_real' lacks digit after radix" >&2; exit 1 ;;
esac

exit 0
