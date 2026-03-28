# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-012
# SHALL-20-64-04-006
# "Values of signal_name shall be recognized in a case-independent fashion,
#  without the SIG prefix. In addition, the symbolic name 0 shall be
#  recognized, representing the signal value zero."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# case-independent named signals without SIG prefix, plus symbolic name 0.

_got=""
trap '_got=hup' HUP
kill -s hup $$ 2>/dev/null
if [ "$_got" != "hup" ]; then
  printf '%s\n' "FAIL: kill -s hup was not recognized case-insensitively" >&2
  exit 1
fi

_got=""
trap '_got=hup-mixed' HUP
kill -s HuP $$ 2>/dev/null
if [ "$_got" != "hup-mixed" ]; then
  printf '%s\n' "FAIL: kill -s HuP was not recognized case-insensitively" >&2
  exit 1
fi

# Test 3: signal 0 tests process existence without killing
kill -s 0 $$ 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 returned $_rc for live shell" >&2
  exit 1
fi

trap - HUP
exit 0
