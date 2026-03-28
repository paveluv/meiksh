# SHALL-20-122-03-003
# "The message shall include the following information: The User CPU time,
#  equivalent to the sum of the tms_utime and tms_cutime fields..."
# Verify time -p output includes user CPU time.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
_user=$(printf '%s\n' "$_err" | grep '^user ')
if [ -z "$_user" ]; then
  printf '%s\n' "FAIL: no 'user' line in time -p output" >&2
  exit 1
fi

exit 0
