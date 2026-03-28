# SHALL-18-01-01-04-022
# "Regular file. When attempting to create a regular file, and the existing
#  file is a regular file: The user ID, group ID, and permission bits of the
#  file shall not be changed. The file shall be truncated to zero length."
# Verify > on existing regular file: preserves perms, truncates content.

tmpf="$TMPDIR/shall_18_04_022_$$"
printf '%s\n' "old data that is long" > "$tmpf"
chmod 640 "$tmpf"

"${SHELL}" -c 'printf "%s\n" "new" > "'"$tmpf"'"'

perms=$(ls -l "$tmpf" | cut -c2-10)
content=$(cat "$tmpf")
rm -f "$tmpf"

if [ "$content" != "new" ]; then
  printf '%s\n' "FAIL: content should be 'new', got '$content'" >&2
  exit 1
fi
if [ "$perms" != "rw-r-----" ]; then
  printf '%s\n' "FAIL: perms should be rw-r-----, got '$perms'" >&2
  exit 1
fi

exit 0
