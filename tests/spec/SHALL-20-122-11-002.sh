# SHALL-20-122-11-002
# "If -p is specified, the following format shall be used for the timing
#  statistics in the POSIX locale:
#  \"real %f\\nuser %f\\nsys %f\\n\""
# Verify the POSIX -p format: three lines starting with real/user/sys followed
# by a floating-point number.

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)

_real=$(printf '%s\n' "$_err" | grep '^real [0-9]')
_user=$(printf '%s\n' "$_err" | grep '^user [0-9]')
_sys=$(printf '%s\n' "$_err" | grep '^sys [0-9]')

if [ -z "$_real" ]; then
  printf '%s\n' "FAIL: 'real' line missing or wrong format" >&2; exit 1
fi
if [ -z "$_user" ]; then
  printf '%s\n' "FAIL: 'user' line missing or wrong format" >&2; exit 1
fi
if [ -z "$_sys" ]; then
  printf '%s\n' "FAIL: 'sys' line missing or wrong format" >&2; exit 1
fi

exit 0
