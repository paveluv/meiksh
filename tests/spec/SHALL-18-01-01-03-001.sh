# SHALL-18-01-01-03-001
# "The file access control mechanism described by XBD 4.7 File Access
#  Permissions shall apply to all files on an implementation conforming to
#  this volume of POSIX.1-2024."
# Verify the shell fails when file access permissions deny access.

tmpf="$TMPDIR/shall_18_03_001_$$"
printf '%s\n' "readonly" > "$tmpf"
chmod 000 "$tmpf"

"${SHELL}" -c 'cat "'"$tmpf"'"' >/dev/null 2>&1
rc=$?
chmod 644 "$tmpf"
rm -f "$tmpf"

if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: reading permission-denied file should fail" >&2
  exit 1
fi

exit 0
