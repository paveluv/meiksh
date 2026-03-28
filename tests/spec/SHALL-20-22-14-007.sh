# SHALL-20-22-14-007
# "Otherwise, the following exit values shall be returned:: 126"
# command returns 126 when utility is found but cannot be invoked.

fail=0

# Create a file that exists but is not executable
noexec="$TMPDIR/not_executable_$$"
printf '#!/bin/sh\nexit 0\n' > "$noexec"
chmod -x "$noexec"

command "$noexec" 2>/dev/null
rc=$?
if [ "$rc" -ne 126 ]; then
  printf 'FAIL: non-executable file expected 126, got %d\n' "$rc" >&2
  fail=1
fi

rm -f "$noexec"

exit "$fail"
