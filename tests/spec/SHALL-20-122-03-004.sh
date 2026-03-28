# SHALL-20-122-03-004
# "The message shall include the following information: The System CPU time,
#  equivalent to the sum of the tms_stime and tms_cstime fields..."
# Verify time -p output includes system CPU time.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
_sys=$(printf '%s\n' "$_err" | grep '^sys ')
if [ -z "$_sys" ]; then
  printf '%s\n' "FAIL: no 'sys' line in time -p output" >&2
  exit 1
fi

exit 0
