# SHALL-20-64-04-006
# "Values of signal_name shall be recognized in a case-independent fashion,
#  without the SIG prefix. In addition, the symbolic name 0 shall be
#  recognized, representing the signal value zero."
# Verify: case-independent signal names, signal 0 for existence test.

# Test 1: lowercase signal name
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill -s term "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -s term (lowercase) did not work" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 2: mixed case
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill -s Term "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -s Term (mixed case) did not work" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 3: signal 0 tests process existence without killing
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill -s 0 "$_pid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -s 0 returned $_rc for live process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
if ! kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: signal 0 killed the process" >&2
  exit 1
fi
kill -9 "$_pid" 2>/dev/null
wait "$_pid" 2>/dev/null

exit 0
