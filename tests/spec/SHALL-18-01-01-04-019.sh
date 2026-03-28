# SHALL-18-01-01-04-019
# "Follow link. Unless otherwise specified, the symbolic link shall be
#  followed as specified for pathname resolution, and the operation performed
#  shall be as if the target of the symbolic link (after all resolution) had
#  been named."
# Verify redirection through a symlink writes to the target.

tmpf="$TMPDIR/shall_18_04_019_target_$$"
tmpl="$TMPDIR/shall_18_04_019_link_$$"
rm -f "$tmpf" "$tmpl"
ln -s "$tmpf" "$tmpl"

"${SHELL}" -c 'printf "%s\n" "through_link" > "'"$tmpl"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: target file not created via symlink" >&2
  rm -f "$tmpl"
  exit 1
fi
content=$(cat "$tmpf")
rm -f "$tmpf" "$tmpl"

if [ "$content" != "through_link" ]; then
  printf '%s\n' "FAIL: expected 'through_link', got '$content'" >&2
  exit 1
fi

exit 0
