# SHALL-20-44-05-005
# "The following operands shall be supported:: Replace the first occurrence of
#  string old in the commands to be re-executed by the string new."
# Verify fc -s old=new replaces only the FIRST occurrence.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

outfile="$TMPDIR/fc_firstonly_$$"

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo foo foo foo
  fc -s foo=bar echo > "'"$outfile"'"
  exit 0
'

result=$(cat "$outfile")
# Should be "bar foo foo" — only first occurrence replaced
case "$result" in
  "bar foo foo") ;;
  *) echo "FAIL: fc -s replaced more than first occurrence or wrong result: $result" >&2
     rm -f "$HISTFILE" "$outfile"; exit 1 ;;
esac

rm -f "$HISTFILE" "$outfile"
exit 0
