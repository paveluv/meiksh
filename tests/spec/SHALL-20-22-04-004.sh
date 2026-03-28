# SHALL-20-22-04-004
# "Perform the command search using a default value for PATH that is guaranteed
#  to find all of the standard utilities."

fail=0

# With an empty PATH, command -p should still find standard utilities
out=$(PATH= command -p cat /dev/null 2>&1)
rc=$?
if [ "$rc" -eq 127 ]; then
  printf 'FAIL: command -p cat not found with empty PATH\n' >&2
  fail=1
fi

# command -p should find printf
PATH= command -p printf '' 2>/dev/null
rc=$?
if [ "$rc" -eq 127 ]; then
  printf 'FAIL: command -p printf not found with empty PATH\n' >&2
  fail=1
fi

exit "$fail"
