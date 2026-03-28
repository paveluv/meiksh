# SHALL-20-22-14-010
# "An error occurred in the command utility or the utility specified by
#  command_name could not be found."
# 127 for not-found command_name.

fail=0

command __missing_utility_98765__ 2>/dev/null
rc=$?
if [ "$rc" -ne 127 ]; then
  printf 'FAIL: missing utility expected 127, got %d\n' "$rc" >&2
  fail=1
fi

exit "$fail"
