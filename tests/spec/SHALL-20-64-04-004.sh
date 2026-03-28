# SHALL-20-64-04-004
# "Write all values of signal_name supported by the implementation, if no
#  operand is given. If an exit_status operand is given and it is the
#  unsigned decimal integer value of a signal number, the signal_name ...
#  corresponding to that signal shall be written."
# Verify: kill -l lists signals; kill -l <number> maps to signal name.

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

# Test 3: kill -l with signal-exit-status (128+9=137) should output KILL
_name=$(kill -l 137 2>/dev/null)
case "$_name" in
  *KILL*) ;;
  *)
    printf '%s\n' "FAIL: kill -l 137 gave '$_name', expected KILL" >&2
    exit 1
    ;;
esac

exit 0
