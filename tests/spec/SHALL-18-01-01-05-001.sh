# SHALL-18-01-01-05-001
# "When a directory that is the root directory or current working directory
#  of any process is removed, the effect is implementation-defined. If file
#  access permissions deny access, the requested operation shall fail."
# Verify that removing a file with no write permission on parent dir fails.

tmpd="$TMPDIR/shall_18_05_001_$$"
mkdir -p "$tmpd"
printf '%s\n' "protected" > "$tmpd/file"
chmod 555 "$tmpd"

"${SHELL}" -c 'rm "'"$tmpd/file"'"' 2>/dev/null
rc=$?

chmod 755 "$tmpd"
rm -rf "$tmpd"

if [ "$rc" -eq 0 ]; then
  printf '%s\n' "FAIL: removing file in read-only dir should fail" >&2
  exit 1
fi

exit 0
