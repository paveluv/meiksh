# SHALL-20-64-03-004
# "The sig argument is the value specified by the -s option, -signal_number
#  option, or the -signal_name option, or by SIGTERM, if none of these
#  options is specified."
# Verify: default signal is SIGTERM; -s overrides it.

_tmp="$TMPDIR/kill_03_004.$$"

# Test 1: default signal is SIGTERM (process should die)
sh -c 'sleep 60' &
_pid=$!
sleep 1
kill "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: default kill did not terminate process (SIGTERM expected)" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 2: -s HUP sends SIGHUP
_got=""
trap '_got=HUP' HUP
kill -s HUP $$ 2>/dev/null
sleep 1
if [ "$_got" != "HUP" ]; then
  printf '%s\n' "FAIL: kill -s HUP did not deliver SIGHUP" >&2
  exit 1
fi

exit 0
