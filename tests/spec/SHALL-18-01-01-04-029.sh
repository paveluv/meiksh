# SHALL-18-01-01-04-029
# "When a file is to be read or written, the file shall be opened with an
#  access mode corresponding to the operation to be performed. If file access
#  permissions deny access, the requested operation shall fail."
# Verify reading a non-readable file fails, writing a non-writable file fails.

tmpf="$TMPDIR/shall_18_04_029_$$"
printf '%s\n' "secret" > "$tmpf"
chmod 000 "$tmpf"

"${SHELL}" -c 'cat < "'"$tmpf"'"' >/dev/null 2>&1
rc_read=$?

chmod 444 "$tmpf"
"${SHELL}" -c 'printf "%s\n" "x" > "'"$tmpf"'"' 2>/dev/null
rc_write=$?

chmod 644 "$tmpf"
rm -f "$tmpf"

if [ "$rc_read" -eq 0 ]; then
  printf '%s\n' "FAIL: reading no-permission file should fail" >&2
  exit 1
fi
if [ "$rc_write" -eq 0 ]; then
  printf '%s\n' "FAIL: writing read-only file should fail" >&2
  exit 1
fi

exit 0
