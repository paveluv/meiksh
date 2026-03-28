# SHALL-20-44-05-004
# "The following operands shall be supported:: old=new"
# Verify fc -s supports the old=new substitution operand.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

outfile="$TMPDIR/fc_oldnew_$$"

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo original_word
  fc -s original_word=replaced_word echo > "'"$outfile"'"
  exit 0
'

result=$(cat "$outfile")
case "$result" in
  *replaced_word*) ;;
  *) echo "FAIL: fc -s old=new did not substitute: $result" >&2; rm -f "$HISTFILE" "$outfile"; exit 1 ;;
esac

rm -f "$HISTFILE" "$outfile"
exit 0
