# SHALL-20-44-04-010
# "The following options shall be supported:: Reverse the order of the commands
#  listed (with -l) or edited (with neither -l nor -s)."
# Verify fc -r reverses listing order.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${MEIKSH:-meiksh} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo AAA
  echo BBB
  echo CCC
  fwd=$(fc -l -n -3 -1)
  rev=$(fc -l -n -r -3 -1)
  last_fwd=$(printf "%s\n" "$fwd" | tail -1 | sed "s/^[[:space:]]*//")
  first_rev=$(printf "%s\n" "$rev" | head -1 | sed "s/^[[:space:]]*//")
  # The last line of forward should be the first line of reversed
  if [ "$last_fwd" != "$first_rev" ]; then
    echo "FAIL: reverse order mismatch: last_fwd=\"$last_fwd\" first_rev=\"$first_rev\"" >&2
    exit 1
  fi
  exit 0
'

rm -f "$HISTFILE"
exit 0
