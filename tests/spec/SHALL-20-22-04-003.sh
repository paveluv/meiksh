# SHALL-20-22-04-003
# "The following options shall be supported:: -p"
# command -p uses a default PATH to find standard utilities.

fail=0

# Even with PATH empty, command -p should find standard utilities
out=$(PATH= command -p ls / 2>/dev/null)
rc=$?
if [ "$rc" -eq 127 ]; then
  printf 'FAIL: command -p ls not found with empty PATH\n' >&2
  fail=1
fi

exit "$fail"
