# SHALL-20-44-04-012
# "The following options shall be supported:: Re-execute the command without
#  invoking an editor."
# Verify fc -s re-executes without opening an editor.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

outfile="$TMPDIR/fc_s_noeditor_$$"

# Set FCEDIT to something that would fail if invoked
${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  FCEDIT=/nonexistent_editor_$$
  export FCEDIT
  echo reexec_test_value
  fc -s echo > "'"$outfile"'"
  exit 0
'

result=$(cat "$outfile")
case "$result" in
  *reexec_test_value*) ;;
  *) echo "FAIL: fc -s did not re-execute: $result" >&2; rm -f "$HISTFILE" "$outfile"; exit 1 ;;
esac

rm -f "$HISTFILE" "$outfile"
exit 0
