# SHALL-20-22-14-011
# "Otherwise, the exit status of command shall be that of the simple command
#  specified by the arguments to command."

fail=0

# command passes through the exit status of the invoked utility
command sh -c 'exit 42'
rc=$?
if [ "$rc" -ne 42 ]; then
  printf 'FAIL: command sh -c exit42 expected 42, got %d\n' "$rc" >&2
  fail=1
fi

command true
rc=$?
if [ "$rc" -ne 0 ]; then
  printf 'FAIL: command true expected 0, got %d\n' "$rc" >&2
  fail=1
fi

command false
rc=$?
if [ "$rc" -eq 0 ]; then
  printf 'FAIL: command false should exit nonzero\n' >&2
  fail=1
fi

exit "$fail"
