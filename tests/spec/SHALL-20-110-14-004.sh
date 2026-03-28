# SHALL-20-110-14-004
# "The following exit values shall be returned:: 1-125"
# Verify sh exits in range 1-125 for a non-interactive syntax error.

tmpf="$TMPDIR/shall_20_110_14_004_$$"
printf '%s\n' 'if then fi fi fi' > "$tmpf"

"${SHELL}" "$tmpf" 2>/dev/null
rc=$?
rm -f "$tmpf"

if [ "$rc" -lt 1 ] || [ "$rc" -gt 125 ]; then
  printf '%s\n' "FAIL: expected exit 1-125 for syntax error, got $rc" >&2
  exit 1
fi

exit 0
