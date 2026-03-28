# reviewed: GPT-5.4
# Also covers: SHALL-20-64-04-011
# SHALL-20-64-04-004
# "Write all values of signal_name supported by the implementation, if no
#  operand is given. If an exit_status operand is given and it is the
#  unsigned decimal integer value of a signal number, the signal_name ...
#  corresponding to that signal shall be written."
# Verifies docs/posix/utilities/kill.html#tag_20_64_04:
# kill -l with no operand, with a signal number, and with a real $? value
# from a signal-terminated process.

# Test 1: kill -l with no operand lists signals
_out=$(kill -l 2>/dev/null)
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: kill -l produced no output" >&2
  exit 1
fi

# Test 2: kill -l 9 should output KILL
_name=$(kill -l 9 2>/dev/null)
case "$_name" in
  *KILL*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 9 gave '$_name', expected KILL" >&2
    exit 1
    ;;
esac

# Test 3: kill -l with a real signal-based $? value should output KILL
sh -c 'kill -9 $$' >/dev/null 2>&1
_status=$?
_name=$(kill -l "$_status" 2>/dev/null)
case "$_name" in
  *KILL*) ;;
  *)
    printf '%s\n' "FAIL: kill -l \$_status=$_status gave '$_name', expected KILL" >&2
    exit 1
    ;;
esac

exit 0
