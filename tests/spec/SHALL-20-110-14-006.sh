# reviewed: GPT-5.4
# SHALL-20-110-14-006
# "The following exit values shall be returned:: 126"
# Verify sh exits 126 when command_file is rejected as not being a script.

tmpf="$TMPDIR/shall_20_110_14_006_$$"
printf '\000bad\n' > "$tmpf"

SH="${MEIKSH:-${SHELL:-sh}}"

"$SH" "$tmpf" 2>/dev/null
rc=$?
rm -f "$tmpf"

# 126 = found but could not be invoked
if [ "$rc" -ne 126 ]; then
  printf '%s\n' "FAIL: expected exit 126, got $rc" >&2
  exit 1
fi

exit 0
