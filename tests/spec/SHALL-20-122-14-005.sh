# SHALL-20-122-14-005
# "If the utility utility is invoked, the exit status of time shall be the
#  exit status of utility; otherwise, the time utility shall exit with one of
#  the following values:: The utility specified by utility was found but could
#  not be invoked."
# Verify time exits 126 when utility is found but not invocable.

tmpf="$TMPDIR/shall_20_122_14_005_$$"
printf '%s\n' "not a valid script or binary" > "$tmpf"
chmod +x "$tmpf"

"${MEIKSH:-meiksh}" -c "time '$tmpf'" 2>/dev/null
rc=$?
rm -f "$tmpf"

if [ "$rc" -ne 126 ]; then
  printf '%s\n' "FAIL: expected exit 126, got $rc" >&2
  exit 1
fi

exit 0
