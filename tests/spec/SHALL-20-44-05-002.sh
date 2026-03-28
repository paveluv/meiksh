# SHALL-20-44-05-002
# "The following operands shall be supported:: first, last"
# Verify fc accepts first and last operands as number, negative offset, or string.

set -e
HISTFILE="$TMPDIR/hist_$$"
export HISTFILE
HISTSIZE=100
export HISTSIZE

${SHELL} -c '
  HISTFILE="'"$HISTFILE"'"
  HISTSIZE=100
  echo cmd_alpha
  echo cmd_bravo
  echo cmd_charlie
  # Negative offset form
  out=$(fc -l -n -2 -1)
  case "$out" in
    *cmd_bravo*) ;;
    *) echo "FAIL: negative offset did not find cmd_bravo: $out" >&2; exit 1 ;;
  esac
  # String prefix form
  out2=$(fc -l -n cmd_a cmd_a)
  case "$out2" in
    *cmd_alpha*) ;;
    *) echo "FAIL: string prefix did not find cmd_alpha: $out2" >&2; exit 1 ;;
  esac
  exit 0
'

rm -f "$HISTFILE"
exit 0
