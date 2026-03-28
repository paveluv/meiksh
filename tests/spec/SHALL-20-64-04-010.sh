# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-013
# SHALL-20-64-04-010
# "[XSI] Specify a non-negative decimal integer, signal_number, representing
#  the signal to be used instead of SIGTERM ... 0 0, 1 SIGHUP, 2 SIGINT,
#  3 SIGQUIT, 6 SIGABRT, 9 SIGKILL, 14 SIGALRM, 15 SIGTERM.
#  If the first argument is a negative integer, it shall be interpreted
#  as a -signal_number option, not as a negative pid operand."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# selected mandatory signal-number mappings and first-argument disambiguation.

TMP_BASE=${TMPDIR:-/tmp}
_tmp="$TMP_BASE/kill_04_010.$$"
rm -rf "$_tmp"
mkdir -p "$_tmp" || exit 1

expect_trapped_signal() {
  _num=$1
  _sig=$2
  _marker="$_tmp/${_sig}"
  rm -f "$_marker"
  trap "printf '%s' $_sig >\"$_marker\"" "$_sig"
  kill -$_num $$ 2>/dev/null
  if [ ! -f "$_marker" ]; then
    printf '%s\n' "FAIL: kill -$_num did not deliver $_sig" >&2
    rm -rf "$_tmp"
    exit 1
  fi
  _got=$(cat "$_marker")
  if [ "$_got" != "$_sig" ]; then
    printf '%s\n' "FAIL: kill -$_num wrote '$_got', expected '$_sig'" >&2
    rm -rf "$_tmp"
    exit 1
  fi
  trap - "$_sig"
}

# Test 1: kill -0 tests process existence (signal 0)
sh -c 'trap "exit 0" TERM; while :; do sleep 60; done' &
_pid=$!
sleep 1
kill -0 "$_pid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -0 returned $_rc for live process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

# Test 2: trap-able mandatory mappings
expect_trapped_signal 1 HUP
expect_trapped_signal 2 INT
expect_trapped_signal 3 QUIT
expect_trapped_signal 6 ABRT
expect_trapped_signal 14 ALRM
expect_trapped_signal 15 TERM

# Test 3: kill -9 sends SIGKILL
kill -9 "$_pid" 2>/dev/null
sleep 1
if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: kill -9 did not terminate process (SIGKILL)" >&2
  exit 1
fi
wait "$_pid" 2>/dev/null

# Test 4: first negative integer is parsed as -signal_number, not negative pid
_got=""
trap '_got=HUP-FIRST' HUP
kill -1 $$ 2>/dev/null
if [ "$_got" != "HUP-FIRST" ]; then
  printf '%s\n' "FAIL: kill -1 was not interpreted as signal number 1 (HUP)" >&2
  rm -rf "$_tmp"
  exit 1
fi

trap - HUP INT QUIT ABRT ALRM TERM
rm -rf "$_tmp"
exit 0
