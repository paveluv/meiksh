# SHALL-20-122-14-004
# "126 - The utility specified by utility was found but could not be invoked."
# Verify time exits 126 for a non-executable file.

_f="${TMPDIR}/test_time_noexec_$$"
printf '' > "$_f"
chmod -x "$_f"

"${SHELL:-sh}" -c "time -p '$_f'" 2>/dev/null
_rc=$?
rm -f "$_f"

if [ "$_rc" != "126" ]; then
  printf '%s\n' "FAIL: expected 126, got $_rc" >&2
  exit 1
fi

exit 0
