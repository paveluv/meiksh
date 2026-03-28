# SHALL-20-64-04-010
# "[XSI] Specify a non-negative decimal integer, signal_number, representing
#  the signal to be used instead of SIGTERM ... 0 0, 1 SIGHUP, 2 SIGINT,
#  3 SIGQUIT, 6 SIGABRT, 9 SIGKILL, 14 SIGALRM, 15 SIGTERM.
#  If the first argument is a negative integer, it shall be interpreted
#  as a -signal_number option, not as a negative pid operand."
# Verify: well-known signal number mappings and disambiguation.

# Test 1: kill -0 tests process existence (signal 0)
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill -0 "$_pid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -0 returned $_rc for live process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

# Test 2: kill -9 sends SIGKILL
kill -9 "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -9 did not terminate process (SIGKILL)" >&2
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 3: kill -1 sends SIGHUP
_got=""
trap '_got=HUP' HUP
kill -1 $$ 2>/dev/null
sleep 1
if [ "$_got" != "HUP" ]; then
  printf '%s\n' "FAIL: kill -1 did not deliver SIGHUP" >&2
  exit 1
fi

exit 0
