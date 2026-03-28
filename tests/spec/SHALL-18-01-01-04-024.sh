# SHALL-18-01-01-04-024
# "Follow link. Unless otherwise specified, the symbolic link shall be
#  followed as specified for pathname resolution"
# (Duplicate of 04-019) Verify symlink is followed for redirection.

tmpf="$TMPDIR/shall_18_04_024_target_$$"
tmpl="$TMPDIR/shall_18_04_024_link_$$"
rm -f "$tmpf" "$tmpl"
ln -s "$tmpf" "$tmpl"

"${MEIKSH:-meiksh}" -c 'printf "%s\n" "linked" > "'"$tmpl"'"'
if [ ! -f "$tmpf" ]; then
  printf '%s\n' "FAIL: target not created via symlink" >&2
  rm -f "$tmpl"
  exit 1
fi
content=$(cat "$tmpf")
rm -f "$tmpf" "$tmpl"

if [ "$content" != "linked" ]; then
  printf '%s\n' "FAIL: expected 'linked', got '$content'" >&2
  exit 1
fi

exit 0
