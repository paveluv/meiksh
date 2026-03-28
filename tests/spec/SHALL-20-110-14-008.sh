# SHALL-20-110-14-008
# "The following exit values shall be returned:: 127"
# Verify sh exits 127 when command_file is not found.

"${SHELL}" "$TMPDIR/shall_20_110_14_008_nonexistent_$$" 2>/dev/null
rc=$?

if [ "$rc" -ne 127 ]; then
  printf '%s\n' "FAIL: expected exit 127 for missing file, got $rc" >&2
  exit 1
fi

exit 0
