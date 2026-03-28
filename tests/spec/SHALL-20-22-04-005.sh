# SHALL-20-22-04-005
# "The following options shall be supported:: -v"
# command -v writes how the shell would resolve a command name.

fail=0

# -v for an external utility should produce a pathname
out=$(command -v ls)
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command -v ls exited %d\n' "$rc" >&2
  fail=1
fi
if [ -z "$out" ]; then
  printf 'FAIL: command -v ls produced no output\n' >&2
  fail=1
fi

# -v for a nonexistent command should fail silently
out=$(command -v __nonexistent_cmd_12345__ 2>/dev/null)
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command -v for nonexistent cmd should fail\n' >&2
  fail=1
fi

exit "$fail"
