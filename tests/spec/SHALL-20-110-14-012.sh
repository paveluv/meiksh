# reviewed: GPT-5.4
# SHALL-20-110-14-012
# "Otherwise, the shell shall terminate in the same manner as for an exit
#  command with no operands, unless the last command the shell invoked was
#  executed without forking, in which case the wait status seen by the parent
#  process of the shell shall be the wait status of the last command the shell
#  invoked."
# Verify shell exit status equals $? of last command when reaching EOF.

# A script whose last command exits 42 should itself exit 42
SH="${MEIKSH:-${SHELL:-sh}}"
tmpf="$TMPDIR/shall_20_110_14_012_$$.sh"
printf '%s\n' 'exit 42' > "$tmpf"
"$SH" "$tmpf" >/dev/null 2>&1
_rc=$?
rm -f "$tmpf"
if [ "$_rc" != "42" ]; then
  printf '%s\n' "FAIL: expected 42, got $_rc" >&2
  exit 1
fi

# A script whose last command succeeds should itself exit 0
tmpf="$TMPDIR/shall_20_110_14_012_ok_$$.sh"
printf '%s\n' 'true' > "$tmpf"
"$SH" "$tmpf" >/dev/null 2>&1
_rc=$?
rm -f "$tmpf"
if [ "$_rc" != "0" ]; then
  printf '%s\n' "FAIL: expected 0 from successful last command, got $_rc" >&2
  exit 1
fi

exit 0
