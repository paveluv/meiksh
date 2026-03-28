# SHALL-20-64-03-001
# "The kill utility shall send a signal to the process or processes specified
#  by each pid operand."
# Verify: kill sends a signal to a target process.

_tmp="$TMPDIR/kill_03_001.$$"

sh -c 'echo $$ > '"$_tmp"'.pid; exec sleep 60' &
_bgpid=$!
sleep 1

kill "$_bgpid" 2>/dev/null
sleep 1

if kill -0 "$_bgpid" 2>/dev/null; then
  printf '%s\n' "FAIL: process $_bgpid still alive after kill" >&2
  kill -9 "$_bgpid" 2>/dev/null
  rm -f "$_tmp.pid"
  exit 1
fi

wait "$_bgpid" 2>/dev/null
rm -f "$_tmp.pid"
exit 0
