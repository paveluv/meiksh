# SHALL-20-122-04-004
# "Write the timing output to standard error in the format shown in the STDERR
#  section."
# Verify -p produces the POSIX portable format: "real %f\nuser %f\nsys %f\n"

_err=$("${SHELL:-sh}" -c 'time -p true' 2>&1 >/dev/null)
_lines=$(printf '%s\n' "$_err" | grep -c '^[a-z]')
_real=$(printf '%s\n' "$_err" | grep '^real ')
_user=$(printf '%s\n' "$_err" | grep '^user ')
_sys=$(printf '%s\n' "$_err" | grep '^sys ')

if [ -z "$_real" ]; then
  printf '%s\n' "FAIL: missing 'real' line" >&2; exit 1
fi
if [ -z "$_user" ]; then
  printf '%s\n' "FAIL: missing 'user' line" >&2; exit 1
fi
if [ -z "$_sys" ]; then
  printf '%s\n' "FAIL: missing 'sys' line" >&2; exit 1
fi

exit 0
