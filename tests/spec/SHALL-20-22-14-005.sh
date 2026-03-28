# SHALL-20-22-14-005
# "The command_name could not be found or an error occurred."
# Verify exit >0 and no stdout for -v when name is not found.

fail=0

out=$(command -v __totally_missing_cmd__ 2>/dev/null)
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: should exit >0 for missing command\n' >&2
  fail=1
fi
if [ -n "$out" ]; then
  printf 'FAIL: should produce no stdout for missing command\n' >&2
  fail=1
fi

exit "$fail"
