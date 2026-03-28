# SHALL-20-122-03-002
# "The message shall include the following information: The elapsed (real) time
#  between invocation of utility and its termination."
# Verify time -p output includes elapsed real time.

_err=$("${SHELL:-sh}" -c 'time -p sleep 0' 2>&1 >/dev/null)
_real=$(printf '%s\n' "$_err" | grep '^real ')
if [ -z "$_real" ]; then
  printf '%s\n' "FAIL: no 'real' line in time -p output" >&2
  exit 1
fi

exit 0
