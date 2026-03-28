# SHALL-18-01-01-04-018
# "Fail. The attempt to create the new file shall fail and the utility shall
#  either continue with its operation or exit immediately with an exit status
#  that indicates an error occurred"
# Verify redirection to a directory fails.

tmpd="$TMPDIR/shall_18_04_018_$$"
mkdir -p "$tmpd"

"${SHELL}" -c 'printf "%s\n" "data" > "'"$tmpd"'"' 2>/dev/null
rc=$?
rmdir "$tmpd" 2>/dev/null
rm -rf "$tmpd"

if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: redirecting to directory should fail" >&2
  exit 1
fi

exit 0
