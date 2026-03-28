# SHALL-20-44-05-006
# "Select the commands to list or edit. [...]  If first is omitted [with -s],
#  the previous command shall be used."
# Verify fc -s with no first operand re-executes the previous command.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

outfile="$TMPDIR/fc_default_$$"

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo default_reexec_test
  fc -s > "'"$outfile"'"
  exit 0
'

result=$(cat "$outfile")
case "$result" in
  *default_reexec_test*) ;;
  *) echo "FAIL: fc -s without first did not re-execute previous: $result" >&2
     rm -f "$HISTFILE" "$outfile"; exit 1 ;;
esac

rm -f "$HISTFILE" "$outfile"
exit 0
