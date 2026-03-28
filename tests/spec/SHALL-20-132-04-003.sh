# SHALL-20-132-04-003
# "The following option shall be supported: -S"
# Verify umask -S is accepted and produces output.

_out=$(umask -S)
if [ -z "$_out" ]; then
  printf '%s\n' "FAIL: umask -S produced no output" >&2
  exit 1
fi

exit 0
