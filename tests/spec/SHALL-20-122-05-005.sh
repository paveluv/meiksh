# SHALL-20-122-05-005
# "Any string to be supplied as an argument when invoking the utility named by
#  the utility operand."
# Verify time passes arguments through to the invoked utility.

_out=$("${SHELL:-sh}" -c 'time -p printf "%s" hello' 2>/dev/null)
if [ "$_out" != "hello" ]; then
  printf '%s\n' "FAIL: expected 'hello', got '$_out'" >&2
  exit 1
fi

exit 0
