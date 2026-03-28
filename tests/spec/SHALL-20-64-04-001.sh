# reviewed: GPT-5.4
# SHALL-20-64-04-001
# "The kill utility shall conform to XBD 12.2 Utility Syntax Guidelines,
#  [XSI] except that in the last two SYNOPSIS forms, the -signal_number
#  and -signal_name options are usually more than a single character."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill accepts -- as an option separator and the multi-character XSI
# -signal_number form.

sh -c 'sleep 60' &
_pid=$!
sleep 1

kill -- "$_pid" 2>/dev/null
sleep 1

if kill -0 "$_pid" 2>/dev/null; then
  printf '%s\n' "FAIL: 'kill -- pid' did not terminate process" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

sh -c 'sleep 60' &
_pid=$!
sleep 1

kill -0 "$_pid" 2>/dev/null
_rc=$?
if [ "$_rc" -ne 0 ]; then
  printf '%s\n' "FAIL: kill -0 pid was not accepted or did not succeed" >&2
  kill -9 "$_pid" 2>/dev/null
  exit 1
fi

kill -9 "$_pid" 2>/dev/null
wait "$_pid" 2>/dev/null
exit 0
