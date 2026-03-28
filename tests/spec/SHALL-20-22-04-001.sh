# SHALL-20-22-04-001
# "The command utility shall conform to XBD 12.2 Utility Syntax Guidelines."

fail=0

# -- terminates option processing
out=$(command -v -- ls 2>/dev/null)
if [ -z "$out" ]; then
  printf 'FAIL: command -v -- ls produced no output\n' >&2
  fail=1
fi

# -v and -p can be combined
out=$(command -pv ls 2>/dev/null)
if [ -z "$out" ]; then
  printf 'FAIL: command -pv ls produced no output\n' >&2
  fail=1
fi

exit "$fail"
