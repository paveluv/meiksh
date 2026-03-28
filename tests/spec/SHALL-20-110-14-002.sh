# SHALL-20-110-14-002
# "The following exit values shall be returned:: 0"
# Verify sh exits 0 when given a script with only blank lines/comments.

tmpf="$TMPDIR/shall_20_110_14_002_$$"
cat > "$tmpf" <<'SCRIPT'

# just a comment

   
# another comment
SCRIPT

"${SHELL}" "$tmpf"
rc=$?
rm -f "$tmpf"

if [ "$rc" -ne 0 ]; then
  printf '%s\n' "FAIL: expected exit 0 for blank/comment script, got $rc" >&2
  exit 1
fi

exit 0
