# SHALL-20-44-04-011
# "The following options shall be supported:: -s"
# Verify fc supports the -s option (re-execute without editor).

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

outfile="$TMPDIR/fc_s_out_$$"

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo hello_reexec
  fc -s echo > "'"$outfile"'"
  exit 0
'

result=$(cat "$outfile")
case "$result" in
  *hello_reexec*) ;;
  *) echo "FAIL: fc -s did not re-execute the echo command: $result" >&2; rm -f "$HISTFILE" "$outfile"; exit 1 ;;
esac

rm -f "$HISTFILE" "$outfile"
exit 0
