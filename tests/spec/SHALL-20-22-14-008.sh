# SHALL-20-22-14-008
# "The utility specified by command_name was found but could not be invoked."
# Same scenario as 14-007: found but not executable → 126.

fail=0

noexec="$TMPDIR/not_invokable_$$"
printf '#!/bin/sh\nexit 0\n' > "$noexec"
chmod -x "$noexec"

command "$noexec" 2>/dev/null
rc=$?
if [ "$rc" -ne 126 ]; then
  printf 'FAIL: found-but-not-invokable expected 126, got %d\n' "$rc" >&2
  fail=1
fi

rm -f "$noexec"

exit "$fail"
