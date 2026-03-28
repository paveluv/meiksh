# SHALL-20-22-04-007
# "The following options shall be supported:: -V"
# command -V writes a human-readable description of how the shell interprets
# a command name.

fail=0

out=$(command -V ls 2>/dev/null)
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -V ls exited %d\n' "$rc" >&2
  fail=1
fi
if [ -z "$out" ]; then
  printf 'FAIL: command -V ls produced no output\n' >&2
  fail=1
fi

# -V for nonexistent should fail
command -V __no_such_cmd_54321__ >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -V nonexistent should exit nonzero\n' >&2
  fail=1
fi

exit "$fail"
