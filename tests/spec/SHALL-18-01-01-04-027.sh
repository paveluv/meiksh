# SHALL-18-01-01-04-027
# "Regular file. When attempting to create a regular file, and the existing
#  file is a regular file: The user ID, group ID, and permission bits of the
#  file shall not be changed. The file shall be truncated to zero length."
# (Duplicate of 04-022) Verify truncation preserves ownership/perms.

tmpf="$TMPDIR/shall_18_04_027_$$"
printf '%s\n' "original content here" > "$tmpf"
chmod 600 "$tmpf"

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "replaced" > "'"$tmpf"'"'

perms=$(ls -l "$tmpf" | cut -c2-10)
content=$(cat "$tmpf")
rm -f "$tmpf"

if [ "$content" != "replaced" ]; then
  printf '%s\n' "FAIL: expected 'replaced', got '$content'" >&2
  exit 1
fi
if [ "$perms" != "rw-------" ]; then
  printf '%s\n' "FAIL: perms should be rw-------, got '$perms'" >&2
  exit 1
fi

exit 0
