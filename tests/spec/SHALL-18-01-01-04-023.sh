# SHALL-18-01-01-04-023
# "Fail. The attempt to create the new file shall fail and the utility shall
#  either continue with its operation or exit immediately with an exit status
#  that indicates an error occurred"
# (Duplicate of 04-018) Verify redirect to directory fails with error.

tmpd="$TMPDIR/shall_18_04_023_$$"
mkdir -p "$tmpd"

"${SHELL}" -c ': > "'"$tmpd"'"' 2>/dev/null
rc=$?
rmdir "$tmpd" 2>/dev/null
rm -rf "$tmpd"

if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: redirect to directory should fail" >&2
  exit 1
fi

exit 0
